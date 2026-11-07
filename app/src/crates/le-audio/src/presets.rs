//! LC3 / QoS presets (v1 le_audio_presets.{h,cpp}).
//!
//! The table holds the BAP 1.0 configurations BlueCaster can use: codec
//! configuration settings (BAP Table 3.12, e.g. "48_2" = 48 kHz, 10 ms, 100
//! octets) combined with a unicast QoS setting (BAP Table 5.2: "_1" low
//! latency, "_2" high reliability), plus GMAP's low-latency settings for a
//! Unicast Game Terminal ("32_1_gr" ... "48_4_gr", GMAP 1.0 Table 3.16:
//! RTN 1, max transport latency 15 ms (7.5 ms frames) / 20 ms (10 ms),
//! presentation delay 10 ms). Every built-in preset is unframed with a
//! presentation delay of 40 ms unless it is a "_gr" one.
//!
//! v1's table (name, Hz, frame us, octets, RTN, max latency ms, target
//! latency):
//! low latency 16_1_1 .. 48_6_1 (RTN 2 below 48 kHz, 5 at 48 kHz),
//! high reliability 16_2_2, 24_2_2, 32_2_2, 48_2_2 (RTN 13, 95 ms),
//! 48_4_2, 48_6_2 (RTN 13, 100 ms), and the six "_gr" presets.

use crate::defs::{TARGET_LATENCY_HIGH_RELIABILITY, TARGET_LATENCY_LOW};

/// One preset (v1 struct StreamPreset)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamPreset {
    /// e.g. "48_2_2"
    pub name: String,
    pub sampling_hz: u32,
    /// 10000 or 7500
    pub frame_us: u16,
    /// Per codec frame and channel
    pub octets: u16,
    /// TARGET_LATENCY_* (Config Codec)
    pub target_latency: u8,
    /// 0 unframed
    pub framing: u8,
    /// Retransmission number
    pub rtn: u8,
    pub max_transport_latency_ms: u16,
    pub presentation_delay_us: u32,
}

impl StreamPreset {
    /// One frame per SDU: the SDU interval is the frame duration
    pub fn sdu_interval_us(&self) -> u32 {
        self.frame_us as u32
    }

    /// Bit rate of one channel
    pub fn kbps_per_channel(&self) -> u32 {
        self.octets as u32 * 8 * 1000 / self.frame_us as u32
    }
}

/// The presets the stream may use, by name: the built-in ones, plus what
/// modules add (v1 stream_presets() / add_stream_preset() /
/// find_stream_preset(), a process-wide table there; one per LE core
/// session here)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetTable {
    presets: Vec<StreamPreset>,
}

impl PresetTable {
    /// The built-in presets (BAP Table 5.2, GMAP Table 3.16)
    pub fn new() -> PresetTable {
        const LL: u8 = TARGET_LATENCY_LOW;
        const HR: u8 = TARGET_LATENCY_HIGH_RELIABILITY;
        let presets = vec![
            // BAP 1.0 Table 5.2, low latency
            make("16_1_1", 16000, 7500, 30, 2, 8, LL),
            make("16_2_1", 16000, 10000, 40, 2, 10, LL),
            make("24_1_1", 24000, 7500, 45, 2, 8, LL),
            make("24_2_1", 24000, 10000, 60, 2, 10, LL),
            make("32_1_1", 32000, 7500, 60, 2, 8, LL),
            make("32_2_1", 32000, 10000, 80, 2, 10, LL),
            make("48_1_1", 48000, 7500, 75, 5, 15, LL),
            make("48_2_1", 48000, 10000, 100, 5, 20, LL),
            make("48_3_1", 48000, 7500, 90, 5, 15, LL),
            make("48_4_1", 48000, 10000, 120, 5, 20, LL),
            make("48_5_1", 48000, 7500, 117, 5, 15, LL),
            make("48_6_1", 48000, 10000, 155, 5, 20, LL),
            // high reliability (media)
            make("16_2_2", 16000, 10000, 40, 13, 95, HR),
            make("24_2_2", 24000, 10000, 60, 13, 95, HR),
            make("32_2_2", 32000, 10000, 80, 13, 95, HR),
            make("48_2_2", 48000, 10000, 100, 13, 95, HR),
            make("48_4_2", 48000, 10000, 120, 13, 100, HR),
            make("48_6_2", 48000, 10000, 155, 13, 100, HR),
            // GMAP 1.0 Table 3.16, UGG -> UGT (game audio to a gaming
            // headset): RTN 1, max transport latency 15 ms (7.5 ms frames) /
            // 20 ms (10 ms), presentation delay 10 ms. Used by TMAP / GMAP.
            make_gr("32_1_gr", 32000, 7500, 60, 15),
            make_gr("32_2_gr", 32000, 10000, 80, 20),
            make_gr("48_1_gr", 48000, 7500, 75, 15),
            make_gr("48_2_gr", 48000, 10000, 100, 20),
            make_gr("48_3_gr", 48000, 7500, 90, 15),
            make_gr("48_4_gr", 48000, 10000, 120, 20),
        ];
        PresetTable { presets }
    }

    pub fn presets(&self) -> &[StreamPreset] {
        &self.presets
    }

    /// Adds a preset, or replaces the one with the same name
    pub fn add(&mut self, preset: StreamPreset) {
        match self.presets.iter_mut().find(|p| p.name == preset.name) {
            Some(p) => *p = preset,
            None => self.presets.push(preset),
        }
    }

    pub fn find(&self, name: &str) -> Option<&StreamPreset> {
        self.presets.iter().find(|p| p.name == name)
    }
}

impl Default for PresetTable {
    fn default() -> Self {
        PresetTable::new()
    }
}

