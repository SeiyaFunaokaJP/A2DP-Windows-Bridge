//! Functions csrc/shim.c calls: they run on the BTstack thread, inside the
//! run loop, and must never unwind into C.

use std::ffi::{CStr, c_char, c_int};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::Ordering;

use crate::{
    APP, App, LINK_KEYS, LOGGER, PENDING, POISONED, Source, Stack, WAKE_APP, WAKE_EXIT, WAKE_FLAGS, ffi, post_wake,
    transport,
};

/// An event for the app that could not be delivered at once
pub(crate) enum Pending {
    Packet(Source, u8, u16, Vec<u8>),
    Timer(u32),
    Intel(i32),
    /// An ATT request of this connection was answered "pending" while the
    /// app was busy (crate::att)
    AttRetry(u16),
}

/// Runs `f` and keeps a panic from reaching C: the panic is logged and the
/// run loop is stopped (the owner of the stack restarts it).
pub(crate) fn guard<R>(default: R, f: impl FnOnce() -> R) -> R {
    match std::panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(e) => {
            let what = e
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| e.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".into());
            tracing::error!(target: "btstack", "panic in a BTstack callback: {what}; stopping the stack");
            POISONED.store(true, Ordering::Release);
            WAKE_FLAGS.fetch_or(WAKE_EXIT, Ordering::AcqRel);
            post_wake();
            default
        }
    }
}

/// Delivers `event` to the app, or queues it if the app is busy (this is a
/// nested call) or not set yet. True when delivered.
fn dispatch(stack: &Stack, event: Pending) -> bool {
    let event = APP.with(|a| match a.try_borrow_mut() {
        Ok(mut slot) => match slot.as_mut() {
            Some(app) => {
                deliver(app.as_mut(), stack, event);
                None
            }
            None => Some(event),
        },
        Err(_) => Some(event),
    });
    match event {
        None => true,
        Some(event) => {
            PENDING.with(|p| p.borrow_mut().push_back(event));
            tracing::trace!(target: "btstack", "event queued (app busy), {} pending", PENDING.with(|p| p.borrow().len()));
            false
        }
    }
}

fn deliver(app: &mut dyn App, stack: &Stack, event: Pending) {
    match event {
        Pending::Packet(source, packet_type, channel, data) => {
            app.on_packet(stack, source, packet_type, channel, &data)
        }
        Pending::Timer(id) => app.on_timer(stack, id),
        Pending::Intel(result) => app.on_intel_firmware(stack, result),
        // Not for the app (drain_pending handles it without a borrow)
        Pending::AttRetry(_) => {}
    }
}

/// Delivers queued events, unless the app is busy (then its caller drains)
pub(crate) fn drain_pending(stack: &Stack) {
    loop {
        let Some(event) = PENDING.with(|p| p.borrow_mut().pop_front()) else { return };
        if let Pending::AttRetry(con) = event {
            if app_free() {
                crate::att::retry(con);
                continue;
            }
            PENDING.with(|p| p.borrow_mut().push_front(event));
            return;
        }
        let undelivered = APP.with(|a| match a.try_borrow_mut() {
            Ok(mut slot) => match slot.as_mut() {
                Some(app) => {
                    deliver(app.as_mut(), stack, event);
                    None
                }
                None => Some(event),
            },
            Err(_) => Some(event),
        });
        if let Some(event) = undelivered {
            PENDING.with(|p| p.borrow_mut().push_front(event));
            return;
        }
    }
}

/// The app is set and not in one of its calls
fn app_free() -> bool {
    APP.with(|a| a.try_borrow().is_ok_and(|slot| slot.is_some()))
}

/// Runs `f` on the app (not from within another app call), then delivers
/// the events it caused
pub(crate) fn with_app<R>(stack: &Stack, f: impl FnOnce(&mut dyn App, &Stack) -> R) -> Option<R> {
    let r = APP.with(|a| {
        let mut slot = a.try_borrow_mut().ok()?;
        let app = slot.as_mut()?;
        Some(f(app.as_mut(), stack))
    });
    drain_pending(stack);
    r
}

