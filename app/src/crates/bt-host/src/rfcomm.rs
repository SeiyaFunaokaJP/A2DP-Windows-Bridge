//! RFCOMM client channels to a device the stack has an ACL link with, for
//! IPC clients that talk to the device's own service over RFCOMM (an
//! earbud app's control channel: battery, noise control, EQ). The stack
//! never connects or pairs for them: without a link there is no channel.
//!
//! An open resolves the service UUID to its RFCOMM server channel with an
//! SDP query, then opens the channel (or opens a server channel it is given,
//! for a service that SDP does not list); one open runs at a time, the others
//! wait. Data a channel receives goes to its [`RfcommSink`] on the BTstack
//! thread; data to send is queued per channel and sent frame by frame as
//! RFCOMM's credits allow.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use btstack_sys::consts::*;
use btstack_sys::{AclState, Stack};

use crate::events::{addr_str, event_type, le16, u8_at};

/// Channels open or opening at a time (all clients together)
const MAX_CHANNELS: usize = 8;
/// Bytes a channel queues for sending before a send is refused; also the
/// largest single send
pub const MAX_SEND_BYTES: usize = 64 * 1024;
/// An open is answered within this time from its request, also while it
/// waits behind other opens
pub const OPEN_DEADLINE: Duration = Duration::from_secs(20);
/// The deadline of the first open to end
pub(crate) const TIMER_RFCOMM: u32 = 6;

/// An open channel: its id (unique in this process, also across restarts
/// of the stack) and the largest frame it sends and receives
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RfcommOpened {
    pub id: u32,
    pub mtu: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RfcommError {
    /// The stack has no A2DP connection to the device (or is not running)
    NotConnected,
    /// The device's SDP lists no RFCOMM channel for the UUID
    ServiceNotFound,
    /// BTstack or the device refused (BTstack's status)
    Refused(u8),
    /// No open channel with this id (closed, or the stack restarted)
    NoChannel,
    /// Too many channels, the channel's send queue is full, or an open
    /// waited behind others until its deadline
    Busy,
    /// No answer in time: an open ran out of time (its channel is closed if
    /// it opens later); a send or close may still be taken
    NoAnswer,
}

/// Why a channel closed (not for a close of its own client)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RfcommCloseReason {
    /// The device closed it, or the link to the device went down
    Remote,
    /// The A2DP connection to the device ended (the stream was stopped),
    /// together with the link
    Disconnected,
    /// The Bluetooth stack stopped (restart, adapter removed, core exit)
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RfcommEvent {
    Data(Vec<u8>),
    /// The last event of the channel
    Closed(RfcommCloseReason),
}

/// Receives a channel's events with its id, on the BTstack thread: it must
/// not block. The first may come before [`crate::BtHost::rfcomm_open`]
/// has returned. False after a [`RfcommEvent::Data`]: close the channel
/// (no event follows).
pub type RfcommSink = Box<dyn FnMut(u32, RfcommEvent) -> bool + Send>;

static NEXT_ID: AtomicU32 = AtomicU32::new(1);

pub(crate) type OpenReply = mpsc::SyncSender<Result<RfcommOpened, RfcommError>>;

/// The outcome of [`crate::BtHost::rfcomm_send`] / `rfcomm_close`, which
/// return before the BTstack thread has taken them
pub struct RfcommReply(pub(crate) mpsc::Receiver<Result<(), RfcommError>>);

impl RfcommReply {
    pub fn wait(self) -> Result<(), RfcommError> {
        match self.0.recv_timeout(crate::JOB_TIMEOUT) {
            Ok(r) => r,
            Err(mpsc::RecvTimeoutError::Timeout) => Err(RfcommError::NoAnswer),
            // The stack stopped: its channels are gone
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(RfcommError::NoChannel),
        }
    }
}

/// What an open connects to
#[derive(Debug, Clone, Copy)]
pub(crate) enum Service {
    /// The service of this UUID, by the device's SDP
    Uuid([u8; 16]),
    /// This server channel
    Channel(u8),
}

impl std::fmt::Display for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Service::Uuid(uuid) => write!(f, "service {}", uuid_str(uuid)),
            Service::Channel(channel) => write!(f, "server channel {channel}"),
        }
    }
}

