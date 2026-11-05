//! The PC's ATT server (LE Audio): BTstack's read / write callbacks, answered
//! synchronously by the app, and the queue of notifications / indications.
//!
//! # Reads and writes
//!
//! BTstack calls the callbacks while it handles an ATT request, i.e. from
//! the HCI packet path ([`crate::transport`]'s pump), where the app is free:
//! they go straight to [`crate::App::on_att_read`] / [`crate::App::on_att_write`].
//! A request can also be handled from within one of the app's own calls
//! (BTstack processes a request it had to hold back as soon as the ATT
//! channel can send, e.g. when the app starts a GATT client query). Then the
//! app is busy, and the callback answers "pending" (ENABLE_ATT_DELAYED_RESPONSE):
//! once the app's call has returned, `att_server_response_ready` makes BTstack
//! handle the request again, and the callback reaches the app. Writes BTstack
//! does not answer (write commands, a bonded client's client configurations
//! restored on reconnection, execute / cancel of prepared writes) are kept
//! and delivered to the app then instead.
//!
//! A read is asked twice by BTstack (length, then value): the app is asked
//! once, its value is kept for the second call.
//!
//! # Notifications and indications
//!
//! [`crate::Stack::att_server_send`] queues them per connection and sends
//! each when BTstack can (att_server_request_to_send_*), cut to ATT_MTU - 3
//! (BTstack would send the full length with garbage). Without the app: the
//! queue is in btstack-sys.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::ffi::c_int;

use crate::callbacks::{Pending, guard};
use crate::consts::*;
use crate::{APP, PENDING, Stack, ffi};

/// att_db.h: a read callback's "answer later" and error encoding
const ATT_READ_RESPONSE_PENDING: u16 = 0xFFFF;
const ATT_READ_ERROR_CODE_OFFSET: u16 = 0xFE00;
/// bluetooth.h: a write callback's "answer later"
const ATT_ERROR_WRITE_RESPONSE_PENDING: c_int = 0x100;
const ERROR_CODE_COMMAND_DISALLOWED: u8 = 0x0C;

/// A write to the PC's GATT server ([`crate::App::on_att_write`]), as
/// BTstack's att_write_callback_t has it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttWriteRequest<'a> {
    pub con: u16,
    /// 0 for [`ATT_TRANSACTION_MODE_EXECUTE`] / `_CANCEL` / `_VALIDATE`
    pub handle: u16,
    /// [`ATT_TRANSACTION_MODE_NONE`] for a plain write; `_ACTIVE` for a
    /// prepared write (queue it), then `_VALIDATE`, `_EXECUTE` or `_CANCEL`
    /// for the whole queue
    pub mode: u16,
    pub offset: u16,
    pub value: &'a [u8],
}

struct ReadCache {
    con: u16,
    handle: u16,
    offset: u16,
    result: Result<Vec<u8>, u8>,
}

struct KeptWrite {
    handle: u16,
    mode: u16,
    offset: u16,
    value: Vec<u8>,
    /// "Pending" was answered: BTstack asks again after response_ready
    answered_pending: bool,
}

#[derive(Default)]
struct SendQueue {
    items: VecDeque<(u16, bool, Vec<u8>)>,
    /// A can-send-now request is registered with BTstack
    waiting: bool,
}

thread_local! {
    static READ_CACHE: RefCell<Option<ReadCache>> = const { RefCell::new(None) };
    static KEPT_WRITES: RefCell<HashMap<u16, Vec<KeptWrite>>> = RefCell::new(HashMap::new());
    /// Connections with an AttRetry in PENDING
    static RETRY_QUEUED: RefCell<Vec<u16>> = const { RefCell::new(Vec::new()) };
    static SEND: RefCell<HashMap<u16, SendQueue>> = RefCell::new(HashMap::new());
}

/// Clears what a previous run left
pub(crate) fn reset() {
    READ_CACHE.with(|c| c.borrow_mut().take());
    KEPT_WRITES.with(|w| w.borrow_mut().clear());
    RETRY_QUEUED.with(|r| r.borrow_mut().clear());
    SEND.with(|s| s.borrow_mut().clear());
}

/// Asks the app (if it is free: Err otherwise)
fn with_free_app<R>(f: impl FnOnce(&mut dyn crate::App, &Stack) -> R) -> Result<R, ()> {
    let stack = Stack::new();
    APP.with(|a| match a.try_borrow_mut() {
        Ok(mut slot) => match slot.as_mut() {
            Some(app) => Ok(f(app.as_mut(), &stack)),
            None => Err(()),
        },
        Err(_) => Err(()),
    })
}

fn queue_retry(con: u16) {
    let new = RETRY_QUEUED.with(|r| {
        let mut r = r.borrow_mut();
        if r.contains(&con) {
            false
        } else {
            r.push(con);
            true
        }
    });
    if new {
        PENDING.with(|p| p.borrow_mut().push_back(Pending::AttRetry(con)));
    }
}