// ---- Run loop ----

#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_on_wake() {
    guard((), || {
        let stack = Stack::new();
        transport::pump();
        let flags = WAKE_FLAGS.swap(0, Ordering::AcqRel);
        if flags & WAKE_APP != 0 {
            with_app(&stack, |app, s| app.on_wake(s));
        }
        if flags & WAKE_EXIT != 0 || POISONED.load(Ordering::Acquire) {
            // SAFETY: on the run loop thread, inside the run loop.
            unsafe { ffi::btstack_run_loop_trigger_exit() };
        }
    })
}

#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_on_timer(id: u32) {
    guard((), || {
        let stack = Stack::new();
        tracing::trace!(target: "btstack", "timer {id} fired");
        if dispatch(&stack, Pending::Timer(id)) {
            drain_pending(&stack);
        }
    })
}

#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_on_packet(source: u8, packet_type: u8, channel: u16, packet: *mut u8, size: u16) {
    guard((), || {
        let data: &[u8] = if packet.is_null() || size == 0 {
            &[]
        } else {
            // SAFETY: BTstack passes a buffer of `size` bytes that stays valid
            // for the duration of the call.
            unsafe { std::slice::from_raw_parts(packet, size as usize) }
        };
        let stack = Stack::new();
        let source = Source::from_raw(source);
        // Delivered directly (no copy) unless the app is busy
        let delivered = APP.with(|a| match a.try_borrow_mut() {
            Ok(mut slot) => match slot.as_mut() {
                Some(app) => {
                    app.on_packet(&stack, source, packet_type, channel, data);
                    true
                }
                None => false,
            },
            Err(_) => false,
        });
        if delivered {
            drain_pending(&stack);
        } else {
            PENDING.with(|p| p.borrow_mut().push_back(Pending::Packet(source, packet_type, channel, data.to_vec())));
            tracing::trace!(target: "btstack", "{source:?} packet of {} bytes queued (app busy)", data.len());
        }
    })
}

#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_intel_done(result: c_int) {
    guard((), || {
        let stack = Stack::new();
        tracing::debug!(target: "btstack", "intel firmware loader done: {result}");
        if dispatch(&stack, Pending::Intel(result)) {
            drain_pending(&stack);
        }
    })
}

// ---- HCI transport ----

#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_transport_open() -> c_int {
    guard(-1, || if transport::open() { 0 } else { -1 })
}

#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_transport_close() -> c_int {
    guard(0, || {
        transport::close();
        0
    })
}

#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_transport_can_send(packet_type: u8) -> c_int {
    guard(0, || transport::can_send(packet_type) as c_int)
}

#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_transport_send(packet_type: u8, packet: *const u8, size: c_int) -> c_int {
    guard(-1, || {
        if packet.is_null() || size < 0 {
            return -1;
        }
        // SAFETY: HCI passes its packet buffer of `size` bytes, valid during
        // the call (an asynchronous backend copies it).
        let data = unsafe { std::slice::from_raw_parts(packet, size as usize) };
        if transport::send(packet_type, data) { 0 } else { -1 }
    })
}

// ---- HCI dump ----

#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_log_packet(packet_type: u8, incoming: u8, packet: *const u8, len: u16) {
    guard((), || {
        if packet.is_null() {
            return;
        }
        // SAFETY: HCI dump passes a buffer of `len` bytes valid during the call.
        let data = unsafe { std::slice::from_raw_parts(packet, len as usize) };
        LOGGER.with(|l| {
            if let Ok(mut l) = l.try_borrow_mut()
                && let Some(logger) = l.as_mut()
            {
                logger(packet_type, incoming != 0, data);
            }
        });
    })
}

