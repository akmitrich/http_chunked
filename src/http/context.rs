use std::cell::RefCell;
use std::ops::{AddAssign, DerefMut};

use crate::{Method, Socket};
use anyhow::Context as AnyHowContext;
use tokio::net::{TcpStream, ToSocketAddrs};

use self::context_state::State;

use super::headers::{HeaderIter, HttpHeader};
use super::skip_line;
use super::status_line::Status;
#[derive(Debug)]
pub struct Context<S: Socket = TcpStream> {
    buffer: crate::bbuf::Buffer<S>,
    response_meta: Vec<u8>,
    state: RefCell<State>,
}

impl Context {
    pub async fn new(host: impl ToSocketAddrs) -> anyhow::Result<Self> {
        Ok(Self {
            buffer: crate::bbuf::Buffer::new(
                TcpStream::connect(host)
                    .await
                    .context("establish connection to some host")?,
            ),
            response_meta: vec![],
            state: RefCell::new(State::SendingRequest),
        })
    }
    pub fn host(&self) -> String {
        self.buffer.socket_addr().unwrap().to_string()
    }
}

impl<S: Socket> Context<S> {
    pub fn begin(&mut self) {}

    pub fn end(&mut self) {}

    // TODO: check resource for correct value as there is a risk that request might become malformed
    pub async fn begin_request(
        &mut self,
        method: Method,
        resource: impl AsRef<str>,
    ) -> anyhow::Result<()> {
        let msg = format!("{} {} HTTP/1.1\r\n", method.as_ref(), resource.as_ref());
        self.buffer.write_str(&msg).await.context("send start line")
    }

    pub fn end_request(&mut self) {}

    pub async fn request_header(&mut self, header: HttpHeader) -> anyhow::Result<()> {
        let msg = format!("{}\r\n", header.to_string());
        self.buffer.write_str(&msg).await.context("request header")
    }

    pub async fn request_headers_end(&mut self) -> anyhow::Result<()> {
        self.buffer
            .write_str("\r\n")
            .await
            .context("end of request headers")
    }

    pub async fn request_body_chunk(&mut self, chunk: impl AsRef<[u8]>) -> anyhow::Result<()> {
        self.buffer
            .write_some_bytes(chunk)
            .await
            .context("send request body chunk")
    }
}

impl<S: Socket> Context<S> {
    pub async fn response_begin(&mut self) -> anyhow::Result<()> {
        self.response_meta = self.buffer.read_until_and_chop(b"\r\n\r\n").await?;

        for header in self.response_header_iter() {
            if let HttpHeader::ContentLength(content_length) = header {
                if let State::Chunked {
                    chunk_size: _,
                    bytes_read: _,
                } = self.state()
                {
                    // ignore content-length header
                } else {
                    self.state.replace(State::Content {
                        content_length,
                        bytes_read: 0,
                    });
                }
            }
            if let HttpHeader::TransferEncodingChunked = header {
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
        let result = self.buffer.read_some_bytes(&mut buf[..need]).await?;
        self.reduce_bytes(result)?;
        Ok(result)
    }

    pub fn response_end(&mut self) {}
}

impl<S: Socket> Context<S> {
    pub fn has_response(&self) -> bool {
        match self.state() {
            State::SendingRequest => false,
            State::Content {
                content_length,
                bytes_read,
            } => bytes_read < content_length,
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

    pub fn debug(&self) -> String {
        format!(
            "Rolling buffer is: {:?}",
            std::str::from_utf8(self.buffer.buffer())
        )
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
            }
            | State::Chunked {
                chunk_size: _,
                bytes_read,
            } => bytes_read.add_assign(bytes),
            _ => return Err(anyhow::Error::msg("reduce bytes")),
        }
        Ok(())
    }

    async fn start_chunk(&mut self) -> anyhow::Result<()> {
        let chunk_size_line = self
            .buffer
            .read_line()
            .await
            .context("read chunk header from socket")?;
        let chunk_size_str =
            std::str::from_utf8(&chunk_size_line).context("chunk header contains non-UTF8")?;
        let chunk_size =
            usize::from_str_radix(chunk_size_str, 16).context("chunk header is not hexadecimal")?;
        if chunk_size == 0 {
            self.state.replace(State::Exhausted);
        } else {
            self.state.replace(State::Chunked {
                chunk_size,
                bytes_read: 0,
            });
        }
        Ok(())
    }

    async fn end_chunk(&mut self) -> anyhow::Result<()> {
        self.buffer.read_line().await?;
        Ok(())
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
