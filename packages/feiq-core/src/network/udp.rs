//! Async UDP socket with broadcast support for IP Messenger protocol.
//! Binds to port 2425, enables SO_BROADCAST and SO_REUSEADDR.

use crate::protocol::constants::MAX_RCV_SIZE;
use mac_address::get_mac_address;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::SocketAddr;
use tokio::net::UdpSocket;

/// UDP transport layer for IPMSG communication
pub struct UdpTransport {
    socket: UdpSocket,
    bound_mac: String,
}

impl UdpTransport {
    /// Create and bind a UDP socket to the given port
    pub async fn bind(port: u16) -> anyhow::Result<Self> {
        // Create a socket2 socket with SO_REUSEADDR enabled for multi-instance support
        let addr: SocketAddr = format!("0.0.0.0:{port}").parse()?;
        let socket2_socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        socket2_socket.set_reuse_address(true)?;
        socket2_socket.set_nonblocking(true)?;
        let sock_addr: socket2::SockAddr = addr.into();
        socket2_socket.bind(&sock_addr)?;
        let socket = UdpSocket::from_std(socket2_socket.into())?;

        // Enable broadcast
        socket.set_broadcast(true)?;

        // Get MAC address of the bound interface
        let bound_mac = get_mac_address()
            .ok()
            .flatten()
            .map(|ma| ma.to_string().replace(':', ""))
            .unwrap_or_default();

        tracing::info!(
            "UDP bound to port {port}, MAC={bound_mac}",
        );

        Ok(Self { socket, bound_mac })
    }

    /// Get the MAC address detected at bind time
    pub fn mac(&self) -> &str {
        &self.bound_mac
    }

    /// Send data to a specific IP:port
    pub async fn send_to(&self, ip: &str, port: u16, data: &[u8]) -> anyhow::Result<()> {
        let addr: SocketAddr = format!("{ip}:{port}").parse()?;
        let sent = self.socket.send_to(data, addr).await?;
        tracing::trace!("UDP sent {sent} bytes to {ip}:{port}");
        Ok(())
    }

    /// Broadcast data to 255.255.255.255:port
    pub async fn broadcast(&self, port: u16, data: &[u8]) -> anyhow::Result<()> {
        self.send_to("255.255.255.255", port, data).await
    }

    /// Receive one datagram. Returns (data, sender_ip, sender_port).
    pub async fn recv_from(&self) -> anyhow::Result<(Vec<u8>, String, u16)> {
        let mut buf = vec![0u8; MAX_RCV_SIZE];
        let (len, addr) = self.socket.recv_from(&mut buf).await?;
        buf.truncate(len);
        Ok((buf, addr.ip().to_string(), addr.port()))
    }

    /// Get the local address this socket is bound to
    pub fn local_addr(&self) -> anyhow::Result<SocketAddr> {
        Ok(self.socket.local_addr()?)
    }

    /// Access the underlying socket
    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }
}
