//! OPPO Enco Free4 earbuds offered A2DPWB LHDC V5 at 48 kHz and 400 kbps while
//! a phone got 192 kHz (issue #237). Their app sets Hi-Res over the HeyMelody
//! protocol on their own RFCOMM service. A stream that wants more of LHDC V5
//! sets it the same way, once per earbuds. They list the new limit only on a
//! new connection, so one is made for it; earbuds that had the mode off get it
//! back when it changed nothing.
//!
//! The protocol as LHDC-Switch uses it (MIT, THIRD_PARTY_LICENSES.md). A packet
//! is `AA | length (LEB128) | 00 00 | command (2 bytes, little endian) |
//! sequence | payload length (2 bytes, little endian) | payload`; the length
//! counts from the `00 00`. An answer has the command's top bit set and its
//! sequence, its payload starts with a status (0: fine). A long one comes in
//! pieces, `01` first and `03` last, each with 3 bytes before its data.
//!
//! LHDC-Switch writes only to the two models it was tried on. Here any earbuds
//! that report the write in their capabilities get it (issue #237: an Enco X4
//! with Hi-Res off was left out): a write that brings nothing is undone.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use bt_host::BtHost;

use super::channel::{Channel, Place, Received};

const PLACES: [Place; 3] = [
    Place::Service("0000079A-D102-11E1-9B23-00025B00A5A5"),
    Place::Service("00001107-D102-11E1-9B23-00025B00A5A5"),
    Place::Channel(15),
];

const START: u8 = 0xAA;
const ANSWER: u16 = 0x8000;
const CAPABILITIES_READ: u16 = 0x0100;
const PRODUCT_READ: u16 = 0x0103;
const FEATURES_READ: u16 = 0x010D;
const CODEC_READ: u16 = 0x0114;
const FEATURE_WRITE: u16 = 0x0403;
const CODEC_WRITE: u16 = 0x041A;

/// The features asked for: their count, then their ids
const FEATURES_ASKED: [u8; 9] = [8, 5, 4, 0x0B, DUAL_CONNECTION, HI_RES, 6, SPATIAL_AUDIO, 0x1C];
const DUAL_CONNECTION: u8 = 0x11;
const HI_RES: u8 = 0x18;
const SPATIAL_AUDIO: u8 = 0x1B;
/// The codec write that asks for LHDC with Hi-Res
const LHDC_HI_RES: [u8; 3] = [8, 1, 0];

// Bits of the capabilities read
const CAN_WRITE_FEATURES: usize = 7;
const CAN_READ_CODEC: usize = 17;
const CAN_WRITE_CODEC: usize = 37;

/// From opening the channel to the first read
const OPEN_SETTLE: Duration = Duration::from_millis(300);
/// Between requests
const REQUEST_GAP: Duration = Duration::from_millis(200);
/// How long an answer is waited for
const ANSWER_TIMEOUT: Duration = Duration::from_millis(3500);
/// Longest packet accepted (more: not a packet start)
const MAX_LENGTH: usize = 4096;

/// How the earbuds took a write
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taken {
    /// The connection stays
    Kept,
    /// They ended the link for it: a new connection is due
    LinkClosed,
}

/// Earbuds of this protocol, by their name
pub fn is_oppo(device_name: &str) -> bool {
    let name = device_name.to_lowercase();
    name.contains("oppo") || name.starts_with("enco ")
}

fn packet(command: u16, sequence: u8, payload: &[u8]) -> Vec<u8> {
    let mut p = vec![START];
    let mut length = 7 + payload.len();
    loop {
        let byte = (length & 0x7F) as u8;
        length >>= 7;
        if length == 0 {
            p.push(byte);
            break;
        }
        p.push(byte | 0x80);
    }
    p.extend([0, 0]);
    p.extend(command.to_le_bytes());
    p.push(sequence);
    p.extend((payload.len() as u16).to_le_bytes());
    p.extend(payload);
    p
}

