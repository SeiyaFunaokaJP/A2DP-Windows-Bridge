//! The run loop with a fake HCI backend: wakes, timers, the HCI Reset that
//! power on sends, exit and a second run in the same process.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use btstack_sys::{App, HciBackend, Remote, Source, Stack, Upstream};

#[derive(Default)]
struct Shared {
    sent: Mutex<Vec<(u8, Vec<u8>)>>,
    opened: AtomicU32,
    closed: AtomicU32,
}

struct FakeBackend(Arc<Shared>);

impl HciBackend for FakeBackend {
    fn is_async(&self) -> bool {
        false
    }
    fn open(&mut self, _upstream: Upstream) -> Result<(), String> {
        self.0.opened.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn close(&mut self) {
        self.0.closed.fetch_add(1, Ordering::SeqCst);
    }
    fn send(&mut self, packet_type: u8, packet: &[u8]) -> bool {
        self.0.sent.lock().unwrap().push((packet_type, packet.to_vec()));
        true
    }
}

struct TestApp {
    events: mpsc::Sender<String>,
}

impl App for TestApp {
    fn on_packet(&mut self, _s: &Stack, source: Source, packet_type: u8, _channel: u16, packet: &[u8]) {
        if source == Source::Hci && packet_type == 4 && packet.first() == Some(&0x60) {
            let _ = self.events.send(format!("state {}", packet[2]));
        }
    }
    fn on_wake(&mut self, s: &Stack) {
        let _ = self.events.send("wake".into());
        s.timer_start(3, 10);
    }
    fn on_timer(&mut self, _s: &Stack, id: u32) {
        let _ = self.events.send(format!("timer {id}"));
    }
}

fn one_run(shared: &Arc<Shared>) {
    let (tx, rx) = mpsc::channel();
    let (remote_tx, remote_rx) = mpsc::channel::<Remote>();
    let backend_shared = shared.clone();
    let thread = std::thread::spawn(move || {
        btstack_sys::run(move |stack, remote| {
            stack.hci_init(Box::new(FakeBackend(backend_shared)));
            stack.set_link_key_store(None);
            stack.l2cap_init();
            stack.sdp_init();
            stack.add_hci_event_handler();
            stack.a2dp_source_init();
            stack.avrcp_init();
            // HFP too: its RFCOMM service is set up and torn down each run
            stack.hfp_ag_init();
            stack.sdp_register_records(2, 1);
            assert!(stack.create_stream_endpoint(0, 0, &[0x3F, 0x15, 2, 53], &[0x11, 0x15, 2, 53]).is_some());
            stack.power_on().expect("power on");
            remote_tx.send(remote).unwrap();
            Box::new(TestApp { events: tx })
        })
        .unwrap();
    });
    let remote = remote_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    remote.wake();
    let mut seen = Vec::new();
    while let Ok(e) = rx.recv_timeout(Duration::from_secs(2)) {
        seen.push(e);
        if seen.iter().any(|e| e == "timer 3") {
            break;
        }
    }
    assert!(seen.contains(&"wake".to_string()), "{seen:?}");
    assert!(seen.contains(&"timer 3".to_string()), "{seen:?}");
    // Power on opened the transport and sent HCI Reset (03 0C 00)
    let sent = shared.sent.lock().unwrap().clone();
    assert!(sent.iter().any(|(t, p)| *t == 1 && p.starts_with(&[0x03, 0x0C, 0x00])), "{sent:?}");
    remote.request_exit();
    thread.join().unwrap();
    assert!(!remote.alive());
}

#[test]
fn run_twice() {
    let shared = Arc::new(Shared::default());
    one_run(&shared);
    one_run(&shared);
    one_run(&shared);
    assert_eq!(shared.opened.load(Ordering::SeqCst), 3);
    assert!(shared.closed.load(Ordering::SeqCst) >= 3);
}

#[test]
fn realtek_table() {
    let ids = btstack_sys::realtek_usb_ids();
    assert!(ids.contains(&(0x2357, 0x0604)) || ids.iter().any(|(v, _)| *v == 0x0BDA), "{ids:?}");
    // The firmware names read from the same table
    let table = btstack_sys::realtek_usb_firmware();
    assert_eq!(table.len(), ids.len());
    assert!(table.contains(&(0x0BDA, 0x4853, "rtl8852bu_fw")), "{table:?}");
    assert!(table.iter().all(|(_, _, fw)| fw.ends_with("_fw")), "{table:?}");
}
