use crate::{Method, Socket};
use anyhow::{Context, Ok};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};

#[derive(Debug)]
pub struct HttpContext<S: Socket = TcpStream> {
    socket: S,
    response_meta: Vec<u8>,
    response_body_chunk: Vec<u8>,
}

impl HttpContext {
    pub async fn new(host: impl ToSocketAddrs) -> anyhow::Result<Self> {
        Ok(Self {
            socket: TcpStream::connect(host)
                .await
                .context("establish connection to some host")?,
            response_meta: vec![],
            response_body_chunk: vec![],
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
        self.write_str(&msg).await.context("begin request")
    }

    pub fn end_request(&mut self) {}

    pub async fn request_header(
        &mut self,
        name: impl AsRef<str>,
        value: impl AsRef<str>,
    ) -> anyhow::Result<()> {
        let msg = format!("{}: {}\r\n", name.as_ref(), value.as_ref());
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
            .context("request body chunk")
    }
}

impl<S: Socket> HttpContext<S> {
    pub async fn response_begin(&mut self) -> anyhow::Result<()> {
        const RESPONSE_HEADERS_SIZE: usize = 8 * 1024;
        let mut buf = [0; RESPONSE_HEADERS_SIZE];
        let n = self
            .socket
            .read(&mut buf)
            .await
            .context("read response begin")?;
        let payload_index = buf[..n]
            .windows(4)
            .enumerate()
            .find(|(_, w)| w.eq(b"\r\n\r\n"))
            .map(|(i, _)| i)
            .ok_or_else(|| anyhow::Error::msg("8kB was not enough for response headers"))?;
        self.response_meta = buf[..payload_index].to_owned();
        Ok(())
    }

    pub fn response_end(&mut self) {}
}

impl<S: Socket> HttpContext<S> {
    async fn write_str(&mut self, data: &str) -> anyhow::Result<()> {
        self.socket
            .write_all(data.as_bytes())
            .await
            .context("write str to socket")
            .map_err(|e| e.into())
    }
}
