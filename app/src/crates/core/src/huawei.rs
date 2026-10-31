//! Huawei / Honor earbuds list LDAC only in their "sound quality first" mode
//! (issue #30), which their app sets over the earbuds' own protocol on the
//! serial port service. A profile that asks for LDAC of such earbuds sets
//! it the same way: they then offer LDAC on the next connection.
//!
//! The protocol as HFreebuds documents it (N2K02, MIT,
//! THIRD_PARTY_LICENSES.md). A packet is `5A | length (2 bytes, big endian)
//! | 00 | command (2 bytes) | params | CRC-16/XMODEM of all before it`; the
//! length counts from the `00` to the params, a param is `type | length |
//! value`. A read lists the types wanted with empty values and is answered
//! by the same command.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use bt_host::{BtHost, RfcommEvent};

const SERIAL_PORT_UUID: &str = "1101";
const QUALITY_WRITE: u16 = 0x2BA2;
const QUALITY_READ: u16 = 0x2BA3;
const DUAL_CONNECT_READ: u16 = 0x2B2F;
/// The read as the official app sends it to the FreeBuds 5, for earbuds that
/// do not answer the usual one
const QUALITY_READ_CAPTURED: [u8; 10] = [0x5A, 0x00, 0x05, 0x00, 0x2B, 0xA3, 0x01, 0x01, 0xF7, 0x94];

/// How long a read is waited for
const ANSWER_TIMEOUT: Duration = Duration::from_millis(1500);
/// From a write to its read back
const WRITE_SETTLE: Duration = Duration::from_millis(400);
/// Longest packet accepted (more: not a packet start)
const MAX_LENGTH: usize = 4096;

/// Earbuds of this protocol, by their name
pub fn is_huawei(device_name: &str) -> bool {
    let name = device_name.to_lowercase();
    ["huawei", "freebuds", "freeclip", "freelace", "freearc", "honor earbuds"].iter().any(|n| name.contains(n))
}

fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0u16;
    for &b in data {
        crc ^= u16::from(b) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        }
    }
    crc
}

fn packet(command: u16, params: &[(u8, &[u8])]) -> Vec<u8> {
    let length = 3 + params.iter().map(|(_, v)| 2 + v.len()).sum::<usize>();
    let mut p = vec![0x5A, (length >> 8) as u8, length as u8, 0];
    p.extend(command.to_be_bytes());
    for (kind, value) in params {
        p.extend([*kind, value.len() as u8]);
        p.extend(*value);
    }
    p.extend(crc16(&p).to_be_bytes());
    p
}

/// A packet read
#[derive(Debug, PartialEq, Eq)]
struct Answer {
    command: u16,
    params: Vec<(u8, Vec<u8>)>,
}

impl Answer {
    /// The value of a param of one byte
    fn byte(&self, kind: u8) -> Option<u8> {
        self.params.iter().find(|(k, v)| *k == kind && v.len() == 1).map(|(_, v)| v[0])
    }

    /// A packet whose checksum was checked
    fn parse(frame: &[u8]) -> Option<Answer> {
        let command = u16::from_be_bytes([*frame.get(4)?, *frame.get(5)?]);
        let mut rest = frame.get(6..frame.len() - 2)?;
        let mut params = Vec::new();
        while let [kind, length, tail @ ..] = rest {
            let Some(value) = tail.get(..*length as usize) else { break };
            params.push((*kind, value.to_vec()));
            rest = &tail[value.len()..];
        }
        Some(Answer { command, params })
    }
}

/// Cuts the bytes received into packets with a valid checksum
#[derive(Default)]
struct Packets {
    buf: Vec<u8>,
}