enum Stage {
    /// Behind the open that runs
    Waiting,
    /// SDP query sent; the first server channel it lists
    Sdp(Option<u8>),
    /// rfcomm_create_channel sent
    Connecting(u16),
}

struct Open {
    id: u32,
    addr: [u8; 6],
    service: Service,
    sink: RfcommSink,
    /// None once answered before its end (deadline, A2DP connection gone):
    /// it then runs to its end and closes a channel that opens. Its
    /// receiver is also gone when the caller stopped waiting.
    reply: Option<OpenReply>,
    deadline: Instant,
    stage: Stage,
}

impl Open {
    /// Answers it (once)
    fn answer(&mut self, r: Result<RfcommOpened, RfcommError>) -> bool {
        self.reply.take().is_some_and(|reply| reply.send(r).is_ok())
    }
}

struct Channel {
    id: u32,
    cid: u16,
    addr: [u8; 6],
    mtu: u16,
    sink: RfcommSink,
    queue: VecDeque<Vec<u8>>,
    queued_bytes: usize,
    can_send_asked: bool,
    /// Its client closed it: it sends what is queued, then disconnects, and
    /// its sink gets nothing more
    closing: bool,
}

/// The channels and opens (BTstack thread only)
#[derive(Default)]
pub(crate) struct Rfcomm {
    opens: VecDeque<Open>,
    channels: Vec<Channel>,
}

impl Rfcomm {
    pub fn open(&mut self, stack: &Stack, addr: [u8; 6], service: Service, sink: RfcommSink, reply: OpenReply) {
        if !matches!(stack.acl_state(&addr), AclState::Open(_)) {
            tracing::info!("RFCOMM: no link to {}, not opening", addr_str(&addr));
            let _ = reply.send(Err(RfcommError::NotConnected));
            return;
        }
        if self.opens.len() + self.channels.len() >= MAX_CHANNELS {
            tracing::info!("RFCOMM: {MAX_CHANNELS} channels open or opening, not opening another");
            let _ = reply.send(Err(RfcommError::Busy));
            return;
        }
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        tracing::info!("RFCOMM: opening #{id} to {} for {service}", addr_str(&addr));
        let deadline = Instant::now() + OPEN_DEADLINE;
        self.opens.push_back(Open { id, addr, service, sink, reply: Some(reply), deadline, stage: Stage::Waiting });
        self.next(stack);
    }

    /// Starts the first open if none runs, and the timer of the next
    /// deadline
    fn next(&mut self, stack: &Stack) {
        while let Some(open) = self.opens.front_mut() {
            if !matches!(open.stage, Stage::Waiting) {
                break;
            }
            stack.rfcomm_init();
            let started = match open.service {
                Service::Uuid(uuid) => match stack.sdp_query_rfcomm(&open.addr, &uuid) {
                    ERROR_CODE_SUCCESS => Ok(Stage::Sdp(None)),
                    status => Err(status),
                },
                Service::Channel(channel) => stack.rfcomm_create_channel(&open.addr, channel).map(Stage::Connecting),
            };
            match started {
                Ok(stage) => {
                    open.stage = stage;
                    break;
                }
                Err(status) => {
                    self.end_first(stack, Err(RfcommError::Refused(status)));
                }
            }
        }
        self.arm_timer(stack);
    }

    fn arm_timer(&self, stack: &Stack) {
        match self.opens.iter().filter(|o| o.reply.is_some()).map(|o| o.deadline).min() {
            Some(d) => {
                let ms = d.saturating_duration_since(Instant::now()).as_millis().max(1);
                stack.timer_start(TIMER_RFCOMM, ms.min(u32::MAX as u128) as u32);
            }
            None => stack.timer_stop(TIMER_RFCOMM),
        }
    }

