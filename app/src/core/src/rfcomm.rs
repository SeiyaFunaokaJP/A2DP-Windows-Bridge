//! RFCOMM passthrough for IPC clients (rfcomm.open / send / close, #194):
//! the channels one connection opened over the stack's link to a device
//! (bt_host::rfcomm). Their data and closes go to that connection only, in
//! order with the answers to its rfcomm requests: both take the same queue
//! to the connection's writer.

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use bt_host::{RfcommCloseReason, RfcommError, RfcommEvent};
use ipc::{ErrorInfo, Event, Method, Msg, RfcommClosed, RfcommData, RfcommOpened, ServerMessage};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::state::{CoreState, bad_request, err};

/// Data of a connection's channels waiting for its writer: past this (a
/// client that stopped reading the pipe) a channel that receives more is
/// closed with reason "overflow". Control traffic stays far below it.
const MAX_PENDING_BYTES: usize = 1 << 20;

/// A message for the connection's writer; [`Channels::written`] once out
pub struct Queued {
    pub msg: ServerMessage,
    /// Data bytes it counts against [`MAX_PENDING_BYTES`]
    bytes: usize,
}

/// The rfcomm part of one connection
pub struct Channels {
    state: Arc<CoreState>,
    client: String,
    out: mpsc::UnboundedSender<Queued>,
    inner: Arc<Mutex<Inner>>,
    /// Bytes of rfcomm_data queued or held, not written yet
    pending: Arc<AtomicUsize>,
}

#[derive(Default)]
struct Inner {
    /// Open channels of this connection
    owned: HashSet<u32>,
    /// Channels its client closed: what still comes from them is dropped
    closed: HashSet<u32>,
    /// Events of channels whose open is not answered yet (they come from
    /// the BTstack thread as soon as the channel is open)
    held: Vec<(u32, Queued)>,
    /// The connection has ended: nothing goes out any more
    ended: bool,
}

impl Inner {
    /// To the writer, or held until the channel's open is answered
    fn put(&mut self, out: &mpsc::UnboundedSender<Queued>, id: u32, q: Queued) {
        if !self.owned.contains(&id) {
            self.held.push((id, q));
            return;
        }
        if matches!(&q.msg, ServerMessage::Event { event, .. } if matches!(**event, Event::RfcommClosed(_))) {
            self.owned.remove(&id);
        }
        let _ = out.send(q);
    }
}

fn event(state: &CoreState, event: Event, bytes: usize) -> Queued {
    Queued { msg: ServerMessage::Event { rev: state.rev(), event: Box::new(event) }, bytes }
}

fn closed(state: &CoreState, id: u32, reason: &str) -> Queued {
    event(state, Event::RfcommClosed(RfcommClosed { channel_id: id, reason: reason.into() }), 0)
}

impl Channels {
    pub fn new(state: Arc<CoreState>, client: String, out: mpsc::UnboundedSender<Queued>) -> Channels {
        Channels { state, client, out, inner: Arc::default(), pending: Arc::default() }
    }

    /// The writer sent `q`
    pub fn written(&self, q: &Queued) {
        self.pending.fetch_sub(q.bytes, Ordering::AcqRel);
    }

