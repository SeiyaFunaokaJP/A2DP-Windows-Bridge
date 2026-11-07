//! LE against the virtual LE Audio sink of tools/emu (emu_le_audio_sink.py),
//! H4 over TCP: with LE unhidden, HCI init completes, LE Read Buffer Size v2
//! reports ISO buffers, the sink is found by an LE scan, connected, paired
//! (LE Secure Connections, Just Works, bond in a TLV file) and its GATT
//! services are discovered; with LE hidden (the default), init completes
//! without any LE command reaching the controller.
//!
//! Needs the emulator, so it is ignored by default:
//!
//! ```text
//! tools\emu\.venv\Scripts\python.exe tools\emu\emu_le_audio_sink.py --port 9311
//! set BLUECASTER_EMU_LE=127.0.0.1:9311
//! cargo test -p btstack-sys --test le_emulator -- --ignored --nocapture --test-threads 1
//! ```

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use btstack_sys::consts::*;
use btstack_sys::{App, CigParams, CisParams, HciBackend, Remote, Source, Stack, Upstream, events};

/// H4 over TCP, synchronous
struct Tcp {
    address: String,
    stream: Option<TcpStream>,
    /// Commands that reached the controller (opcodes)
    sent: Arc<Mutex<Vec<u16>>>,
}

fn h4_length(packet_type: u8, h: &[u8]) -> Option<usize> {
    match packet_type {
        HCI_EVENT_PACKET if h.len() >= 2 => Some(2 + h[1] as usize),
        HCI_ACL_DATA_PACKET if h.len() >= 4 => Some(4 + u16::from_le_bytes([h[2], h[3]]) as usize),
        HCI_ISO_DATA_PACKET if h.len() >= 4 => Some(4 + (u16::from_le_bytes([h[2], h[3]]) & 0x3FFF) as usize),
        _ => None,
    }
}

impl HciBackend for Tcp {
    fn is_async(&self) -> bool {
        false
    }
    fn open(&mut self, upstream: Upstream) -> Result<(), String> {
        let stream = TcpStream::connect(&self.address).map_err(|e| e.to_string())?;
        stream.set_nodelay(true).ok();
        let mut reader = stream.try_clone().map_err(|e| e.to_string())?;
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            while let Ok(n) = reader.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                while let Some(&t) = buf.first() {
                    let Some(len) = h4_length(t, &buf[1..]) else { break };
                    if buf.len() < 1 + len {
                        break;
                    }
                    upstream.deliver(t, &buf[1..1 + len]);
                    buf.drain(..1 + len);
                }
            }
        });
        self.stream = Some(stream);
        Ok(())
    }
    fn close(&mut self) {
        if let Some(s) = self.stream.take() {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    }
    fn send(&mut self, packet_type: u8, packet: &[u8]) -> bool {
        if packet_type == HCI_COMMAND_DATA_PACKET && packet.len() >= 2 {
            self.sent.lock().unwrap().push(u16::from_le_bytes([packet[0], packet[1]]));
        }
        let Some(s) = self.stream.as_mut() else { return false };
        let mut out = Vec::with_capacity(1 + packet.len());
        out.push(packet_type);
        out.extend_from_slice(packet);
        s.write_all(&out).is_ok()
    }
}

#[derive(Debug, PartialEq)]
enum Ev {
    Working,
    IsoBuffers(u16, u8),
    Found([u8; 6], u8),
    Connected(u16),
    Paired(u8),
    Services(usize, Vec<u16>),
    Cig(u8, Vec<u16>),
    AttRead(u16),
}

fn cig_params(cig_id: u8) -> CigParams {
    CigParams {
        cig_id,
        sdu_interval_c_to_p: 10_000,
        sdu_interval_p_to_c: 10_000,
        framing: 0,
        max_transport_latency_c_to_p: 20,
        max_transport_latency_p_to_c: 20,
        cis: vec![CisParams {
            cis_id: 1,
            max_sdu_c_to_p: 240,
            phy_c_to_p: LE_PHY_2M,
            phy_p_to_c: LE_PHY_2M,
            rtn_c_to_p: 2,
            rtn_p_to_c: 2,
            ..Default::default()
        }],
        ..Default::default()
    }
}

struct LeApp {
    tx: mpsc::Sender<Ev>,
    full: bool,
    target: Option<([u8; 6], u8)>,
    con: u16,
    services: Vec<u16>,
    cigs: u32,
}