    /// TIMER_RFCOMM: opens past their deadline are answered. One that waits
    /// is dropped (busy); the one that runs goes on to its end, and its
    /// channel is closed if it opens.
    pub fn on_timer(&mut self, stack: &Stack) {
        let now = Instant::now();
        self.opens.retain_mut(|o| {
            if o.reply.is_none() || o.deadline > now {
                return true;
            }
            if matches!(o.stage, Stage::Waiting) {
                tracing::info!("RFCOMM: #{} waited behind other opens until its deadline", o.id);
                o.answer(Err(RfcommError::Busy));
                return false;
            }
            tracing::info!("RFCOMM: #{} not open by its deadline", o.id);
            o.answer(Err(RfcommError::NoAnswer));
            true
        });
        self.next(stack);
    }

    /// Ends the open that runs (the first): answers it if it was not
    /// answered yet, else closes a channel it opened
    fn end_first(&mut self, stack: &Stack, r: Result<(RfcommOpened, u16), RfcommError>) -> Option<Open> {
        let mut open = self.opens.pop_front()?;
        match r {
            Ok((opened, cid)) => {
                if open.answer(Ok(opened)) {
                    return Some(open);
                }
                tracing::info!("RFCOMM: #{} opened after it was answered, closing it", open.id);
                stack.rfcomm_disconnect(cid);
            }
            Err(e) => {
                tracing::info!("RFCOMM: #{} not opened: {e:?}", open.id);
                open.answer(Err(e));
            }
        }
        None
    }

    /// Source::SdpClient
    pub fn on_sdp(&mut self, stack: &Stack, p: &[u8]) {
        let Some(open) = self.opens.front_mut() else { return };
        let Stage::Sdp(found) = &mut open.stage else { return };
        match event_type(p) {
            Some(SDP_EVENT_QUERY_RFCOMM_SERVICE) => {
                let channel = u8_at(p, 2);
                tracing::debug!("RFCOMM: #{} SDP lists server channel {channel:?}", open.id);
                if found.is_none() {
                    *found = channel;
                }
            }
            Some(SDP_EVENT_QUERY_COMPLETE) => {
                let status = u8_at(p, 2).unwrap_or(ERROR_CODE_UNSPECIFIED_ERROR);
                let found = *found;
                tracing::debug!("RFCOMM: #{} SDP query complete (0x{status:02X}), server channel {found:?}", open.id);
                let result = match (status, found) {
                    // Answered already: no channel for it
                    _ if open.reply.is_none() => Err(RfcommError::NoAnswer),
                    (ERROR_CODE_SUCCESS, Some(server_channel)) => stack
                        .rfcomm_create_channel(&open.addr, server_channel)
                        .map(|cid| open.stage = Stage::Connecting(cid))
                        .map_err(RfcommError::Refused),
                    (ERROR_CODE_SUCCESS, None) => Err(RfcommError::ServiceNotFound),
                    _ if !matches!(stack.acl_state(&open.addr), AclState::Open(_)) => Err(RfcommError::NotConnected),
                    _ => Err(RfcommError::Refused(status)),
                };
                if let Err(e) = result {
                    self.end_first(stack, Err(e));
                    self.next(stack);
                }
            }
            _ => {}
        }
    }

