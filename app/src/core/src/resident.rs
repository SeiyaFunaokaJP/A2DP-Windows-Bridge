//! The resident mode: the core service, the IPC server for the UI, and the
//! tray icon with its own native menu (opens at once, without the UI:
//! profiles to connect, disconnect, open the app, exit). Left click opens
//! the UI's flyout.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use bluecaster_core::{Service, ServiceOptions};
use bt_host::HciCapture;
use ipc::{AclPacket, Event, IsoCounters, LinkCounters, LinkSample, LinkTotals};
use tao::event::{Event as TaoEvent, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use win_sys::notify::{Balloon, BalloonIcon, ClickAction, TrayNotifier};

use crate::state::{BalloonRequest, CoreState, TrayView};

/// Profiles listed in the tray menu (as v1)
const TRAY_PROFILES: usize = 10;

enum UserEvent {
    Tray(TrayView),
    Menu(String),
    Click,
    Balloon(BalloonRequest),
    Quit,
}

/// Starts the UI (bluecaster.exe next to us), optionally its flyout
fn launch_ui(flyout: bool) {
    let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) else { return };
    let ui = dir.join("bluecaster.exe");
    if !ui.exists() {
        tracing::warn!("UI not found: {}", ui.display());
        return;
    }
    let mut cmd = std::process::Command::new(ui);
    if flyout {
        cmd.arg("--flyout");
    }
    if let Err(e) = cmd.spawn() {
        tracing::warn!("cannot start the UI: {e}");
    }
}

fn tr(lang: &str, key: &str, default: &str) -> String {
    bluecaster_core::messages::translate(lang, key, default)
}

fn build_menu(view: &TrayView) -> Menu {
    let lang = view.language.as_str();
    let menu = Menu::new();
    let _ = menu.append(&MenuItem::with_id("open", tr(lang, "tray.open", "Open BlueCaster"), true, None));
    let _ = menu.append(&PredefinedMenuItem::separator());
    // Each by its device (issue #86: profile names are not shown)
    for (i, (name, label)) in view.profiles.iter().zip(&view.labels).take(TRAY_PROFILES).enumerate() {
        let active = view.streaming && view.active_profile.as_deref() == Some(name.as_str());
        let _ = menu.append(&CheckMenuItem::with_id(format!("profile:{i}"), label, true, active, None));
    }
    if !view.profiles.is_empty() {
        let _ = menu.append(&PredefinedMenuItem::separator());
    }
    let _ =
        menu.append(&MenuItem::with_id("disconnect", tr(lang, "tray.disconnect", "Disconnect"), view.streaming, None));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id("exit", tr(lang, "tray.exit", "Exit"), true, None));
    menu
}

/// The sampler's tick: link samples 4 times a second while a UI subscribes
const SAMPLE: Duration = Duration::from_millis(250);
/// Ticks between the flow's audio rates and receiver test updates (1 s)
const RATE_TICKS: u64 = 4;
/// Ticks between capture size updates (2 s)
const CAPTURE_TICKS: u64 = 8;

/// The ISO counters of an LE Audio (experimental) stream on `cis` CIS
fn iso_counters(cis: u32, s: &bt_host::LeStreamStats) -> IsoCounters {
    IsoCounters {
        cis,
        sdu_sets: s.sets_sent,
        silence: s.silence_sets,
        underruns: s.underruns,
        dropped: s.overflows,
        prefill: s.prefill,
    }
}