impl Packets {
    fn feed(&mut self, data: &[u8]) -> Vec<Answer> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        loop {
            let Some(start) = self.buf.iter().position(|&b| b == 0x5A) else {
                self.buf.clear();
                return out;
            };
            self.buf.drain(..start);
            match frame_at(&self.buf) {
                Frame::Valid(total) => {
                    out.extend(Answer::parse(&self.buf[..total]));
                    self.buf.drain(..total);
                }
                Frame::Invalid => {
                    self.buf.remove(0);
                }
                Frame::Incomplete => {
                    // A stray 5A may promise more than ever comes: on to a
                    // whole packet behind it, if one is there already
                    let next = (1..self.buf.len()).find(|&i| matches!(frame_at(&self.buf[i..]), Frame::Valid(_)));
                    let Some(next) = next else { return out };
                    self.buf.drain(..next);
                }
            }
        }
    }
}

enum Frame {
    /// A packet of this many bytes
    Valid(usize),
    Invalid,
    Incomplete,
}

/// What starts at the first byte of `buf`
fn frame_at(buf: &[u8]) -> Frame {
    let [0x5A, high, low, ..] = *buf else {
        return if buf.first().is_some_and(|&b| b != 0x5A) { Frame::Invalid } else { Frame::Incomplete };
    };
    let length = usize::from(u16::from_be_bytes([high, low]));
    let total = length + 5;
    if !(3..=MAX_LENGTH).contains(&length) {
        return Frame::Invalid;
    }
    let Some(frame) = buf.get(..total) else { return Frame::Incomplete };
    let (body, crc) = frame.split_at(total - 2);
    if crc16(body).to_be_bytes() == crc { Frame::Valid(total) } else { Frame::Invalid }
}

/// What a read came to
enum Read {
    Value(u8),
    NoAnswer,
    /// The channel closed: the earbuds went away, or restart
    Closed,
}

/// The serial port channel to the earbuds; closed when dropped
struct Link<'a> {
    host: &'a BtHost,
    id: u32,
    events: mpsc::Receiver<RfcommEvent>,
    packets: Packets,
    closed: bool,
}

impl<'a> Link<'a> {
    fn open(host: &'a BtHost, addr: [u8; 6]) -> Option<Link<'a>> {
        let uuid = bt_host::parse_uuid(SERIAL_PORT_UUID)?;
        let (tx, events) = mpsc::channel();
        let sink = Box::new(move |_, event| tx.send(event).is_ok());
        match host.rfcomm_open(addr, uuid, sink) {
            Ok(o) => Some(Link { host, id: o.id, events, packets: Packets::default(), closed: false }),
            Err(e) => {
                tracing::info!("Huawei: no serial port channel ({e:?}), the sound quality mode stays as it is");
                None
            }
        }
    }

    fn send(&self, data: Vec<u8>) -> bool {
        self.host.rfcomm_send(self.id, data).wait().is_ok()
    }

    /// The next packet of `command`'s param `kind`, until `timeout`
    fn wait_for(&mut self, command: u16, kind: u8, timeout: Duration) -> Read {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.events.recv_timeout(left) {
                Ok(RfcommEvent::Data(data)) => {
                    for answer in self.packets.feed(&data) {
                        tracing::debug!("Huawei: received {:04X} {:02X?}", answer.command, answer.params);
                        if answer.command == command
                            && let Some(v) = answer.byte(kind)
                        {
                            return Read::Value(v);
                        }
                    }
                }
                Ok(RfcommEvent::Closed(reason)) => {
                    tracing::info!("Huawei: serial port closed ({reason:?})");
                    self.closed = true;
                    return Read::Closed;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => return Read::NoAnswer,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.closed = true;
                    return Read::Closed;
                }
            }
        }
    }

    fn read(&mut self, request: Vec<u8>, command: u16, kind: u8) -> Read {
        if !self.send(request) {
            self.closed = true;
            return Read::Closed;
        }
        self.wait_for(command, kind, ANSWER_TIMEOUT)
    }

    /// Sound quality first, when the earbuds answer: the answer's param 2
    fn read_quality(&mut self) -> Read {
        match self.read(packet(QUALITY_READ, &[(1, &[])]), QUALITY_READ, 2) {
            Read::NoAnswer => self.read(QUALITY_READ_CAPTURED.to_vec(), QUALITY_READ, 2),
            r => r,
        }
    }
}

