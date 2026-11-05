//! Link arithmetic for the link quality view and the diagnostics — v1's
//! wx_link_quality_dialog.cpp: which ACL packet type the media packets take
//! (on an EDR 3 Mbps link, or on the fastest the remote device's features
//! allow once known), the air time and bandwidth share that means, the
//! rates of the "Audio sending" step, and what the controller's backlog
//! holds in time.

use bt_host::{LinkDetails, LinkMode, RemoteFeatures};

use crate::diag::AudioRates;

/// Bluetooth BR/EDR baseband slot
pub const SLOT_SECONDS: f64 = 625e-6;
/// Each L2CAP payload gets a 4-byte L2CAP header in the ACL packet
pub const L2CAP_HEADER_BYTES: f64 = 4.0;

/// An EDR 3 Mbps ACL packet type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AclType {
    pub name: &'static str,
    pub max_bytes: u32,
    pub slots: u32,
}

pub const EDR3_TYPES: [AclType; 3] = [
    AclType { name: "3-DH1", max_bytes: 83, slots: 1 },
    AclType { name: "3-DH3", max_bytes: 552, slots: 3 },
    AclType { name: "3-DH5", max_bytes: 1021, slots: 5 },
];

pub const EDR2_TYPES: [AclType; 3] = [
    AclType { name: "2-DH1", max_bytes: 54, slots: 1 },
    AclType { name: "2-DH3", max_bytes: 367, slots: 3 },
    AclType { name: "2-DH5", max_bytes: 679, slots: 5 },
];

/// Basic rate
pub const BR_TYPES: [AclType; 3] = [
    AclType { name: "DH1", max_bytes: 27, slots: 1 },
    AclType { name: "DH3", max_bytes: 183, slots: 3 },
    AclType { name: "DH5", max_bytes: 339, slots: 5 },
];

/// The packet types the remote device's features allow (EDR 3 Mbps up to
/// 5 slots while they are not known)
pub fn acl_types(features: Option<&RemoteFeatures>) -> &'static [AclType] {
    let Some(f) = features else { return &EDR3_TYPES };
    let (table, five, three) = if f.edr_3m() {
        (&EDR3_TYPES, f.edr_five_slot(), f.edr_three_slot())
    } else if f.edr_2m() {
        (&EDR2_TYPES, f.edr_five_slot(), f.edr_three_slot())
    } else {
        (&BR_TYPES, f.five_slot(), f.three_slot())
    };
    let n = if five {
        3
    } else if three {
        2
    } else {
        1
    };
    &table[..n]
}

fn rate_kbps(t: AclType) -> f64 {
    t.max_bytes as f64 * 8.0 / ((t.slots + 1) as f64 * SLOT_SECONDS) / 1000.0
}

/// 1021 bytes every 6 slots (3-DH5 and the reply slot): 2178 kbps
pub fn theoretical_max_kbps() -> f64 {
    rate_kbps(EDR3_TYPES[2])
}

/// Slots one ACL payload of `acl_bytes` takes on air, the receiver's reply
/// slot included, and the packet type used (several 3-DH5 packets when it
/// is larger than one carries)
pub fn acl_slots(acl_bytes: u32) -> (&'static str, u32) {
    acl_slots_in(&EDR3_TYPES, acl_bytes)
}

/// [`acl_slots`] with the packet types of `types` (smallest first)
pub fn acl_slots_in(types: &[AclType], acl_bytes: u32) -> (&'static str, u32) {
    for t in types {
        if acl_bytes <= t.max_bytes {
            return (t.name, t.slots + 1);
        }
    }
    let largest = types[types.len() - 1];
    let n = acl_bytes.div_ceil(largest.max_bytes);
    (largest.name, n * (largest.slots + 1))
}

/// What the media stream takes of the link
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Airtime {
    /// Media packet size (L2CAP payload)
    pub avg_packet_bytes: f64,
    pub packet: &'static str,
    pub slots: u32,
    /// Share of the air time (0..1, more when the link cannot carry it)
    pub airtime: f64,
    /// Share of the link's usable rate (0..1, L2CAP headers included)
    pub bandwidth_share: f64,
    /// The packet types come from the remote device's features (else EDR
    /// 3 Mbps is assumed)
    pub from_features: bool,
}

/// The air time of `packets_per_second` packets carrying
/// `bytes_per_second` (L2CAP payload) on an EDR 3 Mbps link; None while
/// nothing is sent
pub fn airtime(packets_per_second: f64, bytes_per_second: f64) -> Option<Airtime> {
    airtime_on(packets_per_second, bytes_per_second, None)
}

