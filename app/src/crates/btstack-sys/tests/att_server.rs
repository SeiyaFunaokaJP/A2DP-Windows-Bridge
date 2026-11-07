//! The PC's GATT server against a scripted controller (no emulator): LE
//! unhidden, HCI init, an LE link, then ATT requests as ACL packets.
//! - a read is answered with the app's value (asked once per request)
//! - a write reaches the app; its ATT error goes back to the client
//! - a notification queued by the app is sent
//! - a request BTstack handles from within an app call (the app is busy) is
//!   answered "pending" and handled again once the app's call has returned;
//!   a write command then is delivered afterwards

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use btstack_sys::consts::*;
use btstack_sys::{App, AttWriteRequest, HciBackend, Remote, Source, Stack, Upstream};

const CON: u16 = 0x0040;

#[derive(Default)]
struct Controller {
    upstream: Mutex<Option<Upstream>>,
    /// Packets BTstack sent
    sent: Mutex<Vec<(u8, Vec<u8>)>>,
}

impl Controller {
    fn deliver(&self, packet_type: u8, packet: &[u8]) {
        self.upstream.lock().unwrap().as_ref().unwrap().deliver(packet_type, packet);
    }

    /// ATT PDUs BTstack sent on the LE link
    fn att_sent(&self) -> Vec<Vec<u8>> {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .filter(|(t, p)| *t == HCI_ACL_DATA_PACKET && p.len() >= 8 && p[6] == 4)
            .map(|(_, p)| p[8..].to_vec())
            .collect()
    }
}

/// Return parameters of the Command Complete for `opcode`
fn answer(opcode: u16) -> Vec<u8> {
    match opcode {
        // Read Local Version: HCI 5.3, manufacturer 0x05F1
        0x1001 => vec![0x0C, 0, 0, 0x0C, 0xF1, 0x05, 0, 0],
        // Read Local Supported Commands: none of the optional ones
        0x1002 => vec![0; 64],
        // Read Local Supported Features: LE Supported (Controller) only
        0x1003 => vec![0, 0, 0, 0, 0x40, 0, 0, 0],
        // Read Buffer Size: ACL 1021 x 8
        0x1005 => vec![0xFD, 0x03, 0, 8, 0, 0, 0],
        0x1009 => vec![0x11, 0x00, 0x00, 0xD0, 0xA2, 0x00],
        // LE Read Buffer Size: 251 x 1 (one LE ACL buffer)
        0x2002 => vec![0xFB, 0x00, 1],
        0x2003 => vec![0; 8],
        0x200F => vec![8],
        _ => vec![],
    }
}

struct Scripted(Arc<Controller>);

