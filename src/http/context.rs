use std::cell::RefCell;
use std::ops::{AddAssign, DerefMut};

use crate::{Method, Socket};
use anyhow::Context;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};

use self::context_state::State;

use super::headers::{HeaderIter, HttpHeader};
use super::skip_line;
use super::status_line::Status;
#[derive(Debug)]
pub struct HttpContext<S: Socket = TcpStream> {
    socket: S,
    pub response_meta: Vec<u8>,
    pub response_start: Vec<u8>,
    state: RefCell<State>,
}

impl HttpContext {
    pub async fn new(host: impl ToSocketAddrs) -> anyhow::Result<Self> {
        Ok(Self {
            socket: TcpStream::connect(host)
                .await
                .context("establish connection to some host")?,
            response_meta: vec![],
            response_start: vec![],
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
                        self.response_start.extend_from_slice(&buf[body_index..n]);
                    }
                    break;
                }
                None => self.response_meta.extend_from_slice(&buf[..n]),
            }
        }
        for header in self.response_header_iter() {
            if let HttpHeader::ContentLength(content_length) = header {
                self.state.replace(State::Content {
                    content_length,
                    bytes_read: 0,
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
            } => self.get_response_chunk(buf).await,
            State::Chunked {
                chunk_size,
                bytes_read: bytes_left,
            } => todo!(),
            State::Exhausted => Err(anyhow::Error::msg("context exhausted")),
        }
    }

    async fn get_response_chunk(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        let need = buf.len().min(self.bytes_wait()?);
        let has = self.response_start.len();
        if has > need {
            println!("start has {}, need {need}", self.response_start.len());
            buf.copy_from_slice(&self.response_start[..need]);
            self.response_start = Vec::from(&self.response_start[need..]);
            self.reduce_bytes(need)?;
            return Ok(need);
        } else if has > 0 {
            let place = &mut buf[..has];
            place.copy_from_slice(&self.response_start);
            self.response_start.clear();
            println!("has = {}", has)
        }
        match self.socket.read(&mut buf[has..need]).await {
            Ok(has_read) => {
                let result = has + has_read;
                if result != buf.len() {
                    println!("have read {} but asked {}", result, buf.len());
                }
                self.reduce_bytes(result)?;
                Ok(result)
            }
            Err(e) => {
                if has > 0 {
                    Ok(has)
                } else {
                    Err(e).context("cannot read from connection")
                }
            }
        }
    }

    pub fn response_end(&mut self) {}
}

impl<S: Socket> HttpContext<S> {
    pub fn check_response_length(&self) -> Option<usize> {
        for header in self.response_header_iter() {
            if let HttpHeader::ContentLength(size) = header {
                return Some(size);
            }
        }
        None
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
