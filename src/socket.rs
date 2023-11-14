use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

pub trait Socket: AsyncRead + AsyncWrite {}
impl Socket for TcpStream {}
