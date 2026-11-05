//! AFH host channel classification from the Wi-Fi around (v1 afh.cpp)
//!
//! Bluetooth BR/EDR hops over 79 channels (2402-2480 MHz). With Adaptive
//! Frequency Hopping the central's controller leaves out channels it finds
//! bad; the host may help with HCI Set AFH Host Channel Classification,
//! marking channels it knows to be busy. A2DPWB is the central of its links,
//! so what it passes applies to headphones and receivers alike.
//!
//! Policies:
//!   "auto"   the 2.4 GHz Wi-Fi channel this PC is connected on is marked
//!            bad; nothing is passed when the PC is not on 2.4 GHz Wi-Fi
//!   "off"    nothing is passed (the controller decides alone)
//!   "6,11"   these Wi-Fi channels are marked bad
//!   "blend-mild" / "blend-medium" / "blend-strong"
//!            experimental, developer mode only: what "auto" avoids, plus
//!            channels where the access points heard and the controller's
//!            own channel map point at Wi-Fi (see `blend`)
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

use std::collections::VecDeque;

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
    /// Experimental: Wi-Fi scan and controller map combined (`blend`)
    Blend(Strictness),
}

impl Policy {
    /// "" and "off" are Off, "auto" is Auto, anything else is a channel
    /// list: every number in it counts, other characters only separate
    /// (as v1's strtol loop reads it, so "6, 11" and "6;11" both work)
    pub fn parse(policy: &str) -> Policy {
        match policy {
            "" | "off" => Policy::Off,
            "auto" => Policy::Auto,
            "blend-mild" => Policy::Blend(Strictness::Mild),
            "blend-medium" => Policy::Blend(Strictness::Medium),
            "blend-strong" => Policy::Blend(Strictness::Strong),
            _ => Policy::Manual(parse_numbers(policy)),
        }
    }
}

/// The policy a stack runs with: "blend-*" is a developer-mode experiment,
/// so without developer mode it runs as "auto"
pub fn effective_policy(policy: &str, developer: bool) -> &str {
    match Policy::parse(policy) {
        Policy::Blend(_) if !developer => "auto",
        _ => policy,
    }
}

