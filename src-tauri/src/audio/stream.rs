use std::net::TcpStream;

// Abstract Stream Socket (TCP Only now)
pub enum StreamSocket {
    Tcp(TcpStream),
}

// No methods needed for now as we access fields directly via pattern matching
// impl StreamSocket {}