/// A link sample from the counters now and those of the last sample; `iso`:
/// an LE Audio stream's counters (the A2DP media figures are then 0 / None)
fn link_sample(
    time_ms: u64,
    stats: &bt_host::LinkStats,
    last: Option<(Instant, LinkTotals)>,
    now: Instant,
    source_rate: u32,
    iso: Option<IsoCounters>,
) -> (LinkSample, LinkTotals) {
    let (total, start, starts) = crate::state::link_totals(stats);
    let (bps, pps) = match last {
        Some((t, prev)) => {
            let dt = now.duration_since(t).as_secs_f64().max(0.001);
            let d = total.since(&prev);
            (d.bytes as f64 / dt, d.packets as f64 / dt)
        }
        None => (0.0, 0.0),
    };
    let (bps, pps) = if iso.is_some() { (0.0, 0.0) } else { (bps, pps) };
    let air = bluecaster_core::link::airtime(pps, bps);
    let since = total.since(&start);
    let radio = stats.radio();
    let sample = LinkSample {
        time_ms,
        rssi: radio.rssi,
        afh_in_use: radio.afh_in_use,
        afh_usable: radio.afh_usable,
        afh_avoided: radio.afh_avoided,
        bytes_per_second: bps,
        packets_per_second: pps,
        queue_depth: if iso.is_some() { 0 } else { stats.queue_depth.load(Ordering::Relaxed) },
        dropped: since.dropped,
        capture_dropped: since.capture_dropped,
        airtime: air.map_or(0.0, |a| a.airtime),
        avg_packet_bytes: air.map(|a| a.avg_packet_bytes),
        acl_packet: air.map(|a| AclPacket { name: a.packet.into(), slots: a.slots }),
        bandwidth_share: air.map(|a| a.bandwidth_share),
        queue_capacity: iso.is_none().then_some(bt_host::stats::MEDIA_QUEUE_PACKETS as u32),
        source_rate: Some(source_rate),
        counters: Some(LinkCounters { total, stream_start: start, stream_starts: starts }),
        iso,
    };
    (sample, total)
}

/// Link samples while a UI subscribes; every second the stream's rates (the
/// flow's Audio step) and the receiver test's state; every two seconds the
/// HCI capture's size
async fn sampler(state: Arc<CoreState>) {
    let stats = state.service.stats();
    let mut tick = tokio::time::interval(SAMPLE);
    let mut last_link: Option<(Instant, LinkTotals)> = None;
    let mut last_rates: Option<(Instant, LinkTotals)> = None;
    let mut n: u64 = 0;
    loop {
        tick.tick().await;
        n += 1;
        let now = Instant::now();
        if n.is_multiple_of(RATE_TICKS) {
            let (total, _, _) = crate::state::link_totals(&stats);
            if let Some((t, prev)) = last_rates {
                let d = total.since(&prev);
                let secs = now.duration_since(t).as_secs_f64();
                state.set_rates(Some(bluecaster_core::link::audio_rates(d.packets, d.bytes, d.capture_frames, secs)));
            }
            last_rates = Some((now, total));
            state.publish_receiver();
        }
        if n.is_multiple_of(CAPTURE_TICKS) {
            state.refresh_capture();
            state.expire_verbose();
        }
        // ABR: the bit rate and SBC bitpool as they move
        state.apply_live_codec(stats.live_codec());
        if !state.link_subscribed() {
            last_link = None;
            continue;
        }
        let (source_rate, le_cis) = state.with_snapshot(|s| match (&s.stream.state, &s.stream.info) {
            (ipc::StreamState::Streaming, Some(i)) => (i.source_sample_rate, i.le.as_ref().map(|l| l.cis)),
            _ => (0, None),
        });
        // LE Audio (experimental): the sender's counters
        let iso = le_cis.and_then(|cis| state.service.host().map(|h| iso_counters(cis, &h.le_stream_stats())));
        let time_ms = state.started.elapsed().as_millis() as u64;
        let (sample, total) = link_sample(time_ms, &stats, last_link, now, source_rate, iso);
        last_link = Some((now, total));
        state.publish(Event::Link(sample));
    }
}

/// How often the core looks whether v1 runs
const V1_CHECK: Duration = Duration::from_secs(3);

/// How long the start waits for v1 to exit when asked
const V1_EXIT_WAIT: Duration = Duration::from_secs(10);