impl App for LeApp {
    fn on_packet(&mut self, s: &Stack, source: Source, packet_type: u8, _channel: u16, p: &[u8]) {
        if packet_type != HCI_EVENT_PACKET {
            return;
        }
        match source {
            Source::Hci => {
                if p.first() == Some(&BTSTACK_EVENT_STATE) && p.get(2) == Some(&2) {
                    let _ = self.tx.send(Ev::Working);
                    if self.full {
                        s.le_set_scan_params(true, 0x30, 0x30, 0);
                        s.le_start_scan();
                        s.timer_start(TIMER_LE, 5000);
                    }
                }
                if let Some(b) = events::le_read_buffer_size_v2(p) {
                    eprintln!("LE Read Buffer Size v2: {b:?}");
                    let _ = self.tx.send(Ev::IsoBuffers(b.iso_packet_length, b.iso_packets));
                }
                if let Some(r) = events::adv_report(p)
                    && self.target.is_none()
                    // ASCS (0x184E) in the service data / UUID list
                    && r.data.windows(2).any(|w| w == [0x4E, 0x18])
                {
                    s.le_stop_scan();
                    self.target = Some((r.addr, r.addr_type));
                    let _ = self.tx.send(Ev::Found(r.addr, r.addr_type));
                    s.le_connect(&r.addr, r.addr_type);
                }
                if let Some(c) = events::cig_created(p) {
                    let _ = self.tx.send(Ev::Cig(c.status, c.cis_handles));
                    if self.cigs == 1 {
                        // CIG 1 is held by BTstack: same id refused, a
                        // second CIG in the next storage slot
                        assert_eq!(s.cig_create(&cig_params(1)), ERROR_CODE_ACL_CONNECTION_ALREADY_EXISTS);
                        self.cigs = 2;
                        assert_eq!(s.cig_create(&cig_params(2)), 0);
                    }
                }
                if let Some(c) = events::le_connection_complete(p) {
                    assert_eq!(c.status, 0, "LE connection failed");
                    self.con = c.handle;
                    let _ = self.tx.send(Ev::Connected(c.handle));
                    assert!(s.gatt_listen(c.handle));
                    s.sm_request_pairing(c.handle);
                }
            }
            Source::Sm => {
                if p.first() == Some(&SM_EVENT_JUST_WORKS_REQUEST) {
                    s.sm_just_works_confirm(events::sm_handle(p).unwrap());
                }
                if let Some(c) = events::sm_pairing_complete(p) {
                    let _ = self.tx.send(Ev::Paired(c.status));
                    assert_eq!(s.gatt_discover_primary_services(c.handle), 0);
                }
            }
            Source::GattClient => {
                if let Some((_, svc)) = events::gatt_service_query_result(p) {
                    self.services.push(svc.uuid16);
                }
                if let Some((_, status)) = events::gatt_query_complete(p) {
                    let _ = self.tx.send(Ev::Services(status as usize, std::mem::take(&mut self.services)));
                    // LE Set CIG Parameters
                    self.cigs = 1;
                    assert_eq!(s.cig_create(&cig_params(1)), 0);
                }
            }
            _ => {}
        }
    }
    fn on_wake(&mut self, _s: &Stack) {}
    fn on_timer(&mut self, s: &Stack, id: u32) {
        if id == TIMER_LE && self.target.is_none() {
            s.le_stop_scan();
        }
    }
    fn on_att_read(&mut self, _s: &Stack, _con: u16, handle: u16, _offset: u16) -> Result<Vec<u8>, u8> {
        let _ = self.tx.send(Ev::AttRead(handle));
        Ok(b"BlueCaster".to_vec())
    }
}

fn emulator() -> String {
    std::env::var("BLUECASTER_EMU_LE").expect("set BLUECASTER_EMU_LE=host:port of emu_le_audio_sink.py")
}

fn wait_for(rx: &mpsc::Receiver<Ev>, mut what: impl FnMut(&Ev) -> bool, secs: u64, seen: &mut Vec<Ev>) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(e) => {
                let hit = what(&e);
                seen.push(e);
                if hit {
                    return true;
                }
            }
            Err(_) => return false,
        }
    }
    false
}

