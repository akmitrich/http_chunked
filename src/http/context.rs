use std::cell::RefCell;
use std::ops::{AddAssign, DerefMut};

use crate::{Method, Socket};
use anyhow::Context;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};

use self::context_state::State;

use super::headers::{HeaderIter, HttpHeader};
use super::status_line::Status;
use super::{get_line, skip_line};
#[derive(Debug)]
pub struct HttpContext<S: Socket = TcpStream> {
    socket: S,
    pub response_meta: Vec<u8>,
    pub rollin: Vec<u8>,
    state: RefCell<State>,
}

impl HttpContext {
    pub async fn new(host: impl ToSocketAddrs) -> anyhow::Result<Self> {
        Ok(Self {
            socket: TcpStream::connect(host)
                .await
                .context("establish connection to some host")?,
            response_meta: vec![],
            rollin: vec![],
            state: RefCell::new(State::SendingRequest),
        })
    }
    pub fn host(&self) -> String {
        self.socket.peer_addr().unwrap().to_string()
    }
}

impl<S: Socket> HttpContext<S> {
    pub fn begin(&mut self) {}

    pub fn end(&mut self) {}

    // TODO: check resource for correct value as there is a risk that request might become malformed
    pub async fn begin_request(
        &mut self,
        method: Method,
        resource: impl AsRef<str>,
    ) -> anyhow::Result<()> {
        let msg = format!("{} {} HTTP/1.1\r\n", method.as_ref(), resource.as_ref());
        self.write_str(&msg).await.context("send start line")
    }

    pub fn end_request(&mut self) {}

    pub async fn request_header(&mut self, header: HttpHeader) -> anyhow::Result<()> {
        let msg = format!("{}\r\n", header.to_string());
        self.write_str(&msg).await.context("request header")
    }

    pub async fn request_headers_end(&mut self) -> anyhow::Result<()> {
        self.write_str("\r\n")
            .await
            .context("end of request headers")
    }

    pub async fn request_body_chunk(&mut self, chunk: impl AsRef<[u8]>) -> anyhow::Result<()> {
        self.socket
            .write_all(chunk.as_ref())
            .await
            .context("send request body chunk")
    }
}

impl<S: Socket> HttpContext<S> {
    pub async fn response_begin(&mut self) -> anyhow::Result<()> {
        const MAX_HEADERS_SIZE: usize = 4 * 1024;
        let mut buf = [0; MAX_HEADERS_SIZE];
        self.response_meta.clear();
        loop {
            let n = self
                .socket
                .read(&mut buf)
                .await
                .context("read response begin")?;
            if n == 0 {
                break;
            }
            match buf[..n]
                .windows(4)
                .enumerate()
                .find(|(_, w)| w.eq(b"\r\n\r\n"))
            {
                Some((payload_index, _)) => {
                    self.response_meta.extend_from_slice(&buf[..payload_index]);
                    let body_index = payload_index + 4;
                    if body_index < n {
                        self.rollin.extend_from_slice(&buf[body_index..n]);
                    }
                    break;
                }
                None => self.response_meta.extend_from_slice(&buf[..n]),
            }
        }

        // TODO: enquire what is the correct behaviour of the client received both content-length and transfer-encoding: chunked
        for header in self.response_header_iter() {
            if let HttpHeader::ContentLength(content_length) = header {
                self.state.replace(State::Content {
                    content_length,
                    bytes_read: 0,
                });
            }
            if let HttpHeader::TransferEncoding = header {
                self.state.replace(State::Chunked {
                    chunk_size: usize::MAX,
                    bytes_read: usize::MAX,
                });
            }
        }
        Ok(())
    }

    pub fn status(&self) -> anyhow::Result<Status> {
        Status::new(&self.response_meta)
    }

    pub fn state(&self) -> State {
        *self.state.borrow()
    }

    pub fn response_header_iter(&self) -> HeaderIter {
        HeaderIter::new(skip_line(&self.response_meta))
    }

    pub async fn response_body_chunk_read(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        match self.state() {
            State::SendingRequest => Err(anyhow::Error::msg(
                "unexpected read call while sending request",
            )),
            State::Content {
                content_length: _,
                bytes_read: _,
            } => {
                let n = self.get_chunk(buf).await?;
                if self.bytes_wait()? == 0 {
                    self.state.replace(State::Exhausted);
                }
                Ok(n)
            }
            State::Chunked {
                chunk_size: _,
                bytes_read: _,
            } => {
                if self.bytes_wait()? == 0 {
                    self.start_chunk().await?;
                }
                let n = if self.bytes_wait().is_ok() {
                    self.get_chunk(buf).await?
                } else {
                    return Ok(0);
                };
                println!(
                    "Read chunk. Left in rolling: {:?}",
                    std::str::from_utf8(&self.rollin).unwrap()
                );
                if self.bytes_wait()? == 0 {
                    self.end_chunk().await?;
                }
                Ok(n)
            }
            State::Exhausted => Err(anyhow::Error::msg("context exhausted")),
        }
    }