    /// Source::Rfcomm: channel events, and data (packet type
    /// RFCOMM_DATA_PACKET, `channel` its RFCOMM cid)
    pub fn on_packet(&mut self, stack: &Stack, packet_type: u8, channel: u16, p: &[u8]) {
        if packet_type == RFCOMM_DATA_PACKET {
            let Some(i) = self.channels.iter().position(|c| c.cid == channel) else {
                tracing::debug!("RFCOMM: {} bytes for unknown cid 0x{channel:04X}", p.len());
                return;
            };
            let c = &mut self.channels[i];
            if c.closing {
                return;
            }
            tracing::trace!("RFCOMM: #{} received {} bytes", c.id, p.len());
            if !(c.sink)(c.id, RfcommEvent::Data(p.to_vec())) {
                let c = self.channels.remove(i);
                tracing::info!("RFCOMM: #{} closed, its client does not take its data", c.id);
                stack.rfcomm_disconnect(c.cid);
            }
            return;
        }
        if packet_type != HCI_EVENT_PACKET {
            return;
        }
        match event_type(p) {
            Some(RFCOMM_EVENT_CHANNEL_OPENED) => self.on_opened(stack, p),
            Some(RFCOMM_EVENT_CHANNEL_CLOSED) => {
                let Some(cid) = le16(p, 2) else { return };
                if let Some(i) = self.channels.iter().position(|c| c.cid == cid) {
                    let mut c = self.channels.remove(i);
                    if c.closing {
                        tracing::info!("RFCOMM: #{} closed before its queue was sent", c.id);
                    } else {
                        tracing::info!("RFCOMM: #{} closed by the device or the link", c.id);
                        (c.sink)(c.id, RfcommEvent::Closed(RfcommCloseReason::Remote));
                    }
                }
            }
            Some(RFCOMM_EVENT_CAN_SEND_NOW) => {
                let Some(cid) = le16(p, 2) else { return };
                if let Some(i) = self.channels.iter().position(|c| c.cid == cid) {
                    let c = &mut self.channels[i];
                    c.can_send_asked = false;
                    c.send_next(stack);
                    if c.closing && c.queue.is_empty() {
                        let c = self.channels.remove(i);
                        tracing::info!("RFCOMM: #{} sent its queue, closing", c.id);
                        stack.rfcomm_disconnect(c.cid);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_opened(&mut self, stack: &Stack, p: &[u8]) {
        let (Some(status), Some(cid), Some(mtu)) = (u8_at(p, 2), le16(p, 12), le16(p, 14)) else { return };
        let first = self.opens.front().is_some_and(|o| matches!(o.stage, Stage::Connecting(c) if c == cid));
        if !first {
            tracing::debug!("RFCOMM: channel opened event for cid 0x{cid:04X} of no open (0x{status:02X})");
            return;
        }
        let id = self.opens.front().map_or(0, |o| o.id);
        let r = if status == ERROR_CODE_SUCCESS {
            Ok((RfcommOpened { id, mtu }, cid))
        } else {
            tracing::info!("RFCOMM: #{id} refused by the device (0x{status:02X})");
            Err(RfcommError::Refused(status))
        };
        if let Some(open) = self.end_first(stack, r) {
            tracing::info!("RFCOMM: #{id} open (cid 0x{cid:04X}, frames up to {mtu} bytes)");
            self.channels.push(Channel {
                id,
                cid,
                addr: open.addr,
                mtu: mtu.max(1),
                sink: open.sink,
                queue: VecDeque::new(),
                queued_bytes: 0,
                can_send_asked: false,
                closing: false,
            });
        }
        self.next(stack);
    }

    pub fn send(&mut self, stack: &Stack, id: u32, data: &[u8]) -> Result<(), RfcommError> {
        let c = self.channels.iter_mut().find(|c| c.id == id && !c.closing).ok_or(RfcommError::NoChannel)?;
        if c.queued_bytes + data.len() > MAX_SEND_BYTES {
            tracing::info!("RFCOMM: #{id} send queue full ({} bytes), {} bytes refused", c.queued_bytes, data.len());
            return Err(RfcommError::Busy);
        }
        for frame in data.chunks(c.mtu as usize) {
            c.queue.push_back(frame.to_vec());
            c.queued_bytes += frame.len();
        }
        c.send_next(stack);
        Ok(())
    }

    /// A close of the channel's own client, once what it queued is sent:
    /// no event follows
    pub fn close(&mut self, stack: &Stack, id: u32) -> Result<(), RfcommError> {
        let i = self.channels.iter().position(|c| c.id == id && !c.closing).ok_or(RfcommError::NoChannel)?;
        if self.channels[i].queue.is_empty() {
            let c = self.channels.remove(i);
            tracing::info!("RFCOMM: #{id} closed by its client");
            stack.rfcomm_disconnect(c.cid);
        } else {
            let c = &mut self.channels[i];
            tracing::info!("RFCOMM: #{id} closed by its client, after its {} queued bytes", c.queued_bytes);
            c.closing = true;
        }
        Ok(())
    }

    /// The A2DP connection to `addr` ended: its channels and opens go with
    /// it, so that the link goes down as it does without them. An open in
    /// progress is answered now and closes its channel if one opens.
    pub fn close_device(&mut self, stack: &Stack, addr: [u8; 6]) {
        let (gone, kept) = std::mem::take(&mut self.channels).into_iter().partition(|c| c.addr == addr);
        self.channels = kept;
        for mut c in gone {
            tracing::info!("RFCOMM: #{} closed with the A2DP connection", c.id);
            stack.rfcomm_disconnect(c.cid);
            if !c.closing {
                (c.sink)(c.id, RfcommEvent::Closed(RfcommCloseReason::Disconnected));
            }
        }
        self.opens.retain_mut(|o| {
            if o.addr != addr {
                return true;
            }
            tracing::info!("RFCOMM: #{} not opened, the A2DP connection ended", o.id);
            o.answer(Err(RfcommError::NotConnected));
            !matches!(o.stage, Stage::Waiting)
        });
        self.next(stack);
    }

    /// The run loop exits: every channel and open ends
    pub fn stop(&mut self) {
        for mut open in self.opens.drain(..) {
            open.answer(Err(RfcommError::NotConnected));
        }
        for mut c in self.channels.drain(..) {
            tracing::info!("RFCOMM: #{} closed, the stack stops", c.id);
            if !c.closing {
                (c.sink)(c.id, RfcommEvent::Closed(RfcommCloseReason::Stopped));
            }
        }
    }
}

impl Channel {
    /// Sends the queued frames while RFCOMM takes them, then asks for the
    /// moment it can take more
    fn send_next(&mut self, stack: &Stack) {
        if self.can_send_asked {
            return;
        }
        while let Some(frame) = self.queue.front() {
            let status = stack.rfcomm_send(self.cid, frame);
            if status != ERROR_CODE_SUCCESS {
                // No credits or buffers now: once it can
                tracing::trace!("RFCOMM: #{} send deferred (0x{status:02X})", self.id);
                break;
            }
            self.queued_bytes -= frame.len();
            self.queue.pop_front();
        }
        if !self.queue.is_empty() {
            self.can_send_asked = stack.rfcomm_request_can_send_now(self.cid) == ERROR_CODE_SUCCESS;
        }
    }
}

/// "0000xxxx-0000-1000-8000-00805F9B34FB"
fn uuid_str(u: &[u8; 16]) -> String {
    let h: String = u.iter().map(|b| format!("{b:02X}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// Parses a service UUID: 128-bit ("00001101-0000-1000-8000-00805F9B34FB",
/// with or without dashes or braces), or 16 / 32-bit ("1101", "0x1101") on
/// the Bluetooth base UUID. Big endian, as printed.
pub fn parse_uuid(s: &str) -> Option<[u8; 16]> {
    let s = s.trim().trim_start_matches('{').trim_end_matches('}');
    let hex: String = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s).replace('-', "");
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut u = [0u8; 16];
    match hex.len() {
        4 | 8 => {
            u = [0, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0x80, 0x5F, 0x9B, 0x34, 0xFB];
            let short = u32::from_str_radix(&hex, 16).ok()?;
            u[..4].copy_from_slice(&short.to_be_bytes());
        }
        32 => {
            for (i, b) in u.iter_mut().enumerate() {
                *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
            }
        }
        _ => return None,
    }
    Some(u)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuids() {
        let spp = [0, 0, 0x11, 0x01, 0, 0, 0x10, 0, 0x80, 0, 0, 0x80, 0x5F, 0x9B, 0x34, 0xFB];
        assert_eq!(parse_uuid("1101"), Some(spp));
        assert_eq!(parse_uuid("0x1101"), Some(spp));
        assert_eq!(parse_uuid("00001101"), Some(spp));
        assert_eq!(parse_uuid("00001101-0000-1000-8000-00805f9b34fb"), Some(spp));
        assert_eq!(parse_uuid("{00001101-0000-1000-8000-00805F9B34FB}"), Some(spp));
        assert_eq!(parse_uuid("000011010000100080000080 5F9B34FB"), None);
        assert_eq!(parse_uuid("110"), None);
        assert_eq!(parse_uuid("g101"), None);
        assert_eq!(uuid_str(&spp), "00001101-0000-1000-8000-00805F9B34FB");
        let vendor = parse_uuid("0000079a-d102-11e1-9b23-00025b00a5a5").unwrap();
        assert_eq!(uuid_str(&vendor), "0000079A-D102-11E1-9B23-00025B00A5A5");
    }
}
