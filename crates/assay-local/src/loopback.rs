//! Loopback-only TCP binding for the local dashboard server.

use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};

/// A TCP listener that can only ever bind the IPv4 loopback interface.
///
/// No constructor accepts a caller-chosen address, so binding a routable
/// interface is unrepresentable rather than merely rejected at runtime.
#[derive(Debug)]
pub struct LoopbackListener {
    listener: TcpListener,
}

impl LoopbackListener {
    /// Binds `127.0.0.1:<port>`. A port of `0` selects an ephemeral port.
    pub fn bind(port: u16) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
        Ok(Self { listener })
    }

    /// Returns the bound loopback socket address.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Accepts the next inbound loopback connection.
    pub fn accept(&self) -> io::Result<TcpStream> {
        let (stream, _peer) = self.listener.accept()?;
        Ok(stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bound_address_is_always_loopback() {
        let listener = LoopbackListener::bind(0).expect("bind ephemeral loopback port");
        let address = listener.local_addr().expect("resolve local address");
        assert!(address.ip().is_loopback());
        assert_eq!(address.ip(), Ipv4Addr::LOCALHOST);
        assert_ne!(address.port(), 0);
    }
}
