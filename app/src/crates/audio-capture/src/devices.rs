//! Render devices: the list (with v1's virtual device heuristic), the
//! default device, its mix rate and format, switching the default
//! (IPolicyConfig) and the endpoint volume primitives of volume sync.
//!
//! Every function initialises COM on the calling thread for its own
//! duration (MTA, or whatever apartment the thread already has).

use wasapi::{DeviceEnumerator, Direction};
use win_sys::audio_policy::{self, ComApartment, EndpointHints};

use crate::CaptureError;

/// E_NOTFOUND (HRESULT_FROM_WIN32(ERROR_NOT_FOUND)): no such endpoint, or
/// no default one
const E_NOTFOUND: u32 = 0x8007_0490;
/// E_INVALIDARG: what GetDevice returns for an ID that is not an endpoint
/// ID at all (a stale or mangled setting)
const E_INVALIDARG: u32 = 0x8007_0057;

fn not_found(code: i32) -> bool {
    matches!(code as u32, E_NOTFOUND | E_INVALIDARG)
}

fn com() -> Result<ComApartment, CaptureError> {
    ComApartment::init_mta().map_err(|e| CaptureError::windows("CoInitializeEx", e.code().0, e.message()))
}

/// An active render endpoint
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceInfo {
    /// WASAPI endpoint ID (IMMDevice::GetId)
    pub id: String,
    /// Friendly name, e.g. "Speakers (Realtek(R) Audio)"
    pub name: String,
    /// The default output device (Console role)
    pub is_default: bool,
    /// Looks like a virtual device (VB-Cable, VoiceMeeter, ...): v1's
    /// heuristic, used to suggest devices for virtual device mode
    pub is_virtual: bool,
}

/// Active render endpoints, in Windows' order
pub fn render_devices() -> Result<Vec<DeviceInfo>, CaptureError> {
    let _com = com()?;
    let enumerator = DeviceEnumerator::new().map_err(|e| CaptureError::from_wasapi("device enumerator", e))?;
    let default_id = enumerator.get_default_device(&Direction::Render).and_then(|d| d.get_id()).ok();
    let collection = enumerator
        .get_device_collection(&Direction::Render)
        .map_err(|e| CaptureError::from_wasapi("EnumAudioEndpoints", e))?;
    let mut out = Vec::new();
    for device in &collection {
        let Ok(device) = device else { continue };
        let Ok(id) = device.get_id() else { continue };
        let name = device.get_friendlyname().unwrap_or_default();
        let hints = audio_policy::endpoint_hints(&id).unwrap_or_default();
        out.push(DeviceInfo {
            is_default: default_id.as_deref() == Some(id.as_str()),
            is_virtual: is_virtual_device(&name, &hints),
            id,
            name,
        });
    }
    if tracing::enabled!(tracing::Level::DEBUG) {
        tracing::debug!("audio devices: {} active render endpoints, default {default_id:?}", out.len());
        for d in &out {
            tracing::debug!(
                "audio devices: \"{}\" id {}{}{}",
                d.name,
                d.id,
                if d.is_default { " (default)" } else { "" },
                if d.is_virtual { " (virtual)" } else { "" }
            );
        }
    }
    Ok(out)
}

/// ID of the default render device (Console role), if there is one
pub fn default_render_device() -> Option<String> {
    let _com = com().ok()?;
    DeviceEnumerator::new().and_then(|e| e.get_default_device(&Direction::Render)).and_then(|d| d.get_id()).ok()
}

/// Make `id` the default output device (Console and Multimedia roles, as v1
/// does), through the undocumented IPolicyConfig interface. Changes the
/// user's system setting: the caller restores the previous default.
pub fn set_default_render_device(id: &str) -> Result<(), CaptureError> {
    if id.is_empty() {
        return Err(CaptureError::DeviceNotFound(String::new()));
    }
    let _com = com()?;
    tracing::debug!("audio devices: setting default render endpoint to {id}");
    audio_policy::set_default_endpoint(id).map_err(|e| {
        tracing::debug!("audio devices: SetDefaultEndpoint({id}) failed: HRESULT 0x{:08x}", e.code().0 as u32);
        if not_found(e.code().0) {
            CaptureError::DeviceNotFound(id.to_owned())
        } else {
            CaptureError::windows("SetDefaultEndpoint", e.code().0, e.message())
        }
    })
}

