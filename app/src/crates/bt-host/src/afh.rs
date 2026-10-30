//! AFH host channel classification from the Wi-Fi around (v1 afh.cpp)
//!
//! Bluetooth BR/EDR hops over 79 channels (2402-2480 MHz). With Adaptive
//! Frequency Hopping the central's controller leaves out channels it finds
//! bad; the host may help with HCI Set AFH Host Channel Classification,
//! marking channels it knows to be busy. BlueCaster is the central of its links,
//! so what it passes applies to headphones and receivers alike.
//!
//! Policies:
//!   "auto"   the 2.4 GHz Wi-Fi channel this PC is connected on is marked
//!            bad; nothing is passed when the PC is not on 2.4 GHz Wi-Fi
//!   "off"    nothing is passed (the controller decides alone)
//!   "6,11"   these Wi-Fi channels are marked bad
//!
//! "auto" is conservative on purpose. v1 and early v2 avoided every access
//! point heard at -75 dBm or louder; in a hardware test (2026-10-17) that
//! took a neighbour's channel 11 while the PC's own Wi-Fi was disconnected,
//! left 56 of 79 channels, and radio silences over 40 ms went from about 6
//! to 30-95 a minute. Access points the PC only hears are left to the
//! controller's own AFH; only the PC's own Wi-Fi, which shares the antenna
//! area and transmits whenever the PC does, is avoided.
//!
//! At least `MIN_CHANNELS` channels stay usable, as the specification
//! requires: channels later in the list are dropped first.
//!
//! Plain functions over plain data: the connected channels come from
//! win-sys::wlan.

/// Bluetooth BR/EDR channels
pub const CHANNELS: usize = 79;
/// The fewest channels the specification allows AFH to keep (Nmin = 20)
pub const MIN_CHANNELS: usize = 20;
/// A 20 MHz Wi-Fi signal (22 MHz for 802.11b) spreads +-11 MHz around its
/// center
const WIFI_HALF_WIDTH_MHZ: i32 = 11;

/// What to pass to the controller
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classification {
    /// Bit n = Bluetooth channel n: 1 unknown (usable), 0 bad. Bit 79 is
    /// reserved and always 0.
    pub map: [u8; 10],
    /// Channels left usable
    pub usable: usize,
    /// Wi-Fi channels marked bad, in the order they were taken
    pub avoided_wifi: Vec<u8>,
    /// Something to pass to the controller (otherwise leave it alone)
    pub passed: bool,
}

impl Classification {
    /// Every channel usable, nothing to pass
    pub fn all_usable() -> Classification {
        Classification { map: all_usable_map(), usable: CHANNELS, avoided_wifi: Vec::new(), passed: false }
    }
}

/// A parsed policy string
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Policy {
    Off,
    Auto,
    /// Wi-Fi channels as written (out of range ones are skipped later)
    Manual(Vec<i64>),
}

impl Policy {
    /// "" and "off" are Off, "auto" is Auto, anything else is a channel
    /// list: every number in it counts, other characters only separate
    /// (as v1's strtol loop reads it, so "6, 11" and "6;11" both work)
    pub fn parse(policy: &str) -> Policy {
        match policy {
            "" | "off" => Policy::Off,
            "auto" => Policy::Auto,
            _ => Policy::Manual(parse_numbers(policy)),
        }
    }
}

/// Classification for a policy string. `connected` is called only for
/// "auto" and returns the 2.4 GHz channels (1-14) this PC's Wi-Fi
/// interfaces are connected on: empty when it is disconnected, on 5 / 6
/// GHz, or has no Wi-Fi (win-sys::wlan; it scans nothing itself).
pub fn classify(policy: &str, connected: impl FnOnce() -> Vec<u8>) -> Classification {
    match Policy::parse(policy) {
        Policy::Off => Classification::all_usable(),
        Policy::Auto => {
            let connected = connected();
            tracing::debug!("AFH auto: PC Wi-Fi connected on 2.4 GHz channels {connected:?}");
            avoid_wifi(&auto_channels(&connected))
        }
        Policy::Manual(channels) => avoid_wifi(&channels),
    }
}