impl Drop for Link<'_> {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.host.rfcomm_close(self.id).wait();
        }
    }
}

/// Sets the connected earbuds to sound quality first. True when they have
/// that mode and took the write: LDAC can then come with a new connection.
/// The mode is written also when they report it on already, set from a
/// phone: FreeBuds set so did not list LDAC to A2DPWB (issue #30). Nothing
/// is written to earbuds that do not answer the read.
pub fn ask_for_ldac(host: &BtHost, addr: [u8; 6]) -> bool {
    let Some(mut link) = Link::open(host, addr) else { return false };
    let before = match link.read_quality() {
        Read::Value(v) => v,
        Read::NoAnswer => {
            tracing::info!("Huawei: these earbuds report no sound quality mode");
            return false;
        }
        Read::Closed => return false,
    };
    // Only for the log: whether a report of no LDAC may come from it
    let dual = match link.read(packet(DUAL_CONNECT_READ, &[(1, &[])]), DUAL_CONNECT_READ, 1) {
        Read::Value(v) => format!("{v}"),
        Read::NoAnswer => "not reported".into(),
        Read::Closed => return false,
    };
    tracing::info!("Huawei: sound quality first is {before}, dual connection {dual}; setting sound quality first");
    if !link.send(packet(QUALITY_WRITE, &[(1, &[1])])) {
        tracing::info!("Huawei: the sound quality mode could not be sent");
        return false;
    }
    match link.wait_for(QUALITY_READ, 2, WRITE_SETTLE) {
        Read::Closed => return true,
        Read::Value(_) | Read::NoAnswer => {}
    }
    match link.read_quality() {
        Read::Value(1) | Read::NoAnswer | Read::Closed => {
            tracing::info!("Huawei: sound quality first set, connecting again");
            true
        }
        Read::Value(v) => {
            tracing::warn!("Huawei: the earbuds did not take sound quality first (they report {v})");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    // Packets captured from the official app (HFreebuds' ProtocolTest)
    #[test]
    fn packets_as_the_official_app_sends_them() {
        assert_eq!(packet(QUALITY_WRITE, &[(1, &[1])]), hex("5A0006002BA2010101B5EF"));
        assert_eq!(packet(0x2B10, &[(1, &[0])]), hex("5A0006002B10010100B977"));
        assert_eq!(packet(0x2B2A, &[(1, &[])]), hex("5A0005002B2A0100427E"));
        assert_eq!(packet(0x0108, &[(1, &[]), (2, &[]), (3, &[])]), hex("5A0009000108010002000300FBB9"));
    }

    #[test]
    fn the_captured_read_is_a_valid_packet() {
        let (body, crc) = QUALITY_READ_CAPTURED.split_at(8);
        assert_eq!(crc16(body).to_be_bytes(), crc);
    }

    #[test]
    fn packets_are_cut_from_the_stream() {
        let a = hex("5A0006002B10010101A956");
        let b = hex("5A0007002B0401020103D17F");
        let stream = [&[0x01, 0x5A, 0x00][..], &a, &b].concat();
        let mut p = Packets::default();
        let mut got = p.feed(&stream[..9]);
        got.extend(p.feed(&stream[9..]));
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].command, 0x2B10);
        assert_eq!(got[0].byte(1), Some(1));
        assert_eq!(got[1], Answer { command: 0x2B04, params: vec![(1, vec![1, 3])] });
    }

    #[test]
    fn a_bad_checksum_is_no_packet() {
        let mut a = hex("5A0006002B10010101A956");
        a[9] ^= 1;
        assert!(Packets::default().feed(&a).is_empty());
    }

    #[test]
    fn huawei_names() {
        for n in ["HUAWEI FreeBuds Pro 4", "Huawey FreeBuds Pro 4", "FreeClip", "HONOR Earbuds 2 Lite"] {
            assert!(is_huawei(n), "{n}");
        }
        for n in ["WH-1000XM4", "Honor's speaker", ""] {
            assert!(!is_huawei(n), "{n}");
        }
    }
}