#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_log_message(level: c_int, message: *const c_char) {
    guard((), || {
        if message.is_null() {
            return;
        }
        // SAFETY: vsnprintf in shim.c produced a NUL-terminated string.
        let text = unsafe { CStr::from_ptr(message) }.to_string_lossy();
        let text = without_key_material(&text);
        // HCI_DUMP_LOG_LEVEL_DEBUG 0, _INFO 1, _ERROR 2, _PRINT 3 (the
        // Realtek driver's printf, see build.rs)
        match level {
            2 => tracing::warn!(target: "btstack", "{text}"),
            3 => tracing::info!(target: "btstack", "{}", text.trim_end()),
            1 if from_chipset_driver(&text) => tracing::info!(target: "btstack", "{text}"),
            1 => tracing::debug!(target: "btstack", "{text}"),
            _ => tracing::trace!(target: "btstack", "{text}"),
        }
    })
}

/// BTstack's info log prints key material (ENABLE_LOG_INFO; sm.c,
/// btstack_crypto.c): `log_info_key` writes "ltk    <32 hex digits>" for
/// the LTK, IRK, CSRK, the local IR / ER / DHK roots and the derived link
/// key, `log_info_hexdump` the DH key and the CMAC inputs as lines of 16
/// "AB " pairs. With verbose logging on, those lines reach the log file and
/// the diagnostics, so the digits become 'x' here, before the line is a
/// tracing event: a word of exactly 32 hex digits, and a line that is
/// nothing but 16 hex bytes (ATT values of that size go with it).
pub(crate) fn without_key_material(text: &str) -> std::borrow::Cow<'_, str> {
    let b = text.as_bytes();
    let trimmed = text.trim_end();
    let mut pairs = trimmed.split(' ');
    let hexdump = pairs.by_ref().take(16).filter(|p| p.len() == 2 && p.bytes().all(|c| c.is_ascii_hexdigit())).count()
        == 16
        && pairs.next().is_none();
    let mut out: Option<Vec<u8>> = None;
    if hexdump {
        let v = out.get_or_insert_with(|| b.to_vec());
        for c in &mut v[..trimmed.len()] {
            if c.is_ascii_hexdigit() {
                *c = b'x';
            }
        }
    }
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_alphanumeric() {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_alphanumeric() {
            i += 1;
        }
        if i - start == 32 && b[start..i].iter().all(u8::is_ascii_hexdigit) {
            out.get_or_insert_with(|| b.to_vec())[start..i].fill(b'x');
        }
    }
    match out {
        Some(v) => std::borrow::Cow::Owned(String::from_utf8(v).expect("only ASCII bytes were replaced")),
        None => std::borrow::Cow::Borrowed(text),
    }
}

/// A log line of BTstack's firmware loaders ("btstack_chipset_realtek.c.972:
/// Wrong signature. Quit!"): the only place that tells why an upload was
/// skipped (wrong file, size, project id...), so it goes to the log file
/// at the default level
pub(crate) fn from_chipset_driver(msg: &str) -> bool {
    match (msg.find("btstack_chipset_"), msg.find(": ")) {
        (Some(chip), Some(sep)) => chip < sep,
        _ => false,
    }
}

// ---- Link key store ----

fn read_addr(addr: *const u8) -> Option<[u8; 6]> {
    if addr.is_null() {
        return None;
    }
    let mut a = [0u8; 6];
    // SAFETY: BTstack passes a bd_addr_t (6 bytes).
    unsafe { std::ptr::copy_nonoverlapping(addr, a.as_mut_ptr(), 6) };
    Some(a)
}

#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_lk_get(addr: *const u8, key: *mut u8, key_type: *mut c_int) -> c_int {
    guard(0, || {
        let Some(a) = read_addr(addr) else { return 0 };
        let found = LINK_KEYS.with(|s| s.borrow_mut().as_mut().and_then(|s| s.get(&a)));
        match found {
            Some((k, t)) if !key.is_null() && !key_type.is_null() => {
                // SAFETY: BTstack passes a link_key_t (16 bytes) and a
                // link_key_type_t (an int-sized enum) to write to.
                unsafe {
                    std::ptr::copy_nonoverlapping(k.as_ptr(), key, 16);
                    *key_type = t;
                }
                1
            }
            _ => 0,
        }
    })
}

