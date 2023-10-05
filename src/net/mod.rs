/*use std::net::{ToSocketAddrs, UdpSocket};

use common::net::udp::SocketError;

pub struct Socket {
    socket: UdpSocket,
}

impl Socket {
    fn bind<A: ToSocketAddrs>(addrs: A) -> Result<Self, SocketError> {
        match UdpSocket::bind(addrs) {
            Ok(mut socket) => {
                socket
                    .set_nonblocking(true)
                    .map_err(|_| SocketError::Nonblocking)?;
                Ok(Self { socket })
            }
            Err(_) => Err(SocketError::Bind),
        }
    }

    fn connect<A: ToSocketAddrs>(&self, addrs: A) -> Result<(), SocketError> {
        self.socket.connect(addrs).map_err(|_| SocketError::Connect)
    }
}

pub mod client {}
*/