/// v2 replaces v1 like a newer version: a running v1 is asked to exit (and
/// waited for, so that the adapter is free), then v1's mutex is held so
/// that a v1 started later quits at once. None if v1 still runs (it did not
/// exit, or runs elevated): [`watch_v1`] then takes the mutex once it exits.
fn replace_v1() -> Option<win_sys::single_instance::SingleInstance> {
    use win_sys::single_instance::{SingleInstance, V1_MUTEX_NAME, ask_v1_to_exit, is_held};
    if is_held(V1_MUTEX_NAME) {
        let asked = ask_v1_to_exit();
        tracing::info!("A2DPWB v1 runs: asked {asked} window(s) to exit");
        let until = Instant::now() + V1_EXIT_WAIT;
        while is_held(V1_MUTEX_NAME) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    match SingleInstance::acquire(V1_MUTEX_NAME) {
        Ok(Some(g)) => Some(g),
        Ok(None) => {
            tracing::warn!("A2DPWB v1 still runs: it may hold the Bluetooth adapter");
            None
        }
        Err(e) => {
            tracing::warn!("v1 mutex: {e}");
            None
        }
    }
}

/// How long the start waits for a 2.0.x core to exit when asked (it stops
/// its stream and releases the adapter first)
const OLD_CORE_EXIT_WAIT: Duration = Duration::from_secs(8);

/// How often the core tries to take the 2.0.x core's mutex while it could
/// not (the old core did not exit when asked)
const OLD_CORE_CHECK: Duration = Duration::from_secs(5);

/// A 2.0.x core (A2DPWB) that runs is asked to exit, like `--quit` does,
/// and waited for; then its mutex is held too, so that a 2.0.x core started
/// later (an old shortcut) quits at once and its UI talks to this core over
/// the old pipe name. None if it still runs: [`hold_old_core_mutex`] takes
/// the mutex once it has exited.
fn replace_old_core() -> Option<win_sys::single_instance::SingleInstance> {
    crate::quit::stop_old_core(OLD_CORE_EXIT_WAIT);
    match win_sys::single_instance::SingleInstance::acquire(&ipc::old_core_mutex_name()) {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!("2.0.x core mutex: {e}");
            None
        }
    }
}

/// Takes the 2.0.x core's mutex once it is free and holds it from then on
async fn hold_old_core_mutex() {
    let name = ipc::old_core_mutex_name();
    let mut tick = tokio::time::interval(OLD_CORE_CHECK);
    loop {
        tick.tick().await;
        if let Ok(Some(guard)) = win_sys::single_instance::SingleInstance::acquire(&name) {
            tracing::info!("the 2.0.x core has exited: {name} is held now");
            std::future::pending::<()>().await;
            drop(guard);
        }
    }
}

/// While v1 still runs (it did not exit when asked): the UI asks the user to
/// exit it. Once it has exited, v1's mutex is taken and held from then on.
async fn watch_v1(state: Arc<CoreState>) {
    use win_sys::single_instance::{SingleInstance, V1_MUTEX_NAME};
    let mut tick = tokio::time::interval(V1_CHECK);
    loop {
        tick.tick().await;
        if let Ok(Some(guard)) = SingleInstance::acquire(V1_MUTEX_NAME) {
            state.set_v1_running(false);
            // Held for as long as the core runs
            std::future::pending::<()>().await;
            drop(guard);
        }
        state.set_v1_running(true);
    }
}

/// Shows a balloon from the core (tray thread)
fn show_balloon(notifier: &TrayNotifier, b: BalloonRequest) {
    let balloon =
        Balloon { title: b.title, text: b.text, icon: BalloonIcon::Info, on_click: b.url.map(ClickAction::OpenUrl) };
    if let Err(e) = notifier.show(balloon) {
        tracing::warn!("tray balloon: {e}");
    }
}