fn start(
    hidden: bool,
    full: bool,
    sent: Arc<Mutex<Vec<u16>>>,
) -> (mpsc::Receiver<Ev>, Remote, std::thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let (rtx, rrx) = mpsc::channel();
    let address = emulator();
    let tlv = std::env::temp_dir().join(format!("bluecaster_le_test_{}.tlv", std::process::id()));
    let _ = std::fs::remove_file(&tlv);
    let thread = std::thread::spawn(move || {
        btstack_sys::run(move |s, remote| {
            assert!(s.set_le_hidden(hidden));
            s.hci_init(Box::new(Tcp { address, stream: None, sent }));
            assert!(!s.set_le_hidden(!hidden), "too late after hci_init");
            s.set_link_key_store(None);
            s.l2cap_init();
            s.add_hci_event_handler();
            if !hidden {
                assert!(s.le_init(tlv.to_str().unwrap()));
                s.att_db_init();
                s.att_db_add_service_uuid16(0x1800, true);
                let name = s.att_db_add_characteristic_uuid16(
                    0x2A00,
                    ATT_PROPERTY_READ | ATT_PROPERTY_DYNAMIC,
                    ATT_SECURITY_NONE,
                    ATT_SECURITY_NONE,
                    &[],
                );
                assert_eq!(name, 3);
                assert!(s.att_db_size() > 0);
                assert!(!s.att_db_hash_input().is_empty());
                assert!(s.att_server_init());
            }
            s.power_on().expect("power on");
            rtx.send(remote).unwrap();
            Box::new(LeApp { tx, full, target: None, con: 0, services: Vec::new(), cigs: 0 })
        })
        .unwrap();
    });
    (rx, rrx.recv_timeout(Duration::from_secs(5)).unwrap(), thread)
}

#[test]
#[ignore = "needs tools/emu/emu_le_audio_sink.py (BLUECASTER_EMU_LE)"]
fn le_unhidden_init_scan_pair_discover() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let (rx, remote, thread) = start(false, true, sent.clone());
    let mut seen = Vec::new();
    let mut cigs = 0;
    let ok = wait_for(
        &rx,
        |e| {
            if matches!(e, Ev::Cig(..)) {
                cigs += 1;
            }
            cigs == 2
        },
        30,
        &mut seen,
    );
    remote.request_exit();
    thread.join().unwrap();
    eprintln!("events: {seen:?}");
    assert!(ok, "{seen:?}");
    assert!(seen.contains(&Ev::Working));
    assert!(seen.iter().any(|e| matches!(e, Ev::IsoBuffers(len, n) if *len > 0 && *n > 0)), "{seen:?}");
    assert!(seen.contains(&Ev::Paired(0)), "{seen:?}");
    let Some(Ev::Services(status, uuids)) = seen.iter().find(|e| matches!(e, Ev::Services(..))) else { unreachable!() };
    assert_eq!(*status, 0);
    assert!(!btstack_sys::poisoned(), "a callback panicked (see above)");
    let cigs: Vec<&Ev> = seen.iter().filter(|e| matches!(e, Ev::Cig(..))).collect();
    assert!(matches!(cigs[..], [Ev::Cig(0, a), Ev::Cig(0, b)] if a.len() == 1 && b.len() == 1 && a != b), "{cigs:?}");
    // PACS and ASCS
    assert!(uuids.contains(&0x1850) && uuids.contains(&0x184E), "{uuids:x?}");
    let sent = sent.lock().unwrap();
    // (Write LE Host Supported is not sent to Bumble's controller: it does
    // not list it as supported)
    for op in [HCI_OPCODE_LE_READ_BUFFER_SIZE_V2, HCI_OPCODE_LE_SET_SCAN_PARAMETERS, HCI_OPCODE_LE_SET_CIG_PARAMETERS] {
        assert!(sent.contains(&op), "0x{op:04x} not sent: {sent:x?}");
    }
}

#[test]
#[ignore = "needs tools/emu/emu_le_audio_sink.py (BLUECASTER_EMU_LE)"]
fn le_hidden_init() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let (rx, remote, thread) = start(true, false, sent.clone());
    let mut seen = Vec::new();
    let ok = wait_for(&rx, |e| *e == Ev::Working, 15, &mut seen);
    // LE Set Scan Parameters comes once HCI is working
    std::thread::sleep(Duration::from_millis(500));
    remote.request_exit();
    thread.join().unwrap();
    assert!(ok, "{seen:?}");
    let sent = sent.lock().unwrap();
    assert!(sent.contains(&0x0C03), "{sent:x?}");
    assert!(
        !sent.iter().any(|op| op >> 10 == OGF_LE_CONTROLLER || *op == HCI_OPCODE_WRITE_LE_HOST_SUPPORTED),
        "an LE command reached the controller: {sent:x?}"
    );
}

#[test]
#[ignore = "needs tools/emu/emu_le_audio_sink.py (BLUECASTER_EMU_LE)"]
fn le_unhidden_twice_in_one_process() {
    // LE setup, teardown (SM, ATT server, TLV store) and setup again
    for run in 0..2 {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let (rx, remote, thread) = start(false, false, sent);
        let mut seen = Vec::new();
        let ok = wait_for(&rx, |e| *e == Ev::Working, 15, &mut seen);
        remote.request_exit();
        thread.join().unwrap();
        assert!(ok, "run {run}: {seen:?}");
        assert!(seen.iter().any(|e| matches!(e, Ev::IsoBuffers(_, n) if *n > 0)), "run {run}: {seen:?}");
        assert!(!btstack_sys::poisoned());
    }
}