/// The bytes of `value` from `offset` that fit (att_read_callback_handle_blob)
pub(crate) fn blob(value: &[u8], offset: u16, buffer_size: u16) -> &[u8] {
    let offset = offset as usize;
    if offset > value.len() {
        return &[];
    }
    let n = (value.len() - offset).min(buffer_size as usize);
    &value[offset..offset + n]
}

fn encode_read(result: &Result<Vec<u8>, u8>, offset: u16, buffer: *mut u8, buffer_size: u16) -> u16 {
    match result {
        Err(e) => ATT_READ_ERROR_CODE_OFFSET + *e as u16,
        // The length (below the error codes; values are at most 512 bytes)
        Ok(v) if buffer.is_null() => v.len().min(ATT_READ_ERROR_CODE_OFFSET as usize - 1) as u16,
        Ok(v) => {
            let part = blob(v, offset, buffer_size);
            // SAFETY: BTstack passes a buffer of buffer_size bytes; part is
            // at most that long.
            unsafe { std::ptr::copy_nonoverlapping(part.as_ptr(), buffer, part.len()) };
            part.len() as u16
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn a2dpwb_rs_att_read(con: u16, handle: u16, offset: u16, buffer: *mut u8, buffer_size: u16) -> u16 {
    guard(ATT_READ_ERROR_CODE_OFFSET + ATT_ERROR_UNLIKELY_ERROR as u16, || {
        if handle == ATT_READ_RESPONSE_PENDING {
            // BTstack tells that a read was answered "pending"
            return 0;
        }
        // The value asked for the length is the value to send
        if !buffer.is_null() {
            let cached = READ_CACHE.with(|c| {
                let mut c = c.borrow_mut();
                match c.as_ref() {
                    Some(e) if e.con == con && e.handle == handle && e.offset == offset => c.take(),
                    _ => None,
                }
            });
            if let Some(e) = cached {
                return encode_read(&e.result, offset, buffer, buffer_size);
            }
        }
        let Ok(result) = with_free_app(|app, s| app.on_att_read(s, con, handle, offset)) else {
            tracing::debug!(target: "le", "ATT: read of 0x{handle:04X} on 0x{con:04X} answered pending (app busy)");
            queue_retry(con);
            return ATT_READ_RESPONSE_PENDING;
        };
        let n = encode_read(&result, offset, buffer, buffer_size);
        tracing::trace!(target: "le", "ATT: read 0x{handle:04X} offset {offset} on 0x{con:04X}: {:?}", result.as_ref().map(|v| v.len()));
        if buffer.is_null() {
            READ_CACHE.with(|c| *c.borrow_mut() = Some(ReadCache { con, handle, offset, result }));
        }
        n
    })
}

#[unsafe(no_mangle)]
extern "C" fn a2dpwb_rs_att_write(
    con: u16,
    handle: u16,
    mode: u16,
    offset: u16,
    buffer: *const u8,
    size: u16,
) -> c_int {
    guard(ATT_ERROR_UNLIKELY_ERROR as c_int, || {
        let value: &[u8] = if buffer.is_null() || size == 0 {
            &[]
        } else {
            // SAFETY: BTstack passes the value of `size` bytes, valid during
            // the call.
            unsafe { std::slice::from_raw_parts(buffer, size as usize) }
        };
        // A write invalidates what a read kept
        READ_CACHE.with(|c| c.borrow_mut().take());
        let request = AttWriteRequest { con, handle, mode, offset, value };
        if let Ok(code) = with_free_app(|app, s| app.on_att_write(s, &request)) {
            tracing::trace!(target: "le", "ATT: write 0x{handle:04X} mode {mode} offset {offset} on 0x{con:04X}, {size} bytes: 0x{code:02X}");
            return code as c_int;
        }
        // The app is busy
        let answered_pending = matches!(mode, ATT_TRANSACTION_MODE_NONE | ATT_TRANSACTION_MODE_ACTIVE);
        if mode == ATT_TRANSACTION_MODE_VALIDATE {
            // Cannot be put off (BTstack would keep the "pending" as the
            // queue's error): the execute that follows is delivered
            tracing::debug!(target: "le", "ATT: prepared writes of 0x{con:04x} validated without the app (busy)");
            return 0;
        }
        KEPT_WRITES.with(|w| {
            w.borrow_mut().entry(con).or_default().push(KeptWrite {
                handle,
                mode,
                offset,
                value: value.to_vec(),
                answered_pending,
            })
        });
        tracing::debug!(target: "le", "ATT: write to 0x{handle:04X} on 0x{con:04X} kept (app busy, mode {mode})");
        queue_retry(con);
        if answered_pending { ATT_ERROR_WRITE_RESPONSE_PENDING } else { 0 }
    })
}

/// From PENDING once the app is free: BTstack handles a request that was
/// answered "pending" again, kept writes go to the app
pub(crate) fn retry(con: u16) {
    RETRY_QUEUED.with(|r| r.borrow_mut().retain(|c| *c != con));
    // SAFETY: on the BTstack thread; the app is not borrowed (BTstack may
    // call the read / write callbacks from here).
    let status = unsafe { ffi::att_server_response_ready(con) };
    let mut kept = KEPT_WRITES.with(|w| w.borrow_mut().remove(&con)).unwrap_or_default();
    tracing::debug!(target: "le", "ATT: retry on 0x{con:04X} (response ready 0x{status:02X}, {} kept write(s))", kept.len());
    if status == ERROR_CODE_SUCCESS {
        // The request answered "pending" comes again by itself
        if let Some(i) = kept.iter().rposition(|w| w.answered_pending) {
            kept.remove(i);
        }
    }
    for w in kept {
        let request = AttWriteRequest { con, handle: w.handle, mode: w.mode, offset: w.offset, value: &w.value };
        let delivered = with_free_app(|app, s| {
            app.on_att_write(s, &request);
        });
        if delivered.is_err() {
            tracing::warn!(target: "le", "ATT: a write to 0x{:04x} on 0x{con:04x} was lost (app busy)", w.handle);
        }
    }
}

/// The connection is gone (HCI Disconnection Complete)
pub(crate) fn disconnected(con: u16) {
    tracing::debug!(target: "le", "ATT: 0x{con:04X} disconnected, dropping its cached read, kept writes and queued sends");
    READ_CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.as_ref().is_some_and(|e| e.con == con) {
            c.take();
        }
    });
    KEPT_WRITES.with(|w| w.borrow_mut().remove(&con));
    SEND.with(|s| s.borrow_mut().remove(&con));
}

#[unsafe(no_mangle)]
extern "C" fn a2dpwb_rs_le_disconnected(con: u16) {
    guard((), || disconnected(con))
}

// ---- Notifications / indications ----

/// Queues a notification or indication of `value_handle` on `con`
pub(crate) fn send(con: u16, value_handle: u16, value: &[u8], indication: bool) {
    SEND.with(|s| s.borrow_mut().entry(con).or_default().items.push_back((value_handle, indication, value.to_vec())));
    request(con);
}

/// Registers for the next can-send-now if something waits. The callback may
/// come from within the request: no borrow is held across it.
fn request(con: u16) {
    let front = SEND.with(|s| {
        let mut s = s.borrow_mut();
        let q = s.get_mut(&con)?;
        if q.waiting {
            return None;
        }
        let indication = q.items.front()?.1;
        q.waiting = true;
        Some(indication)
    });
    let Some(indication) = front else { return };
    // SAFETY: on the BTstack thread; the registration is static in C.
    let status = unsafe { ffi::a2dpwb_att_server_request_to_send(con, indication as c_int) };
    if status != ERROR_CODE_SUCCESS && status != ERROR_CODE_COMMAND_DISALLOWED {
        // COMMAND_DISALLOWED: already registered, the callback comes
        tracing::warn!(target: "le", "LE Audio: GATT server cannot send on 0x{con:04x} (0x{status:02x}), dropping");
        SEND.with(|s| s.borrow_mut().remove(&con));
    }
}

#[unsafe(no_mangle)]
extern "C" fn a2dpwb_rs_att_can_send(con: u16) {
    guard((), || {
        let item = SEND.with(|s| {
            let mut s = s.borrow_mut();
            let q = s.get_mut(&con)?;
            q.waiting = false;
            q.items.pop_front()
        });
        let Some((handle, indication, value)) = item else { return };
        // SAFETY: on the BTstack thread.
        let mtu = unsafe { ffi::att_server_get_mtu(con) } as usize;
        let len = if mtu > 3 { value.len().min(mtu - 3) } else { value.len() };
        if len < value.len() {
            tracing::debug!(target: "le", "ATT: value of 0x{handle:04X} cut from {} to {len} bytes (mtu {mtu})", value.len());
        }
        // SAFETY: on the BTstack thread in its can-send-now callback; value
        // is valid for len bytes during the call (BTstack copies it).
        let status = unsafe {
            if indication {
                ffi::att_server_indicate(con, handle, value.as_ptr(), len as u16)
            } else {
                ffi::att_server_notify(con, handle, value.as_ptr(), len as u16)
            }
        };
        if status != ERROR_CODE_SUCCESS {
            let what = if indication { "indication" } else { "notification" };
            tracing::warn!(target: "le", "LE Audio: GATT {what} of 0x{handle:04x} failed (0x{status:02x})");
        }
        request(con);
    })
}

#[cfg(test)]
mod tests {
    use super::blob;

    #[test]
    fn blob_parts() {
        let v = [1u8, 2, 3, 4, 5];
        assert_eq!(blob(&v, 0, 22), &v);
        assert_eq!(blob(&v, 2, 22), &[3, 4, 5]);
        assert_eq!(blob(&v, 1, 2), &[2, 3]);
        assert_eq!(blob(&v, 5, 22), &[] as &[u8]);
        assert_eq!(blob(&v, 6, 22), &[] as &[u8]);
    }
}