    /// Takes an rfcomm request (false: not one). A send or close is handed
    /// to the stack here, so that they keep the order of the requests; the
    /// answer follows on the queue.
    pub fn request(&self, id: u64, call: &Method) -> bool {
        let started = Instant::now();
        match call {
            Method::RfcommOpen { address, uuid } => self.open(id, address, uuid, started),
            Method::RfcommSend { channel_id, data } => {
                const METHOD: &str = "rfcomm.send";
                let reply = match BASE64.decode(data) {
                    Err(e) => Err(bad_request("rfcomm.bad_request", METHOD, format!("data: {e}"))),
                    Ok(data) if data.len() > bt_host::RFCOMM_MAX_SEND_BYTES => Err(bad_request(
                        "rfcomm.bad_request",
                        METHOD,
                        format!("{} bytes, over {}", data.len(), bt_host::RFCOMM_MAX_SEND_BYTES),
                    )),
                    Ok(data) => self.host_for(*channel_id).map(|h| h.rfcomm_send(*channel_id, data)),
                };
                self.answer_later(id, METHOD, started, reply);
            }
            Method::RfcommClose { channel_id } => {
                let reply = self.host_for(*channel_id).map(|h| {
                    let mut g = self.lock();
                    g.owned.remove(channel_id);
                    g.closed.insert(*channel_id);
                    drop(g);
                    h.rfcomm_close(*channel_id)
                });
                self.answer_later(id, "rfcomm.close", started, reply);
            }
            _ => return false,
        }
        true
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The stack, for a channel of this connection
    fn host_for(&self, channel_id: u32) -> Result<Arc<bt_host::BtHost>, ErrorInfo> {
        if !self.lock().owned.contains(&channel_id) {
            return Err(no_channel());
        }
        self.state.service.host().ok_or_else(no_channel)
    }

    fn answer(&self, id: u64, method: &str, started: Instant, r: Result<Value, ErrorInfo>) {
        answer(&self.out, &self.client, id, method, started, r)
    }

    fn open(&self, id: u64, address: &str, uuid: &str, started: Instant) {
        const METHOD: &str = "rfcomm.open";
        let Some(addr) = bt_host::parse_addr(address) else {
            let e = bad_request("rfcomm.bad_request", METHOD, format!("address \"{address}\""));
            return self.answer(id, METHOD, started, Err(e));
        };
        let Some(uuid) = bt_host::parse_uuid(uuid) else {
            let e = bad_request("rfcomm.bad_request", METHOD, format!("uuid \"{uuid}\""));
            return self.answer(id, METHOD, started, Err(e));
        };
        let Some(host) = self.state.service.host() else {
            return self.answer(id, METHOD, started, Err(open_error(RfcommError::NotConnected)));
        };
        let sink = {
            let (state, out, inner, pending) =
                (self.state.clone(), self.out.clone(), self.inner.clone(), self.pending.clone());
            Box::new(move |channel: u32, ev: RfcommEvent| -> bool {
                let mut g = inner.lock().unwrap_or_else(|e| e.into_inner());
                if g.ended || g.closed.contains(&channel) {
                    return true;
                }
                match ev {
                    RfcommEvent::Data(data) => {
                        let data = BASE64.encode(data);
                        let bytes = data.len();
                        if pending.load(Ordering::Acquire) + bytes > MAX_PENDING_BYTES {
                            tracing::warn!("RFCOMM: #{channel}: its client does not read its data, closing it");
                            g.closed.insert(channel);
                            g.put(&out, channel, closed(&state, channel, "overflow"));
                            return false;
                        }
                        pending.fetch_add(bytes, Ordering::AcqRel);
                        g.put(
                            &out,
                            channel,
                            event(&state, Event::RfcommData(RfcommData { channel_id: channel, data }), bytes),
                        );
                    }
                    RfcommEvent::Closed(reason) => {
                        let reason = match reason {
                            RfcommCloseReason::Remote => "remote",
                            RfcommCloseReason::Disconnected => "disconnected",
                            RfcommCloseReason::Stopped => "stopped",
                        };
                        g.put(&out, channel, closed(&state, channel, reason));
                    }
                }
                true
            })
        };
        let (out, inner, client) = (self.out.clone(), self.inner.clone(), self.client.clone());
        tokio::spawn(async move {
            let opened = {
                let host = host.clone();
                tokio::task::spawn_blocking(move || host.rfcomm_open(addr, uuid, sink))
                    .await
                    .unwrap_or(Err(RfcommError::NoAnswer))
            };
            let opened = match opened {
                Ok(o) => o,
                Err(e) => return answer(&out, &client, id, METHOD, started, Err(open_error(e))),
            };
            let mut g = inner.lock().unwrap_or_else(|e| e.into_inner());
            if g.ended {
                drop(g);
                tracing::debug!("IPC: {client} gone before RFCOMM #{} opened, closing it", opened.id);
                let _ = host.rfcomm_close(opened.id);
                return;
            }
            g.owned.insert(opened.id);
            let result = RfcommOpened { channel_id: opened.id, mtu: opened.mtu };
            answer(&out, &client, id, METHOD, started, Ok(serde_json::to_value(result).unwrap_or(Value::Null)));
            // What came before the answer, its rfcomm_closed included (which
            // ends the ownership again)
            let held = std::mem::take(&mut g.held);
            for (channel, q) in held {
                if channel == opened.id {
                    g.put(&out, channel, q);
                } else {
                    g.held.push((channel, q));
                }
            }
        });
    }

    /// Answers once the stack has taken the send or close
    fn answer_later(
        &self,
        id: u64,
        method: &'static str,
        started: Instant,
        reply: Result<bt_host::RfcommReply, ErrorInfo>,
    ) {
        let reply = match reply {
            Ok(r) => r,
            Err(e) => return self.answer(id, method, started, Err(e)),
        };
        let (out, client) = (self.out.clone(), self.client.clone());
        tokio::spawn(async move {
            let r = tokio::task::spawn_blocking(move || reply.wait()).await.unwrap_or(Err(RfcommError::NoAnswer));
            let r = r.map(|()| Value::Null).map_err(|e| match e {
                RfcommError::Busy => err("rfcomm.busy", "ipc.error.rfcomm_busy"),
                // Taken or not, unknown: not to be repeated
                RfcommError::NoAnswer => err("rfcomm.timeout", "ipc.error.rfcomm_timeout"),
                _ => no_channel(),
            });
            answer(&out, &client, id, method, started, r);
        });
    }
}

impl Drop for Channels {
    /// The connection ended: its channels close
    fn drop(&mut self) {
        let owned: Vec<u32> = {
            let mut g = self.lock();
            g.ended = true;
            g.held.clear();
            g.owned.drain().collect()
        };
        if owned.is_empty() {
            return;
        }
        tracing::info!("IPC: {} disconnected, closing its RFCOMM channels {owned:?}", self.client);
        if let Some(host) = self.state.service.host() {
            for id in owned {
                let _ = host.rfcomm_close(id);
            }
        }
    }
}

fn answer(
    out: &mpsc::UnboundedSender<Queued>,
    client: &str,
    id: u64,
    method: &str,
    started: Instant,
    r: Result<Value, ErrorInfo>,
) {
    let ms = started.elapsed().as_millis();
    let msg = match r {
        Ok(v) => {
            tracing::debug!("IPC: {client} #{id} {method} ok in {ms} ms");
            ServerMessage::Response { id, result: Some(v), error: None }
        }
        Err(e) => {
            tracing::info!("IPC: {client} #{id} {method} failed in {ms} ms: {} ({})", e.code, e.message.key);
            ServerMessage::Response { id, result: None, error: Some(e) }
        }
    };
    let _ = out.send(Queued { msg, bytes: 0 });
}

fn no_channel() -> ErrorInfo {
    err("rfcomm.no_channel", "ipc.error.rfcomm_no_channel")
}

fn open_error(e: RfcommError) -> ErrorInfo {
    match e {
        RfcommError::NotConnected => err("rfcomm.not_connected", "ipc.error.rfcomm_not_connected"),
        RfcommError::ServiceNotFound => err("rfcomm.service_not_found", "ipc.error.rfcomm_service_not_found"),
        RfcommError::Busy => err("rfcomm.busy", "ipc.error.rfcomm_busy"),
        RfcommError::Refused(status) => ErrorInfo {
            code: "rfcomm.open_failed".into(),
            message: Msg { key: "ipc.error.rfcomm_open_failed".into(), args: vec![format!("0x{status:02X}")] },
        },
        RfcommError::NoAnswer | RfcommError::NoChannel => ErrorInfo {
            code: "rfcomm.open_failed".into(),
            message: Msg { key: "ipc.error.rfcomm_open_failed".into(), args: vec!["timeout".into()] },
        },
    }
}