/// [`airtime`] on the packet types the remote device's `features` allow
pub fn airtime_on(
    packets_per_second: f64,
    bytes_per_second: f64,
    features: Option<&RemoteFeatures>,
) -> Option<Airtime> {
    if packets_per_second <= 0.0 || !packets_per_second.is_finite() || !bytes_per_second.is_finite() {
        return None;
    }
    let types = acl_types(features);
    let avg = bytes_per_second / packets_per_second;
    let acl_bytes = avg + L2CAP_HEADER_BYTES;
    let acl_kbps = packets_per_second * acl_bytes * 8.0 / 1000.0;
    let (packet, slots) = acl_slots_in(types, (acl_bytes + 0.5) as u32);
    Some(Airtime {
        avg_packet_bytes: avg,
        packet,
        slots,
        airtime: packets_per_second * slots as f64 * SLOT_SECONDS,
        bandwidth_share: acl_kbps / rate_kbps(types[types.len() - 1]),
        from_features: features.is_some(),
    })
}

/// What `backlog` ACL packets in the controller hold in time (ms) when the
/// stream hands it `acl_per_second` of them; None while nothing is sent
pub fn backlog_ms(backlog: u64, acl_per_second: f64) -> Option<f64> {
    (acl_per_second > 0.0 && acl_per_second.is_finite()).then(|| backlog as f64 * 1000.0 / acl_per_second)
}

/// A link supervision timeout or sniff interval in slots, in ms
pub fn slots_ms(slots: u16) -> f64 {
    slots as f64 * SLOT_SECONDS * 1000.0
}

/// The remote device's chip in a line: "Airoha (0x0094), Bluetooth 5.3, LMP
/// subversion 0x1234; EDR 2/3 Mbps, up to 3-DH5". None while nothing is
/// known.
pub fn remote_chip_text(d: &LinkDetails) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(v) = d.remote_version {
        let company = match bt_host::chips::company_name(v.company_id) {
            Some(n) => format!("{n} (0x{:04X})", v.company_id),
            None => format!("company 0x{:04X}", v.company_id),
        };
        let version = crate::adapter::bt_core_version(v.lmp_version)
            .map(|c| format!("Bluetooth {c}"))
            .unwrap_or_else(|| format!("LMP version {}", v.lmp_version));
        parts.push(format!("{company}, {version}, LMP subversion 0x{:04X}", v.lmp_subversion));
    }
    if let Some(f) = &d.remote_features {
        let edr = match (f.edr_2m(), f.edr_3m()) {
            (true, true) => "EDR 2/3 Mbps",
            (true, false) => "EDR 2 Mbps",
            (false, true) => "EDR 3 Mbps",
            (false, false) => "no EDR",
        };
        parts.push(format!("{edr}, up to {}", bt_host::link_details::best_packet(f)));
    }
    (!parts.is_empty()).then(|| parts.join("; "))
}