/// A packet read
#[derive(Debug, PartialEq, Eq)]
struct Answer {
    command: u16,
    sequence: u8,
    payload: Vec<u8>,
}

impl Answer {
    /// From a whole message, its pieces joined
    fn parse(message: &[u8]) -> Option<Answer> {
        let [low, high, sequence, length_low, length_high, payload @ ..] = message else { return None };
        let length = usize::from(u16::from_le_bytes([*length_low, *length_high]));
        Some(Answer {
            command: u16::from_le_bytes([*low, *high]),
            sequence: *sequence,
            payload: payload.get(..length)?.to_vec(),
        })
    }
}

/// Cuts the bytes received into packets
#[derive(Default)]
struct Packets {
    buf: Vec<u8>,
    /// The pieces of a long message so far
    pieces: Vec<u8>,
}

impl Packets {
    fn feed(&mut self, data: &[u8]) -> Vec<Answer> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        loop {
            let Some(start) = self.buf.iter().position(|&b| b == START) else {
                self.buf.clear();
                return out;
            };
            self.buf.drain(..start);
            let Some((length, header)) = length_at(&self.buf) else { return out };
            if !(2..=MAX_LENGTH).contains(&length) {
                self.buf.remove(0);
                continue;
            }
            if self.buf.len() < header + length {
                return out;
            }
            let body: Vec<u8> = self.buf.drain(..header + length).skip(header).collect();
            out.extend(self.message(&body).and_then(|m| Answer::parse(&m)));
        }
    }

    /// The message a packet ends, if it ends one
    fn message(&mut self, body: &[u8]) -> Option<Vec<u8>> {
        match body[0] {
            0 => return Some(body[2..].to_vec()),
            1 => self.pieces.clear(),
            _ => {}
        }
        self.pieces.extend_from_slice(body.get(3..)?);
        (body[0] == 3).then(|| std::mem::take(&mut self.pieces))
    }
}

/// The length after the start byte, and how many bytes come before the body.
/// None: not all of it is here yet.
fn length_at(buf: &[u8]) -> Option<(usize, usize)> {
    let mut length = 0;
    for (i, &byte) in buf.iter().enumerate().skip(1).take(3) {
        length |= usize::from(byte & 0x7F) << (7 * (i - 1));
        if byte & 0x80 == 0 {
            return Some((length, i + 1));
        }
    }
    // More than three bytes: no length of ours. Too long, so it is skipped.
    (buf.len() > 3).then_some((usize::MAX, 0))
}

/// Bit `n` of the capabilities read
fn capability(caps: &[u8], n: usize) -> bool {
    caps.get(1 + n / 8).is_some_and(|byte| byte & (1 << (n % 8)) != 0)
}

/// The value of a feature in the features read
fn feature(features: &[u8], id: u8) -> Option<u8> {
    let [0, count, pairs @ ..] = features else { return None };
    pairs.as_chunks::<2>().0.iter().take(usize::from(*count)).find(|pair| pair[0] == id).map(|pair| pair[1])
}

/// The earbuds as they report themselves
struct State {
    capabilities: Vec<u8>,
    hi_res: Option<u8>,
    spatial_audio: Option<u8>,
}

/// What a request came to
enum Reply {
    Answer(Vec<u8>),
    NoAnswer,
    /// The channel closed: the earbuds went away, or restart
    Closed,
}

struct Link<'a> {
    channel: Channel<'a>,
    packets: Packets,
    /// Packets received and not looked at yet
    answers: VecDeque<Answer>,
    sequence: u8,
}