/// Shared-mode mix rate of a render device (None = the default one), for
/// the "auto" sample rate: what a loopback capture delivers without a
/// preferred rate. None if the device cannot be opened.
pub fn mix_rate(device_id: Option<&str>) -> Option<u32> {
    let _com = com().ok()?;
    let enumerator = DeviceEnumerator::new().ok()?;
    let device = match device_id {
        Some(id) => enumerator.get_device(id).ok()?,
        None => enumerator.get_default_device(&Direction::Render).ok()?,
    };
    let client = device.get_iaudioclient().ok()?;
    let mix = client.get_mixformat().ok()?;
    tracing::debug!("audio devices: mix rate of {device_id:?}: {} Hz", mix.get_samplespersec());
    Some(mix.get_samplespersec())
}

/// A render device's shared-mode format, for the profile editor's "capture
/// device" line and its warning when a profile asks for another rate or bit
/// depth (v1 capture.device_format)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MixFormat {
    /// The mix rate: what a loopback capture delivers without resampling
    pub rate: u32,
    pub channels: u16,
    /// Valid bits per sample of the format chosen in Windows' Sound
    /// settings (PKEY_AudioEngine_DeviceFormat, as v1 showed), else of the
    /// mix format (32: the engine mixes in float)
    pub bits: u16,
}

/// [`MixFormat`] of a render device (None = the default one). None if the
/// device cannot be opened.
pub fn mix_format(device_id: Option<&str>) -> Option<MixFormat> {
    let _com = com().ok()?;
    let enumerator = DeviceEnumerator::new().ok()?;
    let device = match device_id {
        Some(id) => enumerator.get_device(id).ok()?,
        None => enumerator.get_default_device(&Direction::Render).ok()?,
    };
    let client = device.get_iaudioclient().ok()?;
    let mix = client.get_mixformat().ok()?;
    let bits =
        device.get_device_format().ok().map_or(0, |f| valid_bits(f.get_validbitspersample(), f.get_bitspersample()));
    let format = MixFormat {
        rate: mix.get_samplespersec(),
        channels: mix.get_nchannels(),
        bits: if bits > 0 { bits } else { valid_bits(mix.get_validbitspersample(), mix.get_bitspersample()) },
    };
    tracing::debug!(
        "audio devices: format of {device_id:?}: {format:?} (device format {} bit, mix {} bit)",
        bits,
        mix.get_bitspersample()
    );
    Some(format)
}

/// wValidBitsPerSample, which a plain WAVEFORMATEX leaves at 0 (its
/// container size counts then)
fn valid_bits(valid: u16, container: u16) -> u16 {
    if valid > 0 && valid <= container { valid } else { container }
}

/// v1 AudioDeviceEnumerator::is_virtual_device: name patterns first, then
/// the endpoint's form factor and container ID
pub(crate) fn is_virtual_device(name: &str, hints: &EndpointHints) -> bool {
    const PATTERNS: [&str; 8] = [
        "cable",         // VB-Cable
        "voicemeeter",   // VoiceMeeter
        "virtual audio", // generic
        "sar",           // Synchronous Audio Router
        "blackhole",     // BlackHole
        "stereo mix",    // Stereo Mix
        "what u hear",   // Creative
        "vbvmvaio",      // VoiceMeeter driver name
    ];
    let lower = name.to_lowercase();
    if PATTERNS.iter().any(|p| lower.contains(p)) {
        return true;
    }
    // EndpointFormFactor::UnknownFormFactor
    const UNKNOWN_FORM_FACTOR: u32 = 8;
    if hints.form_factor == Some(UNKNOWN_FORM_FACTOR) {
        return true;
    }
    // Virtual devices typically have no container: the null container
    // {00000000-0000-0000-ffff-ffffffffffff} or an all-zero GUID
    const NULL_CONTAINER: u128 = 0x0000_0000_0000_0000_ffff_ffff_ffff_ffff;
    matches!(hints.container_id, Some(NULL_CONTAINER) | Some(0))
}

/// Master volume (the scalar the Windows slider shows, 0.0..=1.0) and mute
/// of a render endpoint: the primitives of volume sync. The 0..127 mapping
/// and the sync logic are core's.
///
/// Bound to the endpoint it was opened on: with `None`, the one that was the
/// default then. To follow the default device, compare
/// [`EndpointVolume::device_id`] with [`default_render_device`] and reopen.
/// Not `Send`: use it on the thread that opened it (it keeps COM
/// initialised there while it lives).
#[derive(Debug)]
pub struct EndpointVolume {
    // Dropped before the apartment
    inner: audio_policy::EndpointVolume,
    _com: ComApartment,
}

impl EndpointVolume {
    pub fn open(device_id: Option<&str>) -> Result<EndpointVolume, CaptureError> {
        let com = com()?;
        let inner =
            audio_policy::EndpointVolume::open(device_id).map_err(|e| match (not_found(e.code().0), device_id) {
                (true, Some(id)) => CaptureError::DeviceNotFound(id.to_owned()),
                (true, None) => CaptureError::NoDevice,
                _ => CaptureError::Activation { hresult: e.code().0 as u32, message: e.message() },
            })?;
        Ok(EndpointVolume { inner, _com: com })
    }

