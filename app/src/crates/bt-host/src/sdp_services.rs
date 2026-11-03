//! The services a device lists in SDP (developer mode: the device's pairing
//! dialog): each record's service class UUIDs and name. Asked only over a
//! link the stack has; one query at a time.

use std::collections::BTreeMap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use btstack_sys::consts::*;
use btstack_sys::{AclState, Stack};

use crate::events::{addr_str, event_type, le16, u8_at};

/// A query is answered within this time (a late result is dropped)
pub const DEADLINE: Duration = Duration::from_secs(15);

const ATTR_SERVICE_CLASS_ID_LIST: u16 = 0x0001;
const ATTR_SERVICE_NAME: u16 = 0x0100;
/// An attribute value longer than this is cut (a name, or a list of UUIDs,
/// is far shorter)
const MAX_VALUE_BYTES: usize = 1024;

/// One SDP record: its service class UUIDs (the 128-bit form, lowercase)
/// and its name, if it has them
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SdpRecord {
    pub classes: Vec<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServicesError {
    /// No link to the device (or the stack is not running)
    NotConnected,
    /// Another query runs
    Busy,
    /// No answer in time, or the link went down meanwhile
    NoAnswer,
    /// BTstack or the device refused (BTstack's status)
    Failed(u8),
}

pub(crate) type ServicesReply = mpsc::SyncSender<Result<Vec<SdpRecord>, ServicesError>>;

struct Query {
    reply: ServicesReply,
    deadline: Instant,
    /// Attribute values by (record, attribute), as their bytes come
    values: BTreeMap<(u16, u16), Vec<u8>>,
}

/// The running query (BTstack thread only)
#[derive(Default)]
pub(crate) struct SdpServices {
    query: Option<Query>,
}

impl SdpServices {
    pub fn start(&mut self, stack: &Stack, addr: [u8; 6], reply: ServicesReply) {
        if !matches!(stack.acl_state(&addr), AclState::Open(_)) {
            let _ = reply.send(Err(ServicesError::NotConnected));
            return;
        }
        // One past its deadline never got its end: its result goes nowhere
        if self.query.as_ref().is_some_and(|q| Instant::now() < q.deadline) {
            let _ = reply.send(Err(ServicesError::Busy));
            return;
        }
        match stack.sdp_query_services(&addr) {
            ERROR_CODE_SUCCESS => {
                tracing::info!("SDP: asking {} for its services", addr_str(&addr));
                self.query = Some(Query { reply, deadline: Instant::now() + DEADLINE, values: BTreeMap::new() });
            }
            ERROR_CODE_COMMAND_DISALLOWED => {
                let _ = reply.send(Err(ServicesError::Busy));
            }
            status => {
                let _ = reply.send(Err(ServicesError::Failed(status)));
            }
        }
    }

    /// Source::SdpServices
    pub fn on_event(&mut self, p: &[u8]) {
        match event_type(p) {
            Some(SDP_EVENT_QUERY_ATTRIBUTE_VALUE) => {
                let Some(q) = self.query.as_mut() else { return };
                // Its length is 0 for the bytes of the value's own header
                // (BTstack knows it only after them): the offset places it
                let (Some(record), Some(attribute), Some(offset), Some(byte)) =
                    (le16(p, 2), le16(p, 4), le16(p, 8), u8_at(p, 10))
                else {
                    return;
                };
                let value = q.values.entry((record, attribute)).or_default();
                if usize::from(offset) == value.len() && value.len() < MAX_VALUE_BYTES {
                    value.push(byte);
                }
            }
            Some(SDP_EVENT_QUERY_COMPLETE) => {
                let Some(q) = self.query.take() else { return };
                let status = u8_at(p, 2).unwrap_or(ERROR_CODE_UNSPECIFIED_ERROR);
                let result = match status {
                    ERROR_CODE_SUCCESS => Ok(records(&q.values)),
                    // The link went down meanwhile, or the page failed
                    ERROR_CODE_PAGE_TIMEOUT | ERROR_CODE_CONNECTION_TIMEOUT => Err(ServicesError::NoAnswer),
                    s => Err(ServicesError::Failed(s)),
                };
                tracing::info!("SDP: services query ended (0x{status:02X}): {:?}", result.as_ref().map(Vec::len));
                let _ = q.reply.send(result);
            }
            _ => {}
        }
    }
}

fn records(values: &BTreeMap<(u16, u16), Vec<u8>>) -> Vec<SdpRecord> {
    let mut out: BTreeMap<u16, SdpRecord> = BTreeMap::new();
    for (&(record, attribute), value) in values {
        let r = out.entry(record).or_default();
        match attribute {
            ATTR_SERVICE_CLASS_ID_LIST => r.classes = uuids(value),
            ATTR_SERVICE_NAME => r.name = text(value),
            _ => {}
        }
    }
    out.into_values().filter(|r| !r.classes.is_empty() || r.name.is_some()).collect()
}

/// One data element: its type and its content
fn element(data: &[u8]) -> Option<(u8, &[u8], usize)> {
    let head = *data.first()?;
    let (kind, size) = (head >> 3, head & 0x07);
    let (len, skip): (usize, usize) = match size {
        0 if kind == 0 => (0, 1),
        0 => (1, 1),
        1 => (2, 1),
        2 => (4, 1),
        3 => (8, 1),
        4 => (16, 1),
        5 => (usize::from(*data.get(1)?), 2),
        6 => (usize::from(u16::from_be_bytes([*data.get(1)?, *data.get(2)?])), 3),
        _ => (u32::from_be_bytes([*data.get(1)?, *data.get(2)?, *data.get(3)?, *data.get(4)?]) as usize, 5),
    };
    let body = data.get(skip..skip.checked_add(len)?)?;
    Some((kind, body, skip + len))
}

const DE_UUID: u8 = 3;
const DE_TEXT: u8 = 4;
const DE_SEQUENCE: u8 = 6;

/// The UUIDs of a sequence (ServiceClassIDList)
fn uuids(value: &[u8]) -> Vec<String> {
    let Some((DE_SEQUENCE, mut rest, _)) = element(value) else { return Vec::new() };
    let mut out = Vec::new();
    while let Some((kind, body, used)) = element(rest) {
        if kind == DE_UUID
            && let Some(u) = uuid_string(body)
        {
            out.push(u);
        }
        rest = &rest[used..];
    }
    out
}

fn text(value: &[u8]) -> Option<String> {
    let (DE_TEXT, body, _) = element(value)? else { return None };
    // Some devices end it with a NUL
    let s = String::from_utf8_lossy(body).trim_end_matches('\0').trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// A 16-, 32- or 128-bit UUID in its 128-bit form
pub fn uuid_string(body: &[u8]) -> Option<String> {
    let full: [u8; 16] = match body.len() {
        2 | 4 => {
            let mut short = [0u8; 4];
            short[4 - body.len()..].copy_from_slice(body);
            let mut u = [0, 0, 0, 0, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB];
            u[..4].copy_from_slice(&short);
            u
        }
        16 => body.try_into().ok()?,
        _ => return None,
    };
    let h: String = full.iter().map(|b| format!("{b:02x}")).collect();
    Some(format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuids_take_every_size() {
        // DES of a 16-bit, a 32-bit and a 128-bit UUID
        let mut v = vec![0x35, 3 + 5 + 17, 0x19, 0x11, 0x0B, 0x1A, 0x00, 0x00, 0x11, 0x0E, 0x1C];
        v.extend([0xDF, 0x21, 0xFE, 0x2C, 0x25, 0x15, 0x4F, 0xDB, 0x88, 0x86, 0xF1, 0x2C, 0x4D, 0x67, 0x92, 0x7C]);
        assert_eq!(
            uuids(&v),
            [
                "0000110b-0000-1000-8000-00805f9b34fb",
                "0000110e-0000-1000-8000-00805f9b34fb",
                "df21fe2c-2515-4fdb-8886-f12c4d67927c",
            ]
        );
    }

    #[test]
    fn records_pair_classes_and_names() {
        let mut values = BTreeMap::new();
        values.insert((0, ATTR_SERVICE_CLASS_ID_LIST), vec![0x35, 0x03, 0x19, 0x11, 0x0B]);
        values.insert((0, ATTR_SERVICE_NAME), b"\x25\x0AAudio Sink".to_vec());
        values.insert((1, ATTR_SERVICE_CLASS_ID_LIST), vec![0x35, 0x03, 0x19, 0x11, 0x0E]);
        let r = records(&values);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].name.as_deref(), Some("Audio Sink"));
        assert_eq!(r[1].classes, ["0000110e-0000-1000-8000-00805f9b34fb"]);
        assert_eq!(r[1].name, None);
    }

    #[test]
    fn broken_elements_give_nothing() {
        assert!(uuids(&[0x35, 0x05, 0x19, 0x11]).is_empty());
        assert!(uuids(&[0x19, 0x11, 0x0B]).is_empty());
        assert_eq!(text(&[0x25, 0x00]), None);
        assert_eq!(element(&[0x1E, 0x00, 0x10, 0xFF]), None);
    }
}
