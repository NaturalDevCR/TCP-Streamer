use std::net::TcpStream;

pub enum StreamSocket {
    Tcp(TcpStream),
}