    /// The endpoint this volume belongs to
    pub fn device_id(&self) -> &str {
        self.inner.device_id()
    }

    /// Master volume scalar, 0.0..=1.0
    pub fn volume(&self) -> Result<f32, CaptureError> {
        self.inner.volume().map_err(|e| CaptureError::windows("GetMasterVolumeLevelScalar", e.code().0, e.message()))
    }

    /// Set the master volume scalar (clamped to 0.0..=1.0)
    pub fn set_volume(&self, level: f32) -> Result<(), CaptureError> {
        self.inner
            .set_volume(level)
            .map_err(|e| CaptureError::windows("SetMasterVolumeLevelScalar", e.code().0, e.message()))
    }

    pub fn muted(&self) -> Result<bool, CaptureError> {
        self.inner.muted().map_err(|e| CaptureError::windows("GetMute", e.code().0, e.message()))
    }

    pub fn set_muted(&self, muted: bool) -> Result<(), CaptureError> {
        self.inner.set_muted(muted).map_err(|e| CaptureError::windows("SetMute", e.code().0, e.message()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: EndpointHints = EndpointHints {
        form_factor: Some(1), // Speakers
        container_id: Some(0x1234_5678_9abc_def0_1234_5678_9abc_def0),
    };

    #[test]
    fn virtual_by_name() {
        assert!(is_virtual_device("CABLE Input (VB-Audio Virtual Cable)", &REAL));
        assert!(is_virtual_device("VoiceMeeter Input (VB-Audio VoiceMeeter VAIO)", &REAL));
        assert!(is_virtual_device("Stereo Mix (Realtek Audio)", &REAL));
        assert!(is_virtual_device("blackhole 2ch", &REAL));
        assert!(!is_virtual_device("Speakers (Realtek(R) Audio)", &REAL));
        assert!(!is_virtual_device("Headphones (WH-1000XM4)", &REAL));
    }

    #[test]
    fn virtual_by_properties() {
        let unknown_form = EndpointHints { form_factor: Some(8), ..REAL };
        assert!(is_virtual_device("Speakers", &unknown_form));
        let null_container = EndpointHints { container_id: Some(0xffff_ffff_ffff_ffff), ..REAL };
        assert!(is_virtual_device("Speakers", &null_container));
        let zero_container = EndpointHints { container_id: Some(0), ..REAL };
        assert!(is_virtual_device("Speakers", &zero_container));
        // Unreadable properties say nothing
        assert!(!is_virtual_device("Speakers", &EndpointHints::default()));
    }

    #[test]
    fn device_list_is_consistent() {
        let devices = render_devices().unwrap();
        let defaults = devices.iter().filter(|d| d.is_default).count();
        assert!(defaults <= 1);
        if let Some(def) = default_render_device() {
            assert!(devices.iter().any(|d| d.id == def && d.is_default));
            assert!(mix_rate(None).is_some_and(|r| r >= 8_000));
            assert_eq!(mix_rate(Some(&def)), mix_rate(None));
            let vol = EndpointVolume::open(None).unwrap();
            assert_eq!(vol.device_id(), def);
            assert!((0.0..=1.0).contains(&vol.volume().unwrap()));
            vol.muted().unwrap();
        }
        assert_eq!(mix_rate(Some("no such device")), None);
        assert_eq!(mix_format(Some("no such device")), None);
        let err = EndpointVolume::open(Some("no such device")).unwrap_err();
        assert!(matches!(err, CaptureError::DeviceNotFound(_)), "{err}");
    }

    #[test]
    fn mix_format_matches_mix_rate() {
        // No output device (a CI runner): nothing to compare
        let Some(f) = mix_format(None) else {
            assert_eq!(mix_rate(None), None);
            return;
        };
        assert_eq!(Some(f.rate), mix_rate(None));
        assert!(f.channels >= 1, "{f:?}");
        assert!(matches!(f.bits, 8 | 16 | 20 | 24 | 32 | 64), "{f:?}");
        if let Some(def) = default_render_device() {
            assert_eq!(mix_format(Some(&def)), Some(f));
        }
    }

    #[test]
    fn valid_bits_falls_back_to_the_container() {
        assert_eq!(valid_bits(24, 32), 24);
        assert_eq!(valid_bits(0, 16), 16);
        assert_eq!(valid_bits(32, 24), 24);
    }

    #[test]
    fn set_default_rejects_empty_id() {
        // Only the argument check runs: tests never change the default device
        assert!(matches!(set_default_render_device(""), Err(CaptureError::DeviceNotFound(_))));
    }
}