impl<'a> Link<'a> {
    fn open(host: &'a BtHost, addr: [u8; 6]) -> Option<Link<'a>> {
        let channel = Channel::open(host, addr, "OPPO", &PLACES)?;
        std::thread::sleep(OPEN_SETTLE);
        Some(Link { channel, packets: Packets::default(), answers: VecDeque::new(), sequence: 0 })
    }

    fn request(&mut self, command: u16, payload: &[u8]) -> Reply {
        self.sequence = self.sequence.wrapping_add(1);
        if !self.channel.send(packet(command, self.sequence, payload)) {
            return Reply::Closed;
        }
        let deadline = Instant::now() + ANSWER_TIMEOUT;
        loop {
            while let Some(answer) = self.answers.pop_front() {
                tracing::debug!("OPPO: received {:04X} {:02X?}", answer.command, answer.payload);
                if answer.command == command | ANSWER && answer.sequence == self.sequence {
                    return Reply::Answer(answer.payload);
                }
            }
            match self.channel.receive(deadline) {
                Received::Data(data) => self.answers.extend(self.packets.feed(&data)),
                Received::Timeout => return Reply::NoAnswer,
                Received::Closed => return Reply::Closed,
            }
        }
    }

    /// The answer to a read, when it gives one
    fn read(&mut self, command: u16, payload: &[u8]) -> Option<Vec<u8>> {
        let Reply::Answer(answer) = self.request(command, payload) else { return None };
        std::thread::sleep(REQUEST_GAP);
        Some(answer)
    }

    fn state(&mut self) -> Option<State> {
        let capabilities = self.read(CAPABILITIES_READ, &[]).filter(|c| c.first() == Some(&0))?;
        let product = match self.read(PRODUCT_READ, &[])?.as_slice() {
            [0, low, middle, high] => u32::from_le_bytes([*low, *middle, *high, 0]),
            _ => return None,
        };
        let features = self.read(FEATURES_READ, &FEATURES_ASKED)?;
        let codec = if capability(&capabilities, CAN_READ_CODEC) {
            self.read(CODEC_READ, &[]).filter(|c| c.first() == Some(&0)).and_then(|c| c.get(1).copied())
        } else {
            None
        };
        tracing::info!(
            "OPPO: product {product:06X}, Hi-Res {:?}, spatial audio {:?}, dual connection {:?}, codec {codec:?}",
            feature(&features, HI_RES),
            feature(&features, SPATIAL_AUDIO),
            feature(&features, DUAL_CONNECTION),
        );
        tracing::debug!("OPPO: capabilities {capabilities:02X?}, features {features:02X?}");
        Some(State {
            hi_res: feature(&features, HI_RES),
            spatial_audio: feature(&features, SPATIAL_AUDIO),
            capabilities,
        })
    }

    /// How the earbuds took the write; None when they did not
    fn write(&mut self, command: u16, payload: &[u8]) -> Option<Taken> {
        match self.request(command, payload) {
            Reply::Answer(answer) if answer.first() == Some(&0) => {
                tracing::info!("OPPO: write {command:04X} {payload:02X?} taken");
                Some(Taken::Kept)
            }
            Reply::Answer(answer) => {
                tracing::warn!("OPPO: write {command:04X} refused ({:02X?})", answer.first());
                None
            }
            Reply::NoAnswer => {
                tracing::info!("OPPO: no answer to write {command:04X}");
                None
            }
            Reply::Closed => Some(Taken::LinkClosed),
        }
    }
}

/// Asks the connected earbuds for LHDC with Hi-Res. The mode they were in (0:
/// off) and how they took the write: more of LHDC V5 can then come with a new
/// connection. The write goes out also when they report Hi-Res on already, set
/// from a phone: it is that mode and no more that A2DPWB was offered (issue
/// #237). Spatial audio excludes Hi-Res and is the user's: then nothing is
/// written.
pub fn ask_for_hi_res(host: &BtHost, addr: [u8; 6]) -> Option<(u8, Taken)> {
    let mut link = Link::open(host, addr)?;
    let state = link.state()?;
    let (true, Some(before)) = (capability(&state.capabilities, CAN_WRITE_FEATURES), state.hi_res) else {
        tracing::info!("OPPO: these earbuds report no Hi-Res mode");
        return None;
    };
    if state.spatial_audio == Some(1) {
        tracing::info!("OPPO: spatial audio is on, Hi-Res stays as it is");
        return None;
    }
    let taken = if capability(&state.capabilities, CAN_WRITE_CODEC) {
        link.write(CODEC_WRITE, &LHDC_HI_RES)
    } else {
        link.write(FEATURE_WRITE, &[HI_RES, 1])
    };
    taken.map(|t| (before, t))
}

/// Sets the connected earbuds' Hi-Res back off; None when they did not take it
pub fn set_hi_res_off(host: &BtHost, addr: [u8; 6]) -> Option<Taken> {
    Link::open(host, addr)?.write(FEATURE_WRITE, &[HI_RES, 0])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    // The packet of LHDC-Switch's self test
    #[test]
    fn packets_as_the_earbuds_expect_them() {
        assert_eq!(packet(CAPABILITIES_READ, 1, &[]), hex("AA0700000001010000"));
        assert_eq!(packet(FEATURE_WRITE, 2, &[HI_RES, 1]), hex("AA09000003040202001801"));
    }

    #[test]
    fn the_length_is_leb128() {
        assert_eq!(&packet(FEATURES_READ, 1, &[0; 200])[..3], &[START, 0xCF, 0x01]);
    }

    #[test]
    fn packets_are_cut_from_the_stream() {
        let a = packet(CAPABILITIES_READ | ANSWER, 1, &[0, 0x80]);
        let b = packet(PRODUCT_READ | ANSWER, 2, &[0, 0x10, 0x8C, 0x06]);
        let stream = [&[0x01, 0x02][..], &a, &b].concat();
        let mut p = Packets::default();
        let mut got = p.feed(&stream[..5]);
        got.extend(p.feed(&stream[5..]));
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], Answer { command: 0x8100, sequence: 1, payload: vec![0, 0x80] });
        assert_eq!(got[1].payload, [0, 0x10, 0x8C, 0x06]);
    }

    #[test]
    fn a_long_answer_is_joined_from_its_pieces() {
        let message = [0x00, 0x81, 7, 3, 0, 0xA, 0xB, 0xC];
        let piece = |kind: u8, data: &[u8]| {
            let body = [&[kind, 0, 0][..], data].concat();
            [&[START, body.len() as u8][..], &body].concat()
        };
        let stream = [piece(1, &message[..4]), piece(2, &message[4..6]), piece(3, &message[6..])].concat();
        let got = Packets::default().feed(&stream);
        assert_eq!(got, [Answer { command: 0x8100, sequence: 7, payload: vec![0xA, 0xB, 0xC] }]);
    }

    #[test]
    fn a_stray_start_byte_does_not_hide_a_packet() {
        let a = packet(CAPABILITIES_READ | ANSWER, 1, &[0]);
        let stream = [&[START, 0xFF, 0xFF, 0xFF, 0x7F][..], &a].concat();
        let got = Packets::default().feed(&stream);
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn capabilities_and_features() {
        // Bits 7 and 17, of bytes 0 and 2 after the status
        let caps = [0, 0x80, 0, 0x02];
        assert!(capability(&caps, CAN_WRITE_FEATURES));
        assert!(capability(&caps, CAN_READ_CODEC));
        assert!(!capability(&caps, CAN_WRITE_CODEC));
        let features = [0, 2, HI_RES, 1, SPATIAL_AUDIO, 0];
        assert_eq!(feature(&features, HI_RES), Some(1));
        assert_eq!(feature(&features, SPATIAL_AUDIO), Some(0));
        assert_eq!(feature(&features, DUAL_CONNECTION), None);
        assert_eq!(feature(&[1, 2, HI_RES, 1], HI_RES), None);
    }

    #[test]
    fn oppo_names() {
        for n in ["OPPO Enco Free4", "oppo enco x3", "Enco Free4", "ENCO Air3"] {
            assert!(is_oppo(n), "{n}");
        }
        for n in ["Encore Plus", "WH-1000XM4", ""] {
            assert!(!is_oppo(n), "{n}");
        }
    }
}
