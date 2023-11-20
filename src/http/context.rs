use crate::{Method, Socket};
use anyhow::Context;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};

use super::headers::{HeaderIter, HttpHeader};
use super::skip_line;
use super::status_line::Status;
#[derive(Debug)]
pub struct HttpContext<S: Socket = TcpStream> {
    socket: S,
    pub response_meta: Vec<u8>,
    pub response_first_chunk: Vec<u8>,
}

impl HttpContext {
    pub async fn new(host: impl ToSocketAddrs) -> anyhow::Result<Self> {
        Ok(Self {
            socket: TcpStream::connect(host)
                .await
                .context("establish connection to some host")?,
            response_meta: vec![],
            response_first_chunk: vec![],
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
        let msg = format!("{} {} HTTP/1.0\r\n", method.as_ref(), resource.as_ref());
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
                        self.response_first_chunk
                            .extend_from_slice(&buf[body_index..n]);
                    }
                    break;
                }
                None => self.response_meta.extend_from_slice(&buf[..n]),
            }
        }
        Ok(())
    }

    pub fn status(&self) -> anyhow::Result<Status> {
        Status::new(&self.response_meta)
    }

    pub fn response_header_iter(&self) -> HeaderIter {
        HeaderIter::new(skip_line(&self.response_meta))
    }

    pub async fn response_body_chunk_read(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        let need = buf.len();
        let has = self.response_first_chunk.len();
        if self.response_first_chunk.len() > need {
            buf.copy_from_slice(&self.response_first_chunk[..need]);
            self.response_first_chunk = Vec::from(&self.response_first_chunk[need..]);
            return Ok(need);
        } else {
            let place = &mut buf[..has];
            place.copy_from_slice(&self.response_first_chunk);
            self.response_first_chunk.clear();
        }
        match self.socket.read(&mut buf[has..]).await {
            Ok(has_read) => Ok(has + has_read),
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
}

impl<S: Socket> HttpContext<S> {
    async fn write_str(&mut self, data: &str) -> anyhow::Result<()> {
        self.socket
            .write_all(data.as_bytes())
            .await
            .context("write str to socket")
    }
}
