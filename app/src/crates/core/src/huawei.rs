//! Huawei / Honor earbuds list LDAC only in their "sound quality first" mode
//! (issue #30), which their app sets over the earbuds' own protocol on the
//! serial port service. A profile that asks for LDAC of such earbuds sets
//! it the same way: they then offer LDAC on the next connection. Earbuds that
//! had the mode off get it back when no LDAC came of it.
//!
//! The protocol as HFreebuds documents it, and its way of cutting the bytes
//! received into packets (N2K02, MIT, THIRD_PARTY_LICENSES.md). A packet is `5A | length (2 bytes, big endian)
//! | 00 | command (2 bytes) | params | CRC-16/XMODEM of all before it`; the
//! length counts from the `00` to the params, a param is `type | length |
//! value`. A read lists the types wanted with empty values and is answered
//! by the same command.

use std::collections::{BTreeMap, VecDeque};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use bt_host::{BtHost, RfcommError, RfcommEvent, RfcommOpened, RfcommSink};

const SERIAL_PORT_UUID: &str = "1101";
/// Where earbuds serve the protocol, for those whose SDP does not lead to it:
/// the FreeBuds Pro 4 serve it on 1, the FreeBuds 5i on 16
const SERIAL_PORT_CHANNELS: [u8; 2] = [1, 16];
const QUALITY_WRITE: u16 = 0x2BA2;
const QUALITY_READ: u16 = 0x2BA3;
const DUAL_CONNECT_READ: u16 = 0x2B2F;
const DUAL_CONNECT_LIST: u16 = 0x2B31;
const LOW_LATENCY: u16 = 0x2B6C;
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

    /// The first byte of a param
    fn first(&self, kind: u8) -> Option<u8> {
        self.params.iter().find(|(k, _)| *k == kind).and_then(|(_, v)| v.first().copied())
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
                Frame::BadChecksum(total) => {
                    tracing::debug!("Huawei: checksum of a packet wrong: {:02X?}", &self.buf[..total]);
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
    /// As long as its length says, with another checksum: this many bytes
    BadChecksum(usize),
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
    if crc16(body).to_be_bytes() == crc { Frame::Valid(total) } else { Frame::BadChecksum(total) }
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
    /// Packets received and not taken yet
    answers: VecDeque<Answer>,
    closed: bool,
}

impl<'a> Link<'a> {
    fn open(host: &'a BtHost, addr: [u8; 6]) -> Option<Link<'a>> {
        let uuid = bt_host::parse_uuid(SERIAL_PORT_UUID)?;
        let mut opened = Link::connect(host, |sink| host.rfcomm_open(addr, uuid, sink));
        for channel in SERIAL_PORT_CHANNELS {
            if !matches!(opened, Err(RfcommError::ServiceNotFound | RfcommError::Refused(_))) {
                break;
            }
            opened = Link::connect(host, |sink| host.rfcomm_open_channel(addr, channel, sink));
        }
        opened
            .inspect_err(|e| {
                tracing::info!("Huawei: no serial port channel ({e:?}), the sound quality mode stays as it is");
            })
            .ok()
    }

    fn connect(
        host: &'a BtHost,
        open: impl FnOnce(RfcommSink) -> Result<RfcommOpened, RfcommError>,
    ) -> Result<Link<'a>, RfcommError> {
        let (tx, events) = mpsc::channel();
        let opened = open(Box::new(move |_, event| tx.send(event).is_ok()))?;
        Ok(Link { host, id: opened.id, events, packets: Packets::default(), answers: VecDeque::new(), closed: false })
    }

    fn send(&self, data: Vec<u8>) -> bool {
        self.host.rfcomm_send(self.id, data).wait().is_ok()
    }

    /// The next packet, until `deadline`: else `NoAnswer` or `Closed`
    fn next(&mut self, deadline: Instant) -> Result<Answer, Read> {
        loop {
            if let Some(answer) = self.answers.pop_front() {
                // The dual connection list names other devices: not for a
                // log that users post
                if answer.command == DUAL_CONNECT_LIST {
                    tracing::debug!("Huawei: received {:04X}", answer.command);
                } else {
                    tracing::debug!("Huawei: received {:04X} {:02X?}", answer.command, answer.params);
                }
                return Ok(answer);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            match self.events.recv_timeout(left) {
                Ok(RfcommEvent::Data(data)) => self.answers.extend(self.packets.feed(&data)),
                Ok(RfcommEvent::Closed(reason)) => {
                    tracing::info!("Huawei: serial port closed ({reason:?})");
                    self.closed = true;
                    return Err(Read::Closed);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => return Err(Read::NoAnswer),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.closed = true;
                    return Err(Read::Closed);
                }
            }
        }
    }

    /// The next packet of `command`'s param `kind`, until `timeout`
    fn wait_for(&mut self, command: u16, kind: u8, timeout: Duration) -> Read {
        let deadline = Instant::now() + timeout;
        loop {
            match self.next(deadline) {
                Ok(answer) => {
                    if answer.command == command
                        && let Some(v) = answer.byte(kind)
                    {
                        return Read::Value(v);
                    }
                }
                Err(end) => return end,
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

    /// Sets sound quality first to `value`. True when the earbuds took it,
    /// also when they end the link for it.
    fn write_quality(&mut self, value: u8) -> bool {
        if !self.send(packet(QUALITY_WRITE, &[(1, &[value])])) {
            tracing::info!("Huawei: the sound quality mode could not be sent");
            return false;
        }
        match self.wait_for(QUALITY_READ, 2, WRITE_SETTLE) {
            Read::Closed => return true,
            Read::Value(_) | Read::NoAnswer => {}
        }
        match self.read_quality() {
            Read::Value(v) if v != value => {
                tracing::warn!("Huawei: the earbuds did not take sound quality first {value} (they report {v})");
                false
            }
            _ => {
                tracing::info!("Huawei: sound quality first set to {value}");
                true
            }
        }
    }

    /// A read for the log: its value, or how it came out. None: the channel
    /// closed.
    fn read_for_log(&mut self, command: u16, ask: u8, kind: u8) -> Option<String> {
        match self.read(packet(command, &[(ask, &[])]), command, kind) {
            Read::Value(v) => Some(v.to_string()),
            Read::NoAnswer => Some("not reported".into()),
            Read::Closed => None,
        }
    }

    /// The state of each device the earbuds know, for the log: above 0
    /// connected, 9 playing. Not their addresses or names. None: the channel
    /// closed.
    fn dual_connect_states(&mut self) -> Option<String> {
        if !self.send(packet(DUAL_CONNECT_LIST, &[(1, &[])])) {
            self.closed = true;
            return None;
        }
        let deadline = Instant::now() + ANSWER_TIMEOUT;
        let mut count = None;
        let mut states = BTreeMap::new();
        while count.is_none_or(|c| states.len() < usize::from(c)) {
            match self.next(deadline) {
                Ok(answer) if answer.command == DUAL_CONNECT_LIST => {
                    count = answer.byte(2).or(count);
                    if let (Some(index), Some(state)) = (answer.byte(3), answer.first(5)) {
                        states.insert(index, state);
                    }
                }
                Ok(_) => {}
                Err(Read::Closed) => return None,
                Err(_) => break,
            }
        }
        Some(format!("{:?}", states.values().collect::<Vec<_>>()))
    }

    /// What a report of no LDAC may come from. False: the channel closed.
    fn log_state(&mut self, quality: u8) -> bool {
        let Some(dual) = self.read_for_log(DUAL_CONNECT_READ, 1, 1) else { return false };
        let Some(latency) = self.read_for_log(LOW_LATENCY, 2, 2) else { return false };
        let Some(devices) = self.dual_connect_states() else { return false };
        tracing::info!(
            "Huawei: sound quality first is {quality}, dual connection {dual}, device states {devices}, \
             low latency {latency}"
        );
        true
    }
}

impl Drop for Link<'_> {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.host.rfcomm_close(self.id).wait();
        }
    }
}

/// Sets the connected earbuds to sound quality first. The mode they were in
/// (0: off) when they have that mode and took the write: LDAC can then come
/// with a new connection. The mode is written also when they report it on
/// already, set from a phone: FreeBuds set so did not list LDAC to BlueCaster
/// (issue #30). Nothing is written to earbuds that do not answer the read.
pub fn ask_for_ldac(host: &BtHost, addr: [u8; 6]) -> Option<u8> {
    let mut link = Link::open(host, addr)?;
    let before = match link.read_quality() {
        Read::Value(v) => v,
        Read::NoAnswer => {
            tracing::info!("Huawei: these earbuds report no sound quality mode");
            return None;
        }
        Read::Closed => return None,
    };
    if !link.log_state(before) {
        return None;
    }
    link.write_quality(1).then_some(before)
}

/// Sets the connected earbuds back to connection first, the mode that costs
/// them neither battery nor stability. True when a new connection is due:
/// the write may end the audio connection.
pub fn set_connection_first(host: &BtHost, addr: [u8; 6]) -> bool {
    Link::open(host, addr).is_some_and(|mut link| link.write_quality(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    // The packets of HFreebuds' ProtocolTest
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
    fn a_stray_5a_does_not_hide_a_packet() {
        let a = hex("5A0006002B10010101A956");
        let mut stray = vec![0x5A, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00];
        stray.extend(&a);
        let got = Packets::default().feed(&stray);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].byte(1), Some(1));
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
