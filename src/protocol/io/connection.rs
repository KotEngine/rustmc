//! Blocking TCP/UDP connections used by the sync protocol clients.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::time::Duration;

use crate::error::RustmcError;

/// A connected TCP socket with `TCP_NODELAY` set (status pings are
/// latency-sensitive request/response exchanges, Nagle's algorithm only
/// hurts here).
pub struct TcpConnection {
    stream: TcpStream,
}

impl TcpConnection {
    pub fn connect(addr: SocketAddr, timeout: Duration) -> Result<Self, RustmcError> {
        let stream = TcpStream::connect_timeout(&addr, timeout).map_err(RustmcError::Io)?;
        stream.set_nodelay(true).map_err(RustmcError::Io)?;
        stream.set_read_timeout(Some(timeout)).map_err(RustmcError::Io)?;
        stream.set_write_timeout(Some(timeout)).map_err(RustmcError::Io)?;
        Ok(Self { stream })
    }

    pub fn write_all(&mut self, data: &[u8]) -> Result<(), RustmcError> {
        self.stream.write_all(data).map_err(RustmcError::Io)
    }

    /// Reads exactly `length` bytes, looping over short reads. Mirrors a
    /// real socket's behavior: a `0`-byte read before `length` is reached
    /// means the peer closed the connection.
    pub fn read_exact(&mut self, length: usize) -> Result<Vec<u8>, RustmcError> {
        let mut result = vec![0u8; length];
        let mut read = 0;
        while read < length {
            let n = self.stream.read(&mut result[read..]).map_err(RustmcError::Io)?;
            if n == 0 {
                return Err(RustmcError::InvalidResponse(
                    "server closed the connection before sending the expected data".into(),
                ));
            }
            read += n;
        }
        Ok(result)
    }
}

/// A UDP socket bound to a single target via `connect()`. After `connect()`
/// the OS filters incoming datagrams: `recv()` only returns data sent from
/// `target`, packets from any other source (including spoofed ones on the
/// same LAN) are dropped by the kernel before reaching this code. This does
/// not defend against IP spoofing across real network infrastructure — that
/// is out of scope for a client library — but it closes off the trivial
/// "random host on the same network replies first" attack that `recv_from`
/// with an unconnected socket would be vulnerable to.
pub struct UdpConnection {
    socket: UdpSocket,
}

impl UdpConnection {
    pub fn connect(target: SocketAddr, timeout: Duration) -> Result<Self, RustmcError> {
        let bind_addr = if target.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
        let socket = UdpSocket::bind(bind_addr).map_err(RustmcError::Io)?;
        socket.connect(target).map_err(RustmcError::Io)?;
        socket.set_read_timeout(Some(timeout)).map_err(RustmcError::Io)?;
        socket.set_write_timeout(Some(timeout)).map_err(RustmcError::Io)?;
        Ok(Self { socket })
    }

    pub fn send(&self, data: &[u8]) -> Result<(), RustmcError> {
        self.socket.send(data).map_err(RustmcError::Io)?;
        Ok(())
    }

    /// Reads a single datagram. Buffer is `65527` bytes — the maximum
    /// possible UDP payload — allocated once, not grown based on untrusted
    /// input.
    pub fn recv(&self) -> Result<Vec<u8>, RustmcError> {
        let mut buf = vec![0u8; 65527];
        let n = self.socket.recv(&mut buf).map_err(RustmcError::Io)?;
        buf.truncate(n);
        Ok(buf)
    }
}

#[cfg(feature = "async")]
mod tokio_impl {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpStream, UdpSocket};
    use tokio::time::timeout as tokio_timeout;

    /// Async counterpart to `TcpConnection`. Same `TCP_NODELAY` reasoning
    /// applies — status pings are a single small request/response, Nagle
    /// buffering only adds latency.
    pub struct AsyncTcpConnection {
        stream: TcpStream,
        timeout: Duration,
    }

    impl AsyncTcpConnection {
        pub async fn connect(addr: SocketAddr, timeout: Duration) -> Result<Self, RustmcError> {
            let stream = tokio_timeout(timeout, TcpStream::connect(addr))
                .await
                .map_err(|_| RustmcError::Timeout(timeout))?
                .map_err(RustmcError::Io)?;
            stream.set_nodelay(true).map_err(RustmcError::Io)?;
            Ok(Self { stream, timeout })
        }

        pub async fn write_all(&mut self, data: &[u8]) -> Result<(), RustmcError> {
            tokio_timeout(self.timeout, self.stream.write_all(data))
                .await
                .map_err(|_| RustmcError::Timeout(self.timeout))?
                .map_err(RustmcError::Io)
        }

        pub async fn read_exact(&mut self, length: usize) -> Result<Vec<u8>, RustmcError> {
            let mut result = vec![0u8; length];
            tokio_timeout(self.timeout, self.stream.read_exact(&mut result))
                .await
                .map_err(|_| RustmcError::Timeout(self.timeout))?
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        RustmcError::InvalidResponse(
                            "server closed the connection before sending the expected data".into(),
                        )
                    } else {
                        RustmcError::Io(e)
                    }
                })?;
            Ok(result)
        }
    }

    /// Async counterpart to `UdpConnection`; same `connect()`-based
    /// spoofing-resistance rationale (see `UdpConnection`'s docs).
    pub struct AsyncUdpConnection {
        socket: UdpSocket,
        timeout: Duration,
    }

    impl AsyncUdpConnection {
        pub async fn connect(target: SocketAddr, timeout: Duration) -> Result<Self, RustmcError> {
            let bind_addr = if target.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
            let socket = UdpSocket::bind(bind_addr).await.map_err(RustmcError::Io)?;
            tokio_timeout(timeout, socket.connect(target))
                .await
                .map_err(|_| RustmcError::Timeout(timeout))?
                .map_err(RustmcError::Io)?;
            Ok(Self { socket, timeout })
        }

        pub async fn send(&self, data: &[u8]) -> Result<(), RustmcError> {
            tokio_timeout(self.timeout, self.socket.send(data))
                .await
                .map_err(|_| RustmcError::Timeout(self.timeout))?
                .map_err(RustmcError::Io)?;
            Ok(())
        }

        pub async fn recv(&self) -> Result<Vec<u8>, RustmcError> {
            let mut buf = vec![0u8; 65527];
            let n = tokio_timeout(self.timeout, self.socket.recv(&mut buf))
                .await
                .map_err(|_| RustmcError::Timeout(self.timeout))?
                .map_err(RustmcError::Io)?;
            buf.truncate(n);
            Ok(buf)
        }
    }
}

#[cfg(feature = "async")]
pub use tokio_impl::{AsyncTcpConnection, AsyncUdpConnection};
