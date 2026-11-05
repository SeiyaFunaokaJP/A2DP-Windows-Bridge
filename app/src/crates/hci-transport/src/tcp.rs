//! H4 over TCP: a virtual controller of tools/emu (Bumble), for development
//! and the end-to-end tests. Synchronous for BTstack: a packet is written to
//! the socket before `send` returns.

use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::thread::JoinHandle;
use std::time::Duration;

use btstack_sys::{HciBackend, Upstream};

use crate::reassembly::packet_length;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct TcpH4 {
    address: String,
    stream: Option<TcpStream>,
    reader: Option<JoinHandle<()>>,
    /// Type byte + packet, reused for each write
    out: Vec<u8>,
}

impl TcpH4 {
    /// `address`: "host:port"
    pub fn new(address: impl Into<String>) -> TcpH4 {
        TcpH4 { address: address.into(), stream: None, reader: None, out: Vec::with_capacity(2048) }
    }
}

impl HciBackend for TcpH4 {
    fn is_async(&self) -> bool {
        false
    }

    fn open(&mut self, upstream: Upstream) -> Result<(), String> {
        self.close();
        let addr = self
            .address
            .to_socket_addrs()
            .map_err(|e| format!("{}: {e}", self.address))?
            .next()
            .ok_or_else(|| format!("{}: no address", self.address))?;
        tracing::debug!("H4 over TCP: connecting to {addr} ({})", self.address);
        let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(|e| {
            tracing::debug!("H4 over TCP: connecting to {addr} failed: {e} ({:?})", e.kind());
            format!("{addr}: {e}")
        })?;
        // Small HCI packets must not wait for Nagle
        stream.set_nodelay(true).map_err(|e| e.to_string())?;
        let read_side = stream.try_clone().map_err(|e| e.to_string())?;
        tracing::info!(target: "hci", "H4 over TCP connected to {addr}");
        self.reader = Some(
            std::thread::Builder::new()
                .name("hci-tcp-rx".into())
                .spawn(move || read_loop(read_side, upstream))
                .map_err(|e| e.to_string())?,
        );
        self.stream = Some(stream);
        Ok(())
    }

    fn close(&mut self) {
        if let Some(stream) = self.stream.take() {
            tracing::debug!("H4 over TCP: closing the connection to {}", self.address);
            if let Err(e) = stream.shutdown(Shutdown::Both) {
                tracing::debug!("H4 over TCP: shutdown: {e}");
            }
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
            tracing::debug!("H4 over TCP: reader stopped");
        }
    }

    fn send(&mut self, packet_type: u8, packet: &[u8]) -> bool {
        let Some(stream) = self.stream.as_mut() else {
            tracing::debug!("H4 over TCP: packet type {packet_type} not sent, not connected");
            return false;
        };
        tracing::trace!("H4 over TCP: type {packet_type} out, {} bytes", packet.len());
        // One write for type and packet, so they travel in one segment
        self.out.clear();
        self.out.push(packet_type);
        self.out.extend_from_slice(packet);
        match stream.write_all(&self.out) {
            Ok(()) => true,
            Err(e) => {
                tracing::error!(target: "hci", "H4 over TCP write failed: {e}");
                false
            }
        }
    }
}

impl Drop for TcpH4 {
    fn drop(&mut self) {
        self.close();
    }
}

/// Reads H4 frames (type byte, then the packet) until the socket closes
fn read_loop(mut stream: TcpStream, upstream: Upstream) {
    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    let mut chunk = [0u8; 4096];
    tracing::debug!("H4 over TCP: reader started");
    loop {
        let n = match stream.read(&mut chunk) {
            Ok(0) => {
                tracing::debug!("H4 over TCP: end of stream ({} bytes buffered)", buf.len());
                upstream.fail("the virtual controller closed the connection");
                return;
            }
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => {
                tracing::debug!("H4 over TCP: read: {e} ({:?})", e.kind());
                // shutdown() by close() ends up here too; fail() is ignored
                // once the run that opened us has ended
                if !matches!(e.kind(), ErrorKind::ConnectionAborted | ErrorKind::NotConnected) {
                    upstream.fail(&format!("read error: {e}"));
                }
                return;
            }
        };
        buf.extend_from_slice(&chunk[..n]);
        let mut start = 0;
        while buf.len() > start {
            let packet_type = buf[start];
            let Some(len) = packet_length(packet_type, &buf[start + 1..]) else {
                if !matches!(packet_type, 1..=5) {
                    upstream.fail(&format!("bad H4 packet type 0x{packet_type:02x}"));
                    return;
                }
                break; // header incomplete
            };
            if buf.len() - start - 1 < len {
                break;
            }
            tracing::trace!("H4 over TCP: type {packet_type} in, {len} bytes");
            upstream.deliver(packet_type, &buf[start + 1..start + 1 + len]);
            start += 1 + len;
        }
        buf.drain(..start);
    }
}