pub fn run(args: &[String]) -> i32 {
    let background = args.iter().any(|a| a == "--background" || a == "--minimized");
    let _instance = match win_sys::single_instance::SingleInstance::acquire(&ipc::core_mutex_name()) {
        Ok(Some(g)) => g,
        Ok(None) => {
            // Already running: a start from the shortcut opens the UI
            tracing::info!("the core runs already");
            if !background {
                launch_ui(false);
            }
            return 0;
        }
        Err(e) => {
            tracing::error!("single instance check failed: {e}");
            return 1;
        }
    };
    // Before the service starts, so that a 2.0.x core has released the
    // adapter
    let old_core_guard = replace_old_core();
    let holds_old_core_mutex = old_core_guard.is_some();
    // Before the service starts, so that v1 has released the adapter. A
    // development instance (BLUECASTER_INSTANCE) leaves v1 alone.
    let replaces_v1 = ipc::instance_suffix().is_empty();
    let v1_guard = if replaces_v1 { replace_v1() } else { None };
    let watches_v1 = replaces_v1 && v1_guard.is_none();
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let (tx, rx) = mpsc::channel();
    let capture = HciCapture::new();
    let service = Arc::new(Service::new(
        ServiceOptions {
            config_dir: bluecaster_core::paths::config_dir(),
            hci_tcp: None,
            capture,
            afh: None,
            usb: Default::default(),
        },
        tx,
    ));
    let state = {
        let (p1, p2, p3) = (proxy.clone(), proxy.clone(), proxy.clone());
        CoreState::new(
            service.clone(),
            Box::new(move |v| {
                let _ = p1.send_event(UserEvent::Tray(v));
            }),
            Box::new(move || {
                let _ = p2.send_event(UserEvent::Quit);
            }),
            Box::new(move |b| {
                let _ = p3.send_event(UserEvent::Balloon(b));
            }),
        )
    };
    // A missing Run value, v1's, or one naming a copy of the core that is gone
    state.refresh_autostart(replaces_v1);
    {
        let state = state.clone();
        std::thread::Builder::new()
            .name("service-events".into())
            .spawn(move || {
                while let Ok(e) = rx.recv() {
                    state.on_service_event(e);
                }
            })
            .expect("spawning the service event thread");
    }
    {
        let state = state.clone();
        let quit = proxy.clone();
        std::thread::Builder::new()
            .name("ipc".into())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
                    Ok(rt) => rt,
                    Err(e) => {
                        tracing::error!("IPC runtime: {e}");
                        return;
                    }
                };
                rt.block_on(async move {
                    tokio::spawn(sampler(state.clone()));
                    if watches_v1 {
                        tokio::spawn(watch_v1(state.clone()));
                    }
                    if !holds_old_core_mutex {
                        tokio::spawn(hold_old_core_mutex());
                    }
                    let (name, old_name) = match win_sys::pipe::current_user_sid() {
                        Ok(sid) => (ipc::pipe_name(&sid), ipc::old_pipe_name(&sid)),
                        Err(e) => {
                            tracing::error!("cannot read the user SID: {e}");
                            return;
                        }
                    };
                    if let Err(e) = crate::server::serve(state, name, old_name).await {
                        tracing::error!("IPC server: {e}");
                        let _ = quit.send_event(UserEvent::Quit);
                    }
                });
            })
            .expect("spawning the IPC thread");
    }
    {
        let p = proxy.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let _ = p.send_event(UserEvent::Menu(e.id.0));
        }));
        let p = proxy.clone();
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                let _ = p.send_event(UserEvent::Click);
            }
        }));
    }
    state.check_update_if_due();
    if !background {
        launch_ui(false);
    }

    let mut tray: Option<TrayIcon> = None;
    // Balloons from the tray icon; on this thread (not Send)
    let mut notifier: Option<TrayNotifier> = None;
    let mut view: Option<TrayView> = None;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            TaoEvent::NewEvents(StartCause::Init) => {
                let v = view.clone().unwrap_or(TrayView {
                    streaming: false,
                    tooltip: "BlueCaster".into(),
                    profiles: Vec::new(),
                    labels: Vec::new(),
                    active_profile: None,
                    language: "en".into(),
                });
                let icon = tray_icon::Icon::from_resource(101, None).ok();
                let mut b = TrayIconBuilder::new()
                    .with_menu(Box::new(build_menu(&v)))
                    .with_menu_on_left_click(false)
                    .with_tooltip(&v.tooltip);
                if let Some(i) = icon {
                    b = b.with_icon(i);
                }
                match b.build() {
                    Ok(t) => {
                        notifier = Some(TrayNotifier::new(t.window_handle() as isize));
                        tray = Some(t);
                    }
                    Err(e) => tracing::error!("tray icon: {e}"),
                }
            }
            TaoEvent::UserEvent(UserEvent::Tray(v)) => {
                if let Some(t) = &tray {
                    t.set_menu(Some(Box::new(build_menu(&v))));
                    let _ = t.set_tooltip(Some(&v.tooltip));
                }
                view = Some(v);
            }
            TaoEvent::UserEvent(UserEvent::Click) => {
                win_sys::notify::allow_foreground_for_click();
                // The flyout, or the main window when Settings say so
                launch_ui(!state.tray_click_opens_window());
            }
            TaoEvent::UserEvent(UserEvent::Balloon(b)) => match &notifier {
                Some(n) => show_balloon(n, b),
                None => tracing::info!("no tray icon for the balloon \"{}\"", b.title),
            },
            TaoEvent::UserEvent(UserEvent::Menu(id)) => match id.as_str() {
                "open" => launch_ui(false),
                "disconnect" => state.stop(),
                "exit" => state.quit(),
                other => {
                    if let Some(i) = other.strip_prefix("profile:").and_then(|i| i.parse::<usize>().ok()) {
                        let name = view.as_ref().and_then(|v| v.profiles.get(i).cloned());
                        if let Some(name) = name
                            && let Err(e) = state.start_profile(&name)
                        {
                            tracing::warn!("tray: cannot start {name}: {}", e.code);
                        }
                    }
                }
            },
            TaoEvent::UserEvent(UserEvent::Quit) => {
                tracing::info!("exiting");
                notifier = None;
                tray = None;
                // Streaming stops and the adapter is released before exit
                service.shutdown();
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_samples_from_the_counters() {
        let stats = bt_host::LinkStats::default();
        let r = Ordering::Relaxed;
        let t0 = Instant::now();
        let (first, total) = link_sample(0, &stats, None, t0, 48000, None);
        assert_eq!(first.packets_per_second, 0.0);
        assert!(first.acl_packet.is_none());
        assert_eq!(first.queue_capacity, Some(64));
        // One second later: 120 packets of 1005 bytes, 2 dropped, after a
        // stream that started at 10 packets
        stats.start_packets.store(10, r);
        stats.stream_starts.store(1, r);
        stats.packets_sent.store(120, r);
        stats.bytes_sent.store(120 * 1005, r);
        stats.queue_drops.store(2, r);
        let (s, _) = link_sample(1000, &stats, Some((t0, total)), t0 + Duration::from_secs(1), 48000, None);
        assert!((s.packets_per_second - 120.0).abs() < 1e-6);
        assert_eq!(s.acl_packet, Some(AclPacket { name: "3-DH5".into(), slots: 6 }));
        assert!((s.avg_packet_bytes.unwrap() - 1005.0).abs() < 1e-6);
        assert!((s.airtime - 120.0 * 6.0 * 625e-6).abs() < 1e-9);
        assert_eq!(s.dropped, 2);
        let c = s.counters.unwrap();
        assert_eq!((c.total.packets, c.stream_start.packets, c.stream_starts), (120, 10, 1));
        assert_eq!(s.source_rate, Some(48000));
        assert!(s.iso.is_none());
    }

    #[test]
    fn le_audio_samples_carry_the_iso_counters() {
        let stats = bt_host::LinkStats::default();
        let r = Ordering::Relaxed;
        let t0 = Instant::now();
        let (_, total) = link_sample(0, &stats, None, t0, 48000, None);
        stats.packets_sent.store(120, r);
        stats.bytes_sent.store(120 * 1005, r);
        stats.queue_depth.store(5, r);
        let le = bt_host::LeStreamStats {
            sets_encoded: 990,
            sets_sent: 1000,
            sdus_sent: 2000,
            silence_sets: 12,
            underruns: 1,
            overflows: 3,
            send_errors: 0,
            prefill: 6,
        };
        let iso = Some(iso_counters(2, &le));
        let (s, _) = link_sample(1000, &stats, Some((t0, total)), t0 + Duration::from_secs(1), 48000, iso);
        assert_eq!(
            s.iso,
            Some(IsoCounters { cis: 2, sdu_sets: 1000, silence: 12, underruns: 1, dropped: 3, prefill: 6 })
        );
        // No A2DP media figures while LE Audio streams
        assert_eq!((s.bytes_per_second, s.packets_per_second, s.airtime, s.queue_depth), (0.0, 0.0, 0.0, 0));
        assert!(s.acl_packet.is_none() && s.queue_capacity.is_none());
        assert_eq!(s.source_rate, Some(48000));
    }
}
