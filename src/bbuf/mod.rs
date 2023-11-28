use crate::Socket;
use anyhow::Context;
use std::ops::AddAssign;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

const MAX_BUFFER_SIZE: usize = 4096;

#[derive(Debug)]
pub struct Buffer<S: Socket> {
    inner: [u8; MAX_BUFFER_SIZE],
    begin: usize,
    end: usize,
    socket: S,
}

impl Buffer<TcpStream> {
    pub fn socket_addr(&self) -> anyhow::Result<std::net::SocketAddr> {
        self.socket.peer_addr().context("socket addr")
    }
}

impl<S: Socket> Buffer<S> {
    pub fn new(socket: S) -> Self {
        Self {
            inner: [0; MAX_BUFFER_SIZE],
            begin: 0,
            end: 0,
            socket,
        }
    }

    pub async fn write_str(&mut self, data: &str) -> anyhow::Result<()> {
        self.write_some_bytes(data.as_bytes())
            .await
            .context("write str to socket")
    }

    pub async fn write_some_bytes(&mut self, data: impl AsRef<[u8]>) -> anyhow::Result<()> {
        self.socket
            .write_all(data.as_ref())
            .await
            .context("write some bytes")
    }

    pub async fn read_line(&mut self) -> anyhow::Result<Vec<u8>> {
        self.read_until_and_chop(b"\r\n").await
    }

    pub async fn read_until_and_chop(&mut self, delim: &[u8]) -> anyhow::Result<Vec<u8>> {
        let mut result = vec![];
        loop {
            match self.end_of_line(delim) {
                Some(end_of_line) => {
                    result.extend_from_slice(self.slice(end_of_line)?);
                    self.shift_buffer(end_of_line + delim.len())
                        .context("reach the unreachable: buffer is shorter than expected")?;
                    break;
                }
                None => {
                    result.extend_from_slice(self.buffer());
                    self.refill_buffer().await?;
                }
            }
        }
        Ok(result)
    }

    pub async fn read_some_bytes(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        fn copy_as_much_as_possible(dst: &mut [u8], src: &[u8], result: &mut usize) -> usize {
            let copied = dst.len().min(src.len());
            dst[..copied].copy_from_slice(&src[..copied]);
            result.add_assign(copied);
            copied
        }

        let mut result = 0;
        let has = copy_as_much_as_possible(buf, self.buffer(), &mut result);
        self.shift_buffer(has)?;

        let rest = &mut buf[has..];
        if !rest.is_empty() {
            self.refill_buffer().await.context("read some bytes")?;
            self.shift_buffer(copy_as_much_as_possible(rest, self.buffer(), &mut result))?;
        }
        Ok(result)
    }

    pub fn buffer(&self) -> &[u8] {
        &self.inner[self.begin..self.end]
    }

    pub fn slice(&self, until: usize) -> anyhow::Result<&[u8]> {
        self.buffer()
            .get(..until)
            .ok_or_else(|| anyhow::Error::msg("slice overshoots the end of the buffer"))
    }
}

impl<S: Socket> Buffer<S> {
    async fn refill_buffer(&mut self) -> anyhow::Result<()> {
        self.end = self
            .socket
            .read(&mut self.inner)
            .await
            .context("refill buffer read from socket")?;
        self.begin = 0;
        Ok(())
    }

    fn shift_buffer(&mut self, until: usize) -> anyhow::Result<()> {
        if until > self.end - self.begin {
            Err(anyhow::Error::msg("ask to shift more than have"))
        } else {
            self.begin += until;
            Ok(())
        }
    }

    fn end_of_line(&self, delim: &[u8]) -> Option<usize> {
        if delim.is_empty() {
            return None;
        }
        self.buffer()
            .windows(delim.len())
            .enumerate()
            .find(|(_, w)| w.eq(&delim))
            .map(|(i, _)| i)
    }
}