/// Classification for a policy string. `connected` is called only for
/// "auto" (and "blend-*", which has no other inputs here and runs as
/// "auto": `blend` is the one for it) and returns the 2.4 GHz channels (1-14) this PC's Wi-Fi
/// interfaces are connected on: empty when it is disconnected, on 5 / 6
/// GHz, or has no Wi-Fi (win-sys::wlan; it scans nothing itself).
pub fn classify(policy: &str, connected: impl FnOnce() -> Vec<u8>) -> Classification {
    match Policy::parse(policy) {
        Policy::Off => Classification::all_usable(),
        Policy::Auto | Policy::Blend(_) => {
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

/// The channels a map leaves out, as ranges: "0-11, 40, 60-78"
pub fn unused_ranges(map: &[u8; 10]) -> String {
    let unused = |n: usize| map[n / 8] & (1 << (n % 8)) == 0;
    let mut out = Vec::new();
    let mut n = 0;
    while n < CHANNELS {
        if !unused(n) {
            n += 1;
            continue;
        }
        let start = n;
        while n + 1 < CHANNELS && unused(n + 1) {
            n += 1;
        }
        out.push(if start == n { start.to_string() } else { format!("{start}-{n}") });
        n += 1;
    }
    out.join(", ")
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

// ======================================================================
// Blend (experimental, developer mode)
// ======================================================================
//
// "auto" knows only the PC's own Wi-Fi channel. Blend adds two signals and
// marks a Bluetooth channel bad where they agree enough:
//
//   Wi-Fi       the access points a scan heard within +-11 MHz of the
//               channel, their power summed: -90 dBm or less scores 0,
//               -50 dBm or more 1, linear in between. A loud access point
//               may be idle, which is why it does not decide alone (the
//               2026-10-17 test).
//   controller  how often the controller's own AFH map left the channel
//               out in recent readings (0 = always used, 1 = always left
//               out): what the radio actually measured. Channels the host
//               classification itself marked bad say nothing then and are
//               not counted.
//
// The score is the mean of the signals present (one alone counts in full;
// none scores 0). Strictness sets the score a channel needs and how many
// channels stay usable at least, far more than the specification's 20:
//
//   Mild    0.75, keeps 56   (one Wi-Fi channel at most, as "auto")
//   Medium  0.55, keeps 48
//   Strong  0.35, keeps 40
//
// The PC's own 2.4 GHz channel is avoided first exactly as "auto" does
// (blend never avoids less); further channels go highest score first while
// more than the floor remain. A channel marked bad last time stays bad
// down to 0.1 below the threshold, so the map does not flap.

/// How readily "blend" marks a channel bad
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strictness {
    Mild,
    Medium,
    Strong,
}

impl Strictness {
    /// The score a channel needs to be marked bad
    pub fn threshold(self) -> f32 {
        match self {
            Strictness::Mild => 0.75,
            Strictness::Medium => 0.55,
            Strictness::Strong => 0.35,
        }
    }

    /// Channels that stay usable at least (beyond what "auto" avoids)
    pub fn floor(self) -> usize {
        match self {
            Strictness::Mild => 56,
            Strictness::Medium => 48,
            Strictness::Strong => 40,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Strictness::Mild => "mild",
            Strictness::Medium => "medium",
            Strictness::Strong => "strong",
        }
    }
}

/// A channel marked bad last time stays bad down to this much below the
/// threshold
pub const HYSTERESIS: f32 = 0.1;
/// The summed scan level that scores 0
pub const WIFI_QUIET_DBM: f32 = -90.0;
/// The summed scan level that scores 1
pub const WIFI_LOUD_DBM: f32 = -50.0;
/// Observations of a channel kept (about 2 minutes at one map reading
/// every 5 s)
pub const HISTORY_OBSERVATIONS: usize = 24;
/// Readings after which an observation is dropped (about 10 minutes): a
/// channel the host has kept out that long has no controller signal left
pub const HISTORY_MAX_AGE: u32 = 120;
/// Observations a channel needs before its controller signal counts
pub const MIN_OBSERVATIONS: usize = 4;
/// Readings skipped after a new host classification: the controller takes
/// a moment to switch its map
pub const SETTLE_READINGS: u32 = 2;

/// An access point a Wi-Fi scan heard
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApReading {
    /// 1-14
    pub channel: u8,
    /// dBm
    pub rssi: i32,
}

/// The controller's AFH maps (Read AFH Channel Map) over the recent past
#[derive(Debug, Clone)]
pub struct ControllerHistory {
    /// Readings recorded so far (settling ones included)
    seq: u32,
    settle: u32,
    /// Per channel: (reading, left out)
    obs: Vec<VecDeque<(u32, bool)>>,
}

impl Default for ControllerHistory {
    fn default() -> Self {
        ControllerHistory { seq: 0, settle: 0, obs: vec![VecDeque::new(); CHANNELS] }
    }
}

impl ControllerHistory {
    /// One reading: `in_use` is the controller's map (bit set = hops on
    /// it), `host` the host classification in force (None: none passed).
    /// Channels the host marked bad are not observed.
    pub fn record(&mut self, in_use: &[u8; 10], host: Option<&[u8; 10]>) {
        self.seq = self.seq.wrapping_add(1);
        if self.settle > 0 {
            self.settle -= 1;
            return;
        }
        let seq = self.seq;
        for (n, obs) in self.obs.iter_mut().enumerate() {
            if !host.is_some_and(|h| !bit(h, n)) {
                obs.push_back((seq, !bit(in_use, n)));
                if obs.len() > HISTORY_OBSERVATIONS {
                    obs.pop_front();
                }
            }
            while obs.front().is_some_and(|&(s, _)| seq.wrapping_sub(s) >= HISTORY_MAX_AGE) {
                obs.pop_front();
            }
        }
    }

    /// A new host classification went out: skip the next readings
    pub fn host_changed(&mut self) {
        self.settle = SETTLE_READINGS;
    }

    /// Forget everything (a new link)
    pub fn clear(&mut self) {
        *self = ControllerHistory::default();
    }

    /// Share of the recent readings that left channel `n` out; None with
    /// fewer than MIN_OBSERVATIONS
    pub fn left_out(&self, n: usize) -> Option<f32> {
        let recent: Vec<bool> = self.obs[n]
            .iter()
            .filter(|&&(s, _)| self.seq.wrapping_sub(s) < HISTORY_MAX_AGE)
            .map(|&(_, out)| out)
            .collect();
        (recent.len() >= MIN_OBSERVATIONS)
            .then(|| recent.iter().filter(|&&out| out).count() as f32 / recent.len() as f32)
    }

    /// Readings recorded (for the log)
    pub fn readings(&self) -> u32 {
        self.seq
    }
}

/// What "blend" decides from
#[derive(Debug, Clone, Copy)]
pub struct BlendInput<'a> {
    pub strictness: Strictness,
    /// The 2.4 GHz channels the PC's Wi-Fi is connected on (as for "auto")
    pub connected: &'a [u8],
    /// The latest scan; None when there is none (no location permission,
    /// no Wi-Fi). Some(empty) = scanned and heard nothing.
    pub aps: Option<&'a [ApReading]>,
    pub controller: &'a ControllerHistory,
    /// The map blend passed last time (for the hysteresis)
    pub previous: Option<&'a [u8; 10]>,
}

/// One channel's signals
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelScore {
    pub wifi: Option<f32>,
    pub controller: Option<f32>,
    /// Within the PC's own connected 2.4 GHz channel
    pub own_wifi: bool,
    /// Mean of the signals present, 0..1
    pub score: f32,
}

/// Summed level of the access points within +-11 MHz of Bluetooth channel
/// `n`, dBm; None when none is
pub fn wifi_level_dbm(n: usize, aps: &[ApReading]) -> Option<f32> {
    let f = 2402 + n as i32;
    let mw: f64 = aps
        .iter()
        .filter(|ap| (1..=14).contains(&ap.channel) && (f - wifi_center_mhz(ap.channel)).abs() <= WIFI_HALF_WIDTH_MHZ)
        .map(|ap| 10f64.powf(f64::from(ap.rssi) / 10.0))
        .sum();
    (mw > 0.0).then(|| (10.0 * mw.log10()) as f32)
}

/// Wi-Fi signal of channel `n`, 0..1
pub fn wifi_score(n: usize, aps: &[ApReading]) -> f32 {
    wifi_level_dbm(n, aps)
        .map(|dbm| ((dbm - WIFI_QUIET_DBM) / (WIFI_LOUD_DBM - WIFI_QUIET_DBM)).clamp(0.0, 1.0))
        .unwrap_or(0.0)
}

/// Every channel's signals and score
pub fn channel_scores(input: &BlendInput) -> Vec<ChannelScore> {
    let mut own = all_usable_map();
    for &ch in input.connected {
        if (1..=14).contains(&ch) {
            mark_bad(&mut own, ch);
        }
    }
    (0..CHANNELS)
        .map(|n| {
            let wifi = input.aps.map(|aps| wifi_score(n, aps));
            let controller = input.controller.left_out(n);
            let present: Vec<f32> = [wifi, controller].into_iter().flatten().collect();
            let score = if present.is_empty() { 0.0 } else { present.iter().sum::<f32>() / present.len() as f32 };
            ChannelScore { wifi, controller, own_wifi: !bit(&own, n), score }
        })
        .collect()
}

/// The "blend" classification
pub fn blend(input: &BlendInput) -> Classification {
    // 1. What "auto" avoids, under its own rule
    let own: Vec<i64> = input.connected.iter().map(|&ch| i64::from(ch)).collect();
    let mut c = avoid_wifi(&own);

    // 2. Channels the signals point at, highest score first, while more
    //    than the floor remain
    let scores = channel_scores(input);
    let threshold = input.strictness.threshold();
    let mut candidates: Vec<usize> = (0..CHANNELS)
        .filter(|&n| bit(&c.map, n))
        .filter(|&n| {
            let was_bad = input.previous.is_some_and(|p| !bit(p, n));
            let needed = if was_bad { threshold - HYSTERESIS } else { threshold };
            scores[n].score >= needed
        })
        .collect();
    candidates.sort_by(|&a, &b| scores[b].score.total_cmp(&scores[a].score).then(a.cmp(&b)));
    let floor = input.strictness.floor().max(MIN_CHANNELS);
    let mut usable = count(&c.map);
    for n in candidates {
        if usable <= floor {
            break;
        }
        c.map[n / 8] &= !(1 << (n % 8));
        usable -= 1;
    }

    // 3. Name the Wi-Fi channels avoided: the PC's own, then those of the
    //    access points heard whose center is now bad, loudest first
    if let Some(aps) = input.aps {
        let mut heard: Vec<&ApReading> = aps.iter().filter(|ap| (1..=14).contains(&ap.channel)).collect();
        heard.sort_by_key(|ap| std::cmp::Reverse(ap.rssi));
        for ap in heard {
            let center = (wifi_center_mhz(ap.channel) - 2402) as usize;
            if center < CHANNELS && !bit(&c.map, center) && !c.avoided_wifi.contains(&ap.channel) {
                c.avoided_wifi.push(ap.channel);
            }
        }
    }
    c.usable = count(&c.map);
    c.passed = c.usable < CHANNELS;
    tracing::debug!(
        "AFH blend {}: {} of 79 usable, left out {}; scan {}, controller readings {}",
        input.strictness.name(),
        c.usable,
        unused_ranges(&c.map),
        input.aps.map_or("none".to_string(), |a| format!("{} access points", a.len())),
        input.controller.readings()
    );
    c
}

fn bit(map: &[u8; 10], n: usize) -> bool {
    map[n / 8] & (1 << (n % 8)) != 0
}