/// The Wi-Fi channels "auto" avoids: the 2.4 GHz channels this PC is
/// connected on, in interface order, nothing else. Access points the PC
/// only hears, however loud, are not avoided.
pub fn auto_channels(connected: &[u8]) -> Vec<i64> {
    connected.iter().map(|&ch| i64::from(ch)).collect()
}

/// Classification that avoids these Wi-Fi channels in order, skipping
/// those out of range (1-14), repeated, or that would leave fewer than
/// MIN_CHANNELS usable
pub fn avoid_wifi(wifi_channels: &[i64]) -> Classification {
    let mut c = Classification::all_usable();
    for &ch in wifi_channels {
        let Ok(ch) = u8::try_from(ch) else {
            tracing::debug!("AFH: Wi-Fi channel {ch} out of range, skipped");
            continue;
        };
        if !(1..=14).contains(&ch) || c.avoided_wifi.contains(&ch) {
            tracing::debug!("AFH: Wi-Fi channel {ch} out of range or repeated, skipped");
            continue;
        }
        let mut trial = c.map;
        mark_bad(&mut trial, ch);
        if count(&trial) < MIN_CHANNELS {
            tracing::debug!("AFH: avoiding Wi-Fi channel {ch} would leave {} channels, skipped", count(&trial));
            continue; // would leave too few
        }
        c.map = trial;
        c.avoided_wifi.push(ch);
    }
    c.usable = count(&c.map);
    c.passed = !c.avoided_wifi.is_empty();
    c
}

/// Channels set in a 79-bit map (bit 79 is not counted)
pub fn count(map: &[u8; 10]) -> usize {
    (0..CHANNELS).filter(|&n| map[n / 8] & (1 << (n % 8)) != 0).count()
}

/// "6, 11"
pub fn describe(wifi_channels: &[u8]) -> String {
    wifi_channels.iter().map(u8::to_string).collect::<Vec<_>>().join(", ")
}

/// Center frequency of a 2.4 GHz Wi-Fi channel
pub fn wifi_center_mhz(channel: u8) -> i32 {
    if channel == 14 { 2484 } else { 2407 + 5 * i32::from(channel) }
}

fn all_usable_map() -> [u8; 10] {
    let mut map = [0xFF; 10];
    map[9] = 0x7F; // bit 79 is reserved
    map
}

/// Clears the Bluetooth channels within the Wi-Fi channel's width
fn mark_bad(map: &mut [u8; 10], wifi_channel: u8) {
    let center = wifi_center_mhz(wifi_channel);
    for n in 0..CHANNELS {
        let f = 2402 + n as i32;
        if (f - center).abs() <= WIFI_HALF_WIDTH_MHZ {
            map[n / 8] &= !(1 << (n % 8));
        }
    }
}

/// Every integer in `s`, as C's strtol loop reads them: optional leading
/// whitespace and sign, then digits; any other character is skipped
fn parse_numbers(s: &str) -> Vec<i64> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut p = 0;
    while p < b.len() {
        let mut q = p;
        while q < b.len() && b[q].is_ascii_whitespace() {
            q += 1;
        }
        let negative = q < b.len() && b[q] == b'-';
        if q < b.len() && (b[q] == b'-' || b[q] == b'+') {
            q += 1;
        }
        let digits_start = q;
        let mut value: i64 = 0;
        while q < b.len() && b[q].is_ascii_digit() {
            value = value.saturating_mul(10).saturating_add(i64::from(b[q] - b'0'));
            q += 1;
        }
        if q == digits_start {
            p += 1; // no number here
            continue;
        }
        out.push(if negative { -value } else { value });
        p = q;
    }
    out
}