/// What a stream tries when nothing else is asked for, best first (v1
/// StreamRequest::preset_order)
pub const DEFAULT_PRESET_ORDER: [&str; 6] = ["48_6_2", "48_4_2", "48_2_2", "32_2_2", "24_2_2", "16_2_2"];

/// A BAP preset: unframed, presentation delay 40 ms
fn make(name: &str, hz: u32, frame_us: u16, octets: u16, rtn: u8, latency_ms: u16, target_latency: u8) -> StreamPreset {
    StreamPreset {
        name: name.to_string(),
        sampling_hz: hz,
        frame_us,
        octets,
        target_latency,
        framing: 0,
        rtn,
        max_transport_latency_ms: latency_ms,
        presentation_delay_us: 40000,
    }
}

/// A GMAP low-latency setting: unframed, RTN 1, presentation delay 10 ms
fn make_gr(name: &str, hz: u32, frame_us: u16, octets: u16, latency_ms: u16) -> StreamPreset {
    StreamPreset { presentation_delay_us: 10000, ..make(name, hz, frame_us, octets, 1, latency_ms, TARGET_LATENCY_LOW) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::TARGET_LATENCY_BALANCED;

    /// v1's table (le_audio_presets.cpp at b819893), written out field by
    /// field: name, Hz, frame us, octets, target latency, framing, RTN,
    /// max transport latency ms, presentation delay us
    type Row = (&'static str, u32, u16, u16, u8, u8, u8, u16, u32);
    const V1: [Row; 24] = [
        ("16_1_1", 16000, 7500, 30, 1, 0, 2, 8, 40000),
        ("16_2_1", 16000, 10000, 40, 1, 0, 2, 10, 40000),
        ("24_1_1", 24000, 7500, 45, 1, 0, 2, 8, 40000),
        ("24_2_1", 24000, 10000, 60, 1, 0, 2, 10, 40000),
        ("32_1_1", 32000, 7500, 60, 1, 0, 2, 8, 40000),
        ("32_2_1", 32000, 10000, 80, 1, 0, 2, 10, 40000),
        ("48_1_1", 48000, 7500, 75, 1, 0, 5, 15, 40000),
        ("48_2_1", 48000, 10000, 100, 1, 0, 5, 20, 40000),
        ("48_3_1", 48000, 7500, 90, 1, 0, 5, 15, 40000),
        ("48_4_1", 48000, 10000, 120, 1, 0, 5, 20, 40000),
        ("48_5_1", 48000, 7500, 117, 1, 0, 5, 15, 40000),
        ("48_6_1", 48000, 10000, 155, 1, 0, 5, 20, 40000),
        ("16_2_2", 16000, 10000, 40, 3, 0, 13, 95, 40000),
        ("24_2_2", 24000, 10000, 60, 3, 0, 13, 95, 40000),
        ("32_2_2", 32000, 10000, 80, 3, 0, 13, 95, 40000),
        ("48_2_2", 48000, 10000, 100, 3, 0, 13, 95, 40000),
        ("48_4_2", 48000, 10000, 120, 3, 0, 13, 100, 40000),
        ("48_6_2", 48000, 10000, 155, 3, 0, 13, 100, 40000),
        ("32_1_gr", 32000, 7500, 60, 1, 0, 1, 15, 10000),
        ("32_2_gr", 32000, 10000, 80, 1, 0, 1, 20, 10000),
        ("48_1_gr", 48000, 7500, 75, 1, 0, 1, 15, 10000),
        ("48_2_gr", 48000, 10000, 100, 1, 0, 1, 20, 10000),
        ("48_3_gr", 48000, 7500, 90, 1, 0, 1, 15, 10000),
        ("48_4_gr", 48000, 10000, 120, 1, 0, 1, 20, 10000),
    ];

    #[test]
    fn table_equals_v1() {
        let t = PresetTable::new();
        assert_eq!(t.presets().len(), V1.len());
        for (p, &(name, hz, frame_us, octets, tl, framing, rtn, latency, pd)) in t.presets().iter().zip(V1.iter()) {
            assert_eq!(
                (p.name.as_str(), p.sampling_hz, p.frame_us, p.octets, p.target_latency),
                (name, hz, frame_us, octets, tl)
            );
            assert_eq!(
                (p.framing, p.rtn, p.max_transport_latency_ms, p.presentation_delay_us),
                (framing, rtn, latency, pd)
            );
        }
        for name in DEFAULT_PRESET_ORDER {
            assert!(t.find(name).is_some(), "{name}");
        }
        assert_eq!(PresetTable::default(), t);
    }

    #[test]
    fn derived_values() {
        let t = PresetTable::new();
        let p = t.find("48_6_2").unwrap();
        assert_eq!((p.sdu_interval_us(), p.kbps_per_channel()), (10000, 124));
        let p = t.find("48_3_gr").unwrap();
        assert_eq!((p.sdu_interval_us(), p.kbps_per_channel()), (7500, 96));
        assert_eq!(t.find("16_2_1").unwrap().kbps_per_channel(), 32);
        assert!(t.find("48_6_3").is_none());
    }

    #[test]
    fn add_or_replace() {
        let mut t = PresetTable::new();
        let n = t.presets().len();
        let mut p = t.find("48_2_2").unwrap().clone();
        p.rtn = 7;
        t.add(p);
        assert_eq!(t.presets().len(), n);
        assert_eq!(t.find("48_2_2").unwrap().rtn, 7);
        p = make("48_2_x", 48000, 10000, 100, 4, 50, TARGET_LATENCY_BALANCED);
        t.add(p.clone());
        assert_eq!(t.presets().len(), n + 1);
        assert_eq!(t.find("48_2_x"), Some(&p));
    }
}
