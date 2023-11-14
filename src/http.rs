use tokio::net::{TcpStream, ToSocketAddrs};

use crate::{Method, Socket};

#[derive(Debug)]
pub struct HttpContext<S: Socket = TcpStream> {
    socket: S,
}

impl HttpContext {
    pub async fn new(host: impl ToSocketAddrs) -> anyhow::Result<Self> {
        Ok(Self {
            socket: TcpStream::connect(host).await?,
        })
    }
}

impl<S: Socket> HttpContext<S> {
    pub fn begin(&mut self) {}

    pub fn end(&mut self) {}

    pub fn begin_request(&mut self, method: Method, resource: impl AsRef<str>) {}

    pub fn end_request(&mut self) {}

    pub fn request_header(&mut self, name: impl AsRef<str>, value: impl AsRef<str>) {}

    pub fn request_headers_end(&mut self) {}

    pub fn request_body_begin(&mut self) {}

    pub fn request_body_chunk(&mut self, chunk: impl AsRef<[u8]>) {}
}