#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_lk_put(addr: *const u8, key: *const u8, key_type: c_int) {
    guard((), || {
        let Some(a) = read_addr(addr) else { return };
        if key.is_null() {
            return;
        }
        let mut k = [0u8; 16];
        // SAFETY: BTstack passes a link_key_t (16 bytes).
        unsafe { std::ptr::copy_nonoverlapping(key, k.as_mut_ptr(), 16) };
        LINK_KEYS.with(|s| {
            if let Some(s) = s.borrow_mut().as_mut() {
                s.put(&a, &k, key_type)
            }
        });
    })
}

#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_lk_delete(addr: *const u8) {
    guard((), || {
        let Some(a) = read_addr(addr) else { return };
        LINK_KEYS.with(|s| {
            if let Some(s) = s.borrow_mut().as_mut() {
                s.delete(&a)
            }
        });
    })
}

#[unsafe(no_mangle)]
extern "C" fn bluecaster_rs_lk_at(index: c_int, addr: *mut u8, key: *mut u8, key_type: *mut c_int) -> c_int {
    guard(0, || {
        if index < 0 || addr.is_null() || key.is_null() || key_type.is_null() {
            return 0;
        }
        let entry = LINK_KEYS.with(|s| s.borrow().as_ref().and_then(|s| s.at(index as usize)));
        match entry {
            Some((a, k, t)) => {
                // SAFETY: BTstack passes a bd_addr_t, a link_key_t and a
                // link_key_type_t to write to.
                unsafe {
                    std::ptr::copy_nonoverlapping(a.as_ptr(), addr, 6);
                    std::ptr::copy_nonoverlapping(k.as_ptr(), key, 16);
                    *key_type = t;
                }
                1
            }
            None => 0,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{from_chipset_driver, without_key_material};

    #[test]
    fn key_material_is_masked() {
        let m = without_key_material;
        // log_info_key: "%-6s %s"
        assert_eq!(m("ltk    0123456789ABCDEF0123456789ABCDEF"), "ltk    xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
        assert_eq!(m("IRK    0123456789abcdef0123456789abcdef"), "IRK    xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
        assert_eq!(m("Link Key 0123456789ABCDEF0123456789ABCDEF"), "Link Key xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
        // log_info_hexdump: 16 bytes per line, a space after each
        assert_eq!(
            m("01 23 45 67 89 AB CD EF 01 23 45 67 89 AB CD EF "),
            "xx xx xx xx xx xx xx xx xx xx xx xx xx xx xx xx "
        );
        assert_eq!(
            m("01 23 45 67 89 AB CD EF 01 23 45 67 89 AB CD EF"),
            "xx xx xx xx xx xx xx xx xx xx xx xx xx xx xx xx"
        );
        // Shorter dumps (ATT values, L2CAP options) and other hex stay
        assert_eq!(m("01 23 45 67 89 AB CD EF "), "01 23 45 67 89 AB CD EF ");
        assert_eq!(
            m("01 23 45 67 89 AB CD EF 01 23 45 67 89 AB CD EF 01 "),
            "01 23 45 67 89 AB CD EF 01 23 45 67 89 AB CD EF 01 "
        );
        assert_eq!(
            m("hci.c.123: handle 0x0040 addr 00:11:22:33:44:55"),
            "hci.c.123: handle 0x0040 addr 00:11:22:33:44:55"
        );
        assert_eq!(m("sm: hash 0123456789ABCDEF0123456789ABCDE"), "sm: hash 0123456789ABCDEF0123456789ABCDE");
        assert_eq!(m("x0123456789ABCDEF0123456789ABCDEF0"), "x0123456789ABCDEF0123456789ABCDEF0");
        assert!(matches!(m("plain"), std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn chipset_driver_lines() {
        assert!(from_chipset_driver("btstack_chipset_realtek.c.972: Wrong signature. Quit!"));
        assert!(from_chipset_driver("btstack_chipset_bcm.c.120: Using firmware: x.hcd"));
        assert!(!from_chipset_driver("hci.c.4321: hci_power_control: 1"));
        // The name later in the message is no source file prefix
        assert!(!from_chipset_driver("l2cap.c.10: see btstack_chipset_realtek"));
        assert!(!from_chipset_driver("btstack_chipset_realtek"));
    }
}
