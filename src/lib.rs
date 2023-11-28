mod bbuf;
pub mod http;
mod socket;

pub use http::{headers::HttpHeader, method::Method};
pub use socket::Socket;
