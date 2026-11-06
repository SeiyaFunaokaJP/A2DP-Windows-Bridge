//! Developer-mode overrides for finding out why a sink stays silent (issue
//! #87): the UI's Developer screen sets them, the encoders and the media
//! path read them when a stream is set up. All off (the default) changes
//! nothing; the core clears them while developer mode is off.

use std::sync::RwLock;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DevOverrides {
    /// RTP marker bit set on every packet (RTP codecs)
    pub rtp_marker: bool,
    /// Log the first bytes of the first media packets
    pub packet_dump: bool,
    /// AAC: the bit rate field of the configuration is 0 (unknown)
    pub aac_config_bitrate0: bool,
    /// AAC: encoder bit rate in kbit/s; 0 = the quality's
    pub aac_bitrate_kbps: u32,
    /// AAC: no AACENC_PEAK_BITRATE
    pub aac_no_peak: bool,
    /// AAC: no afterburner
    pub aac_no_afterburner: bool,
    /// AAC: the StreamMuxConfig of 2.0.2 and before (AudioMuxVersion 0,
    /// implicit signaling) instead of Android's and PipeWire's
    pub aac_latm_v0: bool,
    /// A codec chosen that the device does not list is asked for anyway,
    /// with no fallback (RemoteCaps::forcing)
    pub force_codec: bool,
    /// Auto-select streams the codec it picks with that codec's own
    /// settings (its card's "..."), not Auto's defaults (issue #86)
    pub auto_codec_settings: bool,
    /// A sample rate and bit depth on Auto follow the captured output's
    /// format (Windows' default format of the device) instead of 48 kHz,
    /// as far as the codec and the device take it (issue #86)
    pub match_source: bool,
    /// Every codec runs up to the quality's bit rate whatever maximum the
    /// sink lists (LHDC V5, AAC, Opus 05, the SBC bitpool; issue #237).
    /// Saved under its old name by 2.0.2-beta.17, when it was LHDC V5's
    #[serde(alias = "lhdcv5_ignore_sink_max")]
    pub ignore_sink_max: bool,
    /// Longest a media packet waits in the send queue before it is dropped,
    /// in ms (QUEUE_WAIT_MIN..=QUEUE_WAIT_MAX); 0 = the default (350 ms).
    /// Takes effect when the next stream starts
    pub queue_max_wait_ms: u32,
    /// Audio waits unencoded while the send queue is backed up, so it goes
    /// out at the bit rate ABR has lowered to meanwhile (issue #279)
    pub encode_at_send: bool,
}

/// Range of [`DevOverrides::queue_max_wait_ms`] (ms)
pub const QUEUE_WAIT_MIN: u32 = 50;
pub const QUEUE_WAIT_MAX: u32 = 2000;

impl DevOverrides {
    pub fn is_default(&self) -> bool {
        *self == DevOverrides::default()
    }
}

static CURRENT: RwLock<DevOverrides> = RwLock::new(DevOverrides {
    rtp_marker: false,
    packet_dump: false,
    aac_config_bitrate0: false,
    aac_bitrate_kbps: 0,
    aac_no_peak: false,
    aac_no_afterburner: false,
    aac_latm_v0: false,
    force_codec: false,
    auto_codec_settings: false,
    match_source: false,
    ignore_sink_max: false,
    queue_max_wait_ms: 0,
    encode_at_send: false,
});

/// The overrides in force
pub fn overrides() -> DevOverrides {
    *CURRENT.read().unwrap_or_else(|e| e.into_inner())
}

/// Sets the overrides; logs when they change
pub fn set_overrides(o: DevOverrides) {
    let mut cur = CURRENT.write().unwrap_or_else(|e| e.into_inner());
    if *cur != o {
        if o.is_default() {
            tracing::info!("developer overrides: off");
        } else {
            tracing::warn!("developer overrides: {o:?}");
        }
        *cur = o;
    }
}
