#![deny(clippy::all)]
#![allow(dead_code)]

use std::net::TcpStream;

pub enum StreamSocket {
    Tcp(TcpStream),
}