    async fn get_chunk(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        let need = buf.len().min(self.bytes_wait()?);
        let start = self.rollin.len();
        if start > need {
            println!("start has {}, need {need}", self.rollin.len());
            buf[..need].copy_from_slice(&self.rollin[..need]);
            self.rollin = self.rollin[need..].to_vec();
            self.reduce_bytes(need)?;
            return Ok(need);
        }

        buf[..start].copy_from_slice(&self.rollin);
        self.rollin.clear();
        self.reduce_bytes(start)?;

        println!("Get chunk. Look to socket");
        match self.socket.read(&mut buf[start..need]).await {
            Ok(has_read) => {
                if has_read + start != buf.len() {
                    println!("has read {} but asked {}", has_read + start, buf.len());
                }
                self.reduce_bytes(has_read)?;
                Ok(has_read + start)
            }
            Err(e) => {
                if start > 0 {
                    Ok(start)
                } else {
                    Err(e).context("cannot read from connection")
                }
            }
        }
    }

    pub fn response_end(&mut self) {}
}

impl<S: Socket> HttpContext<S> {
    pub fn has_response(&self) -> bool {
        match self.state() {
            State::SendingRequest => false,
            State::Content {
                content_length: _,
                bytes_read: _,
            } => true,
            State::Chunked {
                chunk_size: _,
                bytes_read: _,
            } => true,
            State::Exhausted => false,
        }
    }

    pub fn content_length(&self) -> anyhow::Result<usize> {
        if let State::Content {
            content_length,
            bytes_read: _,
        } = self.state()
        {
            Ok(content_length)
        } else {
            Err(anyhow::Error::msg("no content length"))
        }
    }

    fn bytes_wait(&self) -> anyhow::Result<usize> {
        match self.state() {
            State::Content {
                content_length,
                bytes_read,
            } => Ok(content_length - bytes_read),
            State::Chunked {
                chunk_size,
                bytes_read,
            } => Ok(chunk_size - bytes_read),
            _ => Err(anyhow::Error::msg("ask for bytes read")),
        }
    }

    fn reduce_bytes(&mut self, bytes: usize) -> anyhow::Result<()> {
        match self.state.borrow_mut().deref_mut() {
            State::Content {
                content_length: _,
                bytes_read,
            } => bytes_read.add_assign(bytes),
            State::Chunked {
                chunk_size: _,
                bytes_read,
            } => bytes_read.add_assign(bytes),
            _ => return Err(anyhow::Error::msg("ask for bytes reduce")),
        }
        Ok(())
    }

    async fn start_chunk(&mut self) -> anyhow::Result<()> {
        loop {
            let chunk_size_str =
                std::str::from_utf8(get_line(&self.rollin)).context("start chunk with non-UTF8")?;
            if self.rollin.len() > chunk_size_str.len() {
                let chunk_size = usize::from_str_radix(chunk_size_str, 16)
                    .context("chunk header is not hexadecimal")?;
                println!("Start chunk. Size = {} {:x}", chunk_size, chunk_size);
                if chunk_size == 0 {
                    self.state.replace(State::Exhausted);
                } else {
                    self.state.replace(State::Chunked {
                        chunk_size,
                        bytes_read: 0,
                    });
                }
                self.rollin = skip_line(&self.rollin).to_vec();
                break;
            }
            let mut buf = [0; 1024];
            println!("Start chunk. Look to socket");
            let n = self.socket.read(&mut buf).await?;
            if n == 0 {
                println!("Start chunk. wait 0.1 s");
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            } else {
                self.rollin.extend_from_slice(&buf[..n]);
            }
        }
        Ok(())
    }

    async fn end_chunk(&mut self) -> anyhow::Result<()> {
        loop {
            if self.rollin.starts_with(b"\r\n") {
                self.rollin = self.rollin[2..].to_vec();
                println!(
                    "End chunk. Left in rollin: {:?}",
                    std::str::from_utf8(&self.rollin)
                );
                break;
            }
            let mut buf = [0; 2];
            println!("End chunk. Look to socket");
            let n = self.socket.read(&mut buf).await?;
            if n == 0 {
                println!("End chunk. wait 0.1 s");
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            } else {
                self.rollin.extend_from_slice(&buf[..n]);
            }
        }
        Ok(())
    }
}

impl<S: Socket> HttpContext<S> {
    async fn write_str(&mut self, data: &str) -> anyhow::Result<()> {
        self.socket
            .write_all(data.as_bytes())
            .await
            .context("write str to socket")
    }
}

mod context_state {
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum State {
        SendingRequest,
        Content {
            content_length: usize,
            bytes_read: usize,
        },
        Chunked {
            chunk_size: usize,
            bytes_read: usize,
        },
        Exhausted,
    }
}