/// Role, mode, key size and supervision timeout in a line: "central,
/// active, key 16 bytes, supervision timeout 5000 ms". None without a link.
pub fn link_state_text(d: &LinkDetails) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(c) = d.central {
        parts.push(if c { "central" } else { "peripheral" }.to_string());
    }
    match (d.mode, d.sniff_interval_slots) {
        (Some(LinkMode::Sniff), Some(i)) => parts.push(format!("sniff ({:.1} ms interval)", slots_ms(i))),
        (Some(m), _) => parts.push(format!("{m:?}").to_lowercase()),
        (None, _) => {}
    }
    if let Some(k) = d.encryption_key_size {
        parts.push(format!("key {k} bytes"));
    }
    if let Some(t) = d.supervision_timeout_slots {
        parts.push(format!("supervision timeout {:.0} ms", slots_ms(t)));
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// The stream's rates from counter deltas over `seconds`
pub fn audio_rates(packets: u64, bytes: u64, capture_frames: u64, seconds: f64) -> AudioRates {
    if seconds <= 0.0 {
        return AudioRates::default();
    }
    AudioRates {
        packets_per_second: packets as f64 / seconds,
        kbps: bytes as f64 * 8.0 / seconds / 1000.0,
        capture_frames_per_second: capture_frames as f64 / seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_types_by_size() {
        assert_eq!(acl_slots(83), ("3-DH1", 2));
        assert_eq!(acl_slots(84), ("3-DH3", 4));
        assert_eq!(acl_slots(552), ("3-DH3", 4));
        assert_eq!(acl_slots(1021), ("3-DH5", 6));
        // Two 3-DH5 packets
        assert_eq!(acl_slots(1022), ("3-DH5", 12));
    }

    #[test]
    fn theoretical_rate_is_v1s() {
        assert!((theoretical_max_kbps() - 2178.13).abs() < 0.01);
    }

    #[test]
    fn ldac_990_airtime() {
        // LDAC 990 kbps at 96 kHz in 1005-byte packets: about 123 a second
        let pps = 123.0;
        let a = airtime(pps, pps * 1005.0).unwrap();
        assert_eq!(a.packet, "3-DH5");
        assert_eq!(a.slots, 6);
        assert!((a.airtime - pps * 6.0 * SLOT_SECONDS).abs() < 1e-9);
        assert!((a.avg_packet_bytes - 1005.0).abs() < 1e-9);
        assert!(a.bandwidth_share > 0.45 && a.bandwidth_share < 0.47, "{}", a.bandwidth_share);
        assert!(airtime(0.0, 0.0).is_none());
    }

    #[test]
    fn packet_types_from_the_remote_features() {
        let f = |b0: u8, b3: u8, b4: u8, b5: u8| RemoteFeatures([b0, 0, 0, b3, b4, b5, 0, 0]);
        // EDR 2 Mbps only, 5-slot: an LDAC 990 packet takes two 2-DH5
        let edr2 = f(0x03, 0x02, 0x80, 0x01);
        let a = airtime_on(123.0, 123.0 * 1005.0, Some(&edr2)).unwrap();
        assert_eq!((a.packet, a.slots, a.from_features), ("2-DH5", 12, true));
        assert!(a.bandwidth_share > 0.6, "{}", a.bandwidth_share);
        // No 5-slot EDR packets: 3-DH3 at most
        assert_eq!(acl_slots_in(acl_types(Some(&f(0x03, 0x06, 0x80, 0x00))), 600), ("3-DH3", 8));
        // Basic rate only
        assert_eq!(acl_slots_in(acl_types(Some(&f(0x03, 0, 0, 0))), 300), ("DH5", 6));
        // Not known: EDR 3 Mbps as before
        assert!(!airtime(123.0, 123.0 * 1005.0).unwrap().from_features);
    }

    #[test]
    fn backlog_in_time() {
        // 8 ACL packets at 200 a second hold 40 ms
        assert_eq!(backlog_ms(8, 200.0), Some(40.0));
        assert_eq!(backlog_ms(8, 0.0), None);
        assert_eq!(slots_ms(8000), 5000.0);
    }

    #[test]
    fn link_detail_lines() {
        use bt_host::RemoteVersion;
        let mut d = LinkDetails::default();
        assert_eq!(remote_chip_text(&d), None);
        assert_eq!(link_state_text(&d), None);
        d.remote_version = Some(RemoteVersion { lmp_version: 12, company_id: 0x0094, lmp_subversion: 0x1234 });
        d.remote_features = Some(RemoteFeatures([0x03, 0, 0, 0x06, 0x80, 0x01, 0, 0]));
        assert_eq!(
            remote_chip_text(&d).unwrap(),
            "Airoha (0x0094), Bluetooth 5.3, LMP subversion 0x1234; EDR 2/3 Mbps, up to 3-DH5"
        );
        d.remote_version = Some(RemoteVersion { lmp_version: 40, company_id: 0xBEEF, lmp_subversion: 1 });
        assert!(remote_chip_text(&d).unwrap().starts_with("company 0xBEEF, LMP version 40"));
        d.central = Some(false);
        d.mode = Some(LinkMode::Sniff);
        d.sniff_interval_slots = Some(800);
        d.encryption_key_size = Some(16);
        d.supervision_timeout_slots = Some(8000);
        assert_eq!(
            link_state_text(&d).unwrap(),
            "peripheral, sniff (500.0 ms interval), key 16 bytes, supervision timeout 5000 ms"
        );
    }

    #[test]
    fn rates_over_an_interval() {
        let r = audio_rates(250, 125_000, 96_000, 2.0);
        assert_eq!(r.packets_per_second, 125.0);
        assert_eq!(r.kbps, 500.0);
        assert_eq!(r.capture_frames_per_second, 48_000.0);
        assert_eq!(audio_rates(1, 1, 1, 0.0), AudioRates::default());
    }
}