impl HciBackend for Scripted {
    fn is_async(&self) -> bool {
        false
    }
    fn open(&mut self, upstream: Upstream) -> Result<(), String> {
        *self.0.upstream.lock().unwrap() = Some(upstream);
        Ok(())
    }
    fn close(&mut self) {}
    fn send(&mut self, packet_type: u8, packet: &[u8]) -> bool {
        self.0.sent.lock().unwrap().push((packet_type, packet.to_vec()));
        if packet_type == HCI_COMMAND_DATA_PACKET {
            let opcode = u16::from_le_bytes([packet[0], packet[1]]);
            let params = answer(opcode);
            let mut cc = vec![HCI_EVENT_COMMAND_COMPLETE, (4 + params.len()) as u8, 1];
            cc.extend_from_slice(&opcode.to_le_bytes());
            cc.push(0);
            cc.extend_from_slice(&params);
            self.0.deliver(HCI_EVENT_PACKET, &cc);
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Ev {
    Working,
    Read(u16, u16),
    Write(u16, u16, Vec<u8>),
    CompletedPackets,
    /// The app's on_wake that handed a request to BTstack returned
    NestedDone,
}

/// What the app does in its next on_wake
#[derive(Default)]
struct Script {
    notify: Option<Vec<u8>>,
    /// An ATT PDU BTstack gets while the app is in on_wake
    nested_request: Option<Vec<u8>>,
}

struct ServerApp {
    tx: mpsc::Sender<Ev>,
    name_handle: u16,
    controller: Arc<Controller>,
    script: Arc<Mutex<Script>>,
}

impl App for ServerApp {
    fn on_packet(&mut self, _s: &Stack, source: Source, packet_type: u8, _channel: u16, p: &[u8]) {
        if source != Source::Hci || packet_type != HCI_EVENT_PACKET {
            return;
        }
        if p.first() == Some(&BTSTACK_EVENT_STATE) && p.get(2) == Some(&2) {
            let _ = self.tx.send(Ev::Working);
        }
        // HCI Number Of Completed Packets: the ACL buffer is free again
        if p.first() == Some(&0x13) {
            let _ = self.tx.send(Ev::CompletedPackets);
        }
    }
    fn on_wake(&mut self, s: &Stack) {
        let (notify, nested) = {
            let mut script = self.script.lock().unwrap();
            (script.notify.take(), script.nested_request.take())
        };
        if let Some(value) = notify {
            s.att_server_send(CON, self.name_handle, &value, false);
        }
        if let Some(pdu) = nested {
            att_request(&self.controller, &pdu);
            // BTstack handles it now, while the app is busy here
            s.test_process_incoming_now();
            let _ = self.tx.send(Ev::NestedDone);
        }
    }
    fn on_timer(&mut self, _s: &Stack, _id: u32) {}
    fn on_att_read(&mut self, _s: &Stack, con: u16, handle: u16, _offset: u16) -> Result<Vec<u8>, u8> {
        let _ = self.tx.send(Ev::Read(con, handle));
        if handle == self.name_handle { Ok(b"BlueCaster".to_vec()) } else { Err(ATT_ERROR_READ_NOT_PERMITTED) }
    }
    fn on_att_write(&mut self, _s: &Stack, w: &AttWriteRequest) -> u8 {
        let _ = self.tx.send(Ev::Write(w.con, w.handle, w.value.to_vec()));
        if w.value.first() == Some(&0xEE) { 0x80 } else { 0 }
    }
}

/// Events until `want` (inclusive)
fn wait(rx: &mpsc::Receiver<Ev>, want: &Ev) -> Vec<Ev> {
    let mut seen = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(e) => {
                let hit = e == *want;
                seen.push(e);
                if hit {
                    return seen;
                }
            }
            Err(_) => break,
        }
    }
    panic!("{want:?} did not come: {seen:?}");
}

fn att_request(c: &Controller, pdu: &[u8]) {
    let mut acl = Vec::new();
    acl.extend_from_slice(&(CON | 0x2000).to_le_bytes());
    acl.extend_from_slice(&((4 + pdu.len()) as u16).to_le_bytes());
    acl.extend_from_slice(&(pdu.len() as u16).to_le_bytes());
    acl.extend_from_slice(&4u16.to_le_bytes());
    acl.extend_from_slice(pdu);
    c.deliver(HCI_ACL_DATA_PACKET, &acl);
}

/// HCI Number Of Completed Packets for the link: its LE buffer is free
fn completed(c: &Controller, rx: &mpsc::Receiver<Ev>) {
    let mut e = vec![0x13, 5, 1];
    e.extend_from_slice(&CON.to_le_bytes());
    e.extend_from_slice(&1u16.to_le_bytes());
    c.deliver(HCI_EVENT_PACKET, &e);
    wait(rx, &Ev::CompletedPackets);
}

/// The `n`th ATT PDU BTstack sent on the link (waits for it)
fn att_pdu(c: &Controller, n: usize) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let att = c.att_sent();
        if let Some(p) = att.get(n) {
            return p.clone();
        }
        assert!(Instant::now() < deadline, "ATT PDU {n} not sent: {att:x?}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn att_server_reads_and_writes() {
    let controller = Arc::new(Controller::default());
    let script = Arc::new(Mutex::new(Script::default()));
    let (tx, rx) = mpsc::channel();
    let (rtx, rrx) = mpsc::channel::<(Remote, u16, u16)>();
    let (c2, s2) = (controller.clone(), script.clone());
    let tlv = std::env::temp_dir().join(format!("bluecaster_att_test_{}.tlv", std::process::id()));
    let _ = std::fs::remove_file(&tlv);
    let tlv2 = tlv.clone();
    let thread = std::thread::spawn(move || {
        btstack_sys::run(move |s, remote| {
            assert!(s.set_le_hidden(false));
            s.hci_init(Box::new(Scripted(c2.clone())));
            s.set_link_key_store(None);
            s.l2cap_init();
            s.add_hci_event_handler();
            s.le_init(tlv2.to_str().unwrap());
            s.att_db_init();
            s.att_db_add_service_uuid16(0x1800, true);
            let name = s.att_db_add_characteristic_uuid16(
                0x2A00,
                ATT_PROPERTY_READ | ATT_PROPERTY_NOTIFY | ATT_PROPERTY_DYNAMIC,
                ATT_SECURITY_NONE,
                ATT_SECURITY_NONE,
                &[],
            );
            let cp = s.att_db_add_characteristic_uuid16(
                0x2BBE,
                ATT_PROPERTY_WRITE | ATT_PROPERTY_WRITE_WITHOUT_RESPONSE | ATT_PROPERTY_DYNAMIC,
                ATT_SECURITY_NONE,
                ATT_SECURITY_NONE,
                &[],
            );
            // A static value, patched in place
            let hash = s.att_db_add_characteristic_uuid16(
                0x2B2A,
                ATT_PROPERTY_READ,
                ATT_SECURITY_NONE,
                ATT_SECURITY_NONE,
                &[0xAB; 4],
            );
            assert_eq!(s.att_db_with_mut(|db| db.len()), Some(s.att_db_size()));
            // While lent, the database cannot change
            assert_eq!(s.att_db_with_mut(|_| s.att_db_add_service_uuid16(0x180F, true)), Some(0));
            let patched = s.att_db_with_mut(|db| {
                let at = db.windows(4).position(|w| w == [0xAB; 4]).unwrap();
                db[at..at + 4].copy_from_slice(b"HASH");
            });
            assert!(patched.is_some() && hash > cp);
            assert!(s.att_server_init());
            s.power_on().unwrap();
            rtx.send((remote, name, hash)).unwrap();
            Box::new(ServerApp { tx, name_handle: name, controller: c2, script: s2 })
        })
        .unwrap();
    });
    let (remote, name, hash) = rrx.recv_timeout(Duration::from_secs(5)).unwrap();
    let cp = name + 3; // name value, its CCC, cp declaration, cp value
    wait(&rx, &Ev::Working);
    let [lo, hi] = name.to_le_bytes();
    let [clo, chi] = cp.to_le_bytes();

    // LE Connection Complete: handle 0x40, central, peer 00:A2:D0:00:00:12
    let mut lcc = vec![HCI_EVENT_LE_META, 19, 0x01, 0];
    lcc.extend_from_slice(&CON.to_le_bytes());
    lcc.extend_from_slice(&[0, 0, 0x12, 0, 0, 0xD0, 0xA2, 0, 0x18, 0, 0, 0, 0x48, 0, 0]);
    controller.deliver(HCI_EVENT_PACKET, &lcc);

    // 1. Read Request: the app's value, asked once
    att_request(&controller, &[0x0A, lo, hi]);
    assert_eq!(att_pdu(&controller, 0), b"\x0bBlueCaster");
    completed(&controller, &rx);

    // 2. Write Requests: accepted, then refused with the app's error 0x80
    att_request(&controller, &[0x12, clo, chi, 1, 2]);
    assert_eq!(att_pdu(&controller, 1), [0x13]);
    completed(&controller, &rx);
    att_request(&controller, &[0x12, clo, chi, 0xEE]);
    assert_eq!(att_pdu(&controller, 2), [0x01, 0x12, clo, chi, 0x80]);
    completed(&controller, &rx);

    // The patched static value
    let [hlo, hhi] = hash.to_le_bytes();
    att_request(&controller, &[0x0A, hlo, hhi]);
    assert_eq!(att_pdu(&controller, 3), b"\x0bHASH");
    completed(&controller, &rx);

    // 3. A notification the app queues
    script.lock().unwrap().notify = Some(b"N".to_vec());
    remote.wake();
    assert_eq!(att_pdu(&controller, 4), [0x1B, lo, hi, b'N']);
    completed(&controller, &rx);

    // 4. A read handled while the app is busy: answered after its call
    script.lock().unwrap().nested_request = Some(vec![0x0A, lo, hi]);
    remote.wake();
    let seen = wait(&rx, &Ev::Read(CON, name));
    assert!(seen.contains(&Ev::NestedDone), "read before the app's call returned: {seen:?}");
    assert_eq!(att_pdu(&controller, 5), b"\x0bBlueCaster");
    completed(&controller, &rx);

    // 5. A write request handled while the app is busy: delivered once,
    // after its call, and answered then
    script.lock().unwrap().nested_request = Some(vec![0x12, clo, chi, 7]);
    remote.wake();
    let seen = wait(&rx, &Ev::Write(CON, cp, vec![7]));
    assert_eq!(seen.first(), Some(&Ev::NestedDone), "{seen:?}");
    assert_eq!(att_pdu(&controller, 6), [0x13]);
    completed(&controller, &rx);

    // 6. A write command (no response) while the app is busy: kept and
    // delivered after its call
    script.lock().unwrap().nested_request = Some(vec![0x52, clo, chi, 9]);
    remote.wake();
    let seen = wait(&rx, &Ev::Write(CON, cp, vec![9]));
    assert_eq!(seen.first(), Some(&Ev::NestedDone), "{seen:?}");

    // 7. Nothing was delivered twice, nothing else sent
    std::thread::sleep(Duration::from_millis(100));
    let rest: Vec<Ev> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert!(!rest.iter().any(|e| matches!(e, Ev::Read(..) | Ev::Write(..))), "{rest:?}");
    assert_eq!(controller.att_sent().len(), 7, "{:x?}", controller.att_sent());

    remote.request_exit();
    thread.join().unwrap();
    assert!(!btstack_sys::poisoned());
    let _ = std::fs::remove_file(&tlv);
}
