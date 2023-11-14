use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

pub trait Socket: AsyncRead + AsyncWrite + Unpin {}
impl Socket for TcpStream {}
