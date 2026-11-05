//! The Wi-Fi of this PC, for AFH (the WLAN API part of v1 afh.cpp)
//!
//! Which channel each connected interface is on: AFH "auto" avoids only
//! that channel, when it is a 2.4 GHz one. No scan is requested: a scan
//! would put more traffic on the very band being measured. Deciding which
//! Bluetooth channels to avoid is bt-host's afh module; this returns plain
//! data.
//!
//! Only calls that need no location permission (issue #86): since Windows
//! 11 24H2 the access point list (WlanGetNetworkBssList) and the current
//! connection (its BSSID) count as location, and asking for them shows
//! Windows' location prompt. The channel number does not.
//!
//! The exceptions are `scan` and `scan_24ghz`: the developer-mode scan of
//! the diagnostics screen and AFH "blend". They run only when the user
//! asks for them, and may then meet Windows' location setting.

use std::ffi::c_void;
use std::time::Duration;

use windows::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_SUCCESS, HANDLE};
use windows::Win32::NetworkManagement::WiFi::{
    WLAN_BSS_ENTRY, WLAN_BSS_LIST, WLAN_INTERFACE_INFO, WLAN_INTERFACE_INFO_LIST, WlanCloseHandle, WlanEnumInterfaces,
    WlanFreeMemory, WlanGetNetworkBssList, WlanOpenHandle, WlanQueryInterface, WlanScan, dot11_BSS_type_any,
    wlan_interface_state_connected, wlan_intf_opcode_channel_number,
};

/// One Wi-Fi interface of this PC
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiInterface {
    pub description: String,
    pub connected: bool,
    /// The 2.4 GHz channel (1-14) it is connected on; None when it is
    /// disconnected or on 5 GHz
    pub channel_24ghz: Option<u8>,
}

/// What `survey` found; empty on a PC without Wi-Fi
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WifiSurvey {
    pub interfaces: Vec<WifiInterface>,
}

impl WifiSurvey {
    /// 2.4 GHz channels this PC's interfaces are connected on (what AFH
    /// "auto" avoids); empty when no interface is on 2.4 GHz
    pub fn connected_channels_24ghz(&self) -> Vec<u8> {
        let mut channels: Vec<u8> =
            self.interfaces.iter().filter(|i| i.connected).filter_map(|i| i.channel_24ghz).collect();
        channels.sort_unstable();
        channels.dedup();
        channels
    }
}

/// The 2.4 GHz channel of a connected interface's channel number: 1-14.
/// 5 GHz channels are 32 and up. 6 GHz channels 1-13 share their numbers
/// with 2.4 GHz ones and cannot be told apart without the connection's PHY
/// (location permission): such a channel is avoided too, which costs a few
/// Bluetooth channels and nothing else.
pub fn connected_channel_24ghz(channel_number: Option<u32>) -> Option<u8> {
    match channel_number {
        Some(n @ 1..=14) => Some(n as u8),
        _ => None,
    }
}

/// 2.4 GHz channel number of a center frequency (v1's mapping: 2412-2472
/// MHz in 5 MHz steps are channels 1-13, 2484 MHz is channel 14)
pub fn channel_from_mhz(mhz: u32) -> Option<u8> {
    match mhz {
        2484 => Some(14),
        2412..=2472 if (mhz - 2407).is_multiple_of(5) => Some(((mhz - 2407) / 5) as u8),
        _ => None,
    }
}

/// The channel number of a center frequency on any band: 2.4 GHz as
/// `channel_from_mhz`, 5 GHz from 5000 MHz and 6 GHz from 5950 MHz in
/// 5 MHz steps
pub fn any_channel_from_mhz(mhz: u32) -> Option<u8> {
    let from = |base: u32| (mhz - base).is_multiple_of(5).then(|| ((mhz - base) / 5) as u8);
    match mhz {
        2400..=2500 => channel_from_mhz(mhz),
        5150..=5895 => from(5000),
        5955..=7115 => from(5950),
        _ => None,
    }
}

/// This PC's Wi-Fi interfaces and their channels. A PC without Wi-Fi (or
/// with the WLAN AutoConfig service stopped) gives an empty survey, not an
/// error, as in v1.
pub fn survey() -> windows::core::Result<WifiSurvey> {
    let mut negotiated = 0u32;
    let mut client = HANDLE::default();
    // SAFETY: client version 2 (Vista and later), valid out pointers.
    let err = unsafe { WlanOpenHandle(2, None, &mut negotiated, &mut client) };
    if err != ERROR_SUCCESS.0 {
        tracing::debug!("wlan: WlanOpenHandle failed ({err}): no Wi-Fi");
        return Ok(WifiSurvey::default());
    }
    let client = Client(client);

    let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
    // SAFETY: open client handle, valid out pointer.
    let err = unsafe { WlanEnumInterfaces(client.0, None, &mut list) };
    if err != ERROR_SUCCESS.0 {
        tracing::debug!("wlan: WlanEnumInterfaces failed ({err})");
        return Err(windows::Win32::Foundation::WIN32_ERROR(err).into());
    }
    let list = WlanMemory(list.cast());
    // SAFETY: WlanEnumInterfaces succeeded, so `list` points to a list
    // whose dwNumberOfItems entries follow in one allocation.
    let interfaces: &[WLAN_INTERFACE_INFO] = unsafe {
        let l = list.0.cast::<WLAN_INTERFACE_INFO_LIST>();
        std::slice::from_raw_parts(std::ptr::addr_of!((*l).InterfaceInfo).cast(), (*l).dwNumberOfItems as usize)
    };

    let mut result = WifiSurvey::default();
    for intf in interfaces {
        let connected = intf.isState == wlan_interface_state_connected;
        let number = if connected { channel_number(&client, &intf.InterfaceGuid) } else { None };
        let channel_24ghz = connected_channel_24ghz(number);
        tracing::debug!(
            "wlan: {:?} state {}, channel {number:?}, 2.4 GHz channel {channel_24ghz:?}",
            crate::autostart::utf16_string(&intf.strInterfaceDescription),
            intf.isState.0,
        );
        result.interfaces.push(WifiInterface {
            description: crate::autostart::utf16_string(&intf.strInterfaceDescription),
            connected,
            channel_24ghz,
        });
    }
    tracing::debug!(
        "wlan: {} interface(s) (client version {negotiated}), connected 2.4 GHz channels {:?}",
        result.interfaces.len(),
        result.connected_channels_24ghz()
    );
    Ok(result)
}

/// An access point a scan heard
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessPoint {
    /// Empty for a hidden network
    pub ssid: String,
    /// The channel number on its band (1-14 on 2.4 GHz)
    pub channel: u8,
    /// Center frequency
    pub mhz: u32,
    /// dBm
    pub rssi: i32,
}

impl AccessPoint {
    pub fn is_24ghz(&self) -> bool {
        self.mhz < 2500
    }
}

/// Why `scan_24ghz` found nothing to list
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanError {
    /// No Wi-Fi interface (or the WLAN AutoConfig service is stopped)
    NoWifi,
    /// Windows' location setting keeps the list from apps
    LocationDenied,
    /// Wi-Fi is turned off (airplane mode or the Wi-Fi switch)
    RadioOff,
    /// Another WLAN API error
    Failed(u32),
}

/// ERROR_NDIS_DOT11_POWER_STATE_INVALID: the radio is off
const ERROR_RADIO_OFF: u32 = 0x8034_2002;

/// `scan`, 2.4 GHz access points only
pub fn scan_24ghz(wait: Duration) -> Result<Vec<AccessPoint>, ScanError> {
    let mut aps = scan(wait)?;
    aps.retain(AccessPoint::is_24ghz);
    Ok(aps)
}

/// Asks every Wi-Fi interface for a fresh scan, waits `wait` for it and
/// returns the access points heard on every band, loudest first. Needs
/// location permission on Windows 11 24H2 and later; run it only when the
/// user asks.
pub fn scan(wait: Duration) -> Result<Vec<AccessPoint>, ScanError> {
    let mut negotiated = 0u32;
    let mut client = HANDLE::default();
    // SAFETY: client version 2 (Vista and later), valid out pointers.
    let err = unsafe { WlanOpenHandle(2, None, &mut negotiated, &mut client) };
    if err != ERROR_SUCCESS.0 {
        return Err(ScanError::NoWifi);
    }
    let client = Client(client);

    let mut list: *mut WLAN_INTERFACE_INFO_LIST = std::ptr::null_mut();
    // SAFETY: open client handle, valid out pointer.
    let err = unsafe { WlanEnumInterfaces(client.0, None, &mut list) };
    if err != ERROR_SUCCESS.0 {
        return Err(ScanError::Failed(err));
    }
    let list = WlanMemory(list.cast());
    // SAFETY: as in `survey`.
    let guids: Vec<windows::core::GUID> = unsafe {
        let l = list.0.cast::<WLAN_INTERFACE_INFO_LIST>();
        let items: &[WLAN_INTERFACE_INFO] =
            std::slice::from_raw_parts(std::ptr::addr_of!((*l).InterfaceInfo).cast(), (*l).dwNumberOfItems as usize);
        items.iter().map(|i| i.InterfaceGuid).collect()
    };
    if guids.is_empty() {
        return Err(ScanError::NoWifi);
    }

    for guid in &guids {
        // SAFETY: open client, valid GUID. A refused scan still leaves the
        // list Windows keeps from its own scans.
        let err = unsafe { WlanScan(client.0, guid, None, None, None) };
        if err != ERROR_SUCCESS.0 {
            tracing::debug!("wlan: WlanScan failed ({err})");
        }
    }
    std::thread::sleep(wait);

    let mut heard: Vec<([u8; 6], AccessPoint)> = Vec::new();
    let (mut failures, mut denied, mut last_err) = (0, false, 0);
    for guid in &guids {
        let mut bss: *mut WLAN_BSS_LIST = std::ptr::null_mut();
        // SAFETY: open client, valid GUID and out pointer.
        let err = unsafe { WlanGetNetworkBssList(client.0, guid, None, dot11_BSS_type_any, false, None, &mut bss) };
        if err != ERROR_SUCCESS.0 || bss.is_null() {
            tracing::debug!("wlan: WlanGetNetworkBssList failed ({err})");
            failures += 1;
            denied |= err == ERROR_ACCESS_DENIED.0;
            last_err = err;
            continue;
        }
        let bss = WlanMemory(bss.cast());
        // SAFETY: the call succeeded, so `bss` holds dwNumberOfItems entries
        // in one allocation. Fields are read through raw pointers: the
        // entry has a BOOLEAN field that a Rust reference would require to
        // be exactly 0 or 1.
        unsafe {
            let l = bss.0.cast::<WLAN_BSS_LIST>();
            let n = (*l).dwNumberOfItems as usize;
            let first = std::ptr::addr_of!((*l).wlanBssEntries).cast::<WLAN_BSS_ENTRY>();
            for i in 0..n {
                let e = first.add(i);
                let khz = std::ptr::addr_of!((*e).ulChCenterFrequency).read_unaligned();
                let mhz = khz / 1000;
                let Some(channel) = any_channel_from_mhz(mhz) else { continue };
                let bssid = std::ptr::addr_of!((*e).dot11Bssid).read_unaligned();
                let ssid = std::ptr::addr_of!((*e).dot11Ssid).read_unaligned();
                let len = (ssid.uSSIDLength as usize).min(ssid.ucSSID.len());
                let ap = AccessPoint {
                    ssid: String::from_utf8_lossy(&ssid.ucSSID[..len]).into_owned(),
                    channel,
                    mhz,
                    rssi: std::ptr::addr_of!((*e).lRssi).read_unaligned(),
                };
                // Two interfaces can hear the same access point: keep the louder
                match heard.iter_mut().find(|(b, _)| *b == bssid) {
                    Some((_, old)) if old.rssi >= ap.rssi => {}
                    Some((_, old)) => *old = ap,
                    None => heard.push((bssid, ap)),
                }
            }
        }
    }
    // Every interface refused: say why
    if failures == guids.len() {
        return Err(if denied {
            ScanError::LocationDenied
        } else if last_err == ERROR_RADIO_OFF {
            ScanError::RadioOff
        } else {
            ScanError::Failed(last_err)
        });
    }
    let mut aps: Vec<AccessPoint> = heard.into_iter().map(|(_, ap)| ap).collect();
    aps.sort_by(|a, b| b.rssi.cmp(&a.rssi).then(a.channel.cmp(&b.channel)));
    tracing::debug!(
        "wlan: scan heard {} access point(s), {} on 2.4 GHz",
        aps.len(),
        aps.iter().filter(|a| a.is_24ghz()).count()
    );
    Ok(aps)
}

/// The channel number a connected interface is on (no band: see
/// `connected_channel_24ghz`)
fn channel_number(client: &Client, interface: &windows::core::GUID) -> Option<u32> {
    let mut size = 0u32;
    let mut data: *mut c_void = std::ptr::null_mut();
    // SAFETY: open client, valid GUID and out pointers.
    let err = unsafe {
        WlanQueryInterface(client.0, interface, wlan_intf_opcode_channel_number, None, &mut size, &mut data, None)
    };
    if err != ERROR_SUCCESS.0 || data.is_null() {
        tracing::debug!("wlan: querying the channel number failed ({err})");
        return None;
    }
    let data = WlanMemory(data);
    if (size as usize) < size_of::<u32>() {
        return None;
    }
    // SAFETY: the opcode returns a ULONG of `size` bytes, checked above.
    Some(unsafe { data.0.cast::<u32>().read_unaligned() })
}

/// A WLAN client handle, closed on drop
struct Client(HANDLE);

impl Drop for Client {
    fn drop(&mut self) {
        // SAFETY: opened by WlanOpenHandle, closed only here.
        unsafe { WlanCloseHandle(self.0, None) };
    }
}

/// Memory the WLAN API allocated, freed on drop
struct WlanMemory(*mut c_void);

impl Drop for WlanMemory {
    fn drop(&mut self) {
        // SAFETY: allocated by the WLAN API and freed only here.
        unsafe { WlanFreeMemory(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_mapping_matches_v1() {
        assert_eq!(channel_from_mhz(2412), Some(1));
        assert_eq!(channel_from_mhz(2437), Some(6));
        assert_eq!(channel_from_mhz(2462), Some(11));
        assert_eq!(channel_from_mhz(2472), Some(13));
        assert_eq!(channel_from_mhz(2484), Some(14));
        assert_eq!(channel_from_mhz(2414), None);
        assert_eq!(channel_from_mhz(5180), None);
        assert_eq!(channel_from_mhz(5955), None); // 6 GHz channel 1
    }

    #[test]
    fn channels_on_every_band() {
        assert_eq!(any_channel_from_mhz(2437), Some(6));
        assert_eq!(any_channel_from_mhz(2484), Some(14));
        assert_eq!(any_channel_from_mhz(5180), Some(36));
        assert_eq!(any_channel_from_mhz(5260), Some(52));
        assert_eq!(any_channel_from_mhz(5500), Some(100));
        assert_eq!(any_channel_from_mhz(5825), Some(165));
        assert_eq!(any_channel_from_mhz(5955), Some(1));
        assert_eq!(any_channel_from_mhz(6415), Some(93));
        assert_eq!(any_channel_from_mhz(7115), Some(233));
        assert_eq!(any_channel_from_mhz(5182), None);
        assert_eq!(any_channel_from_mhz(900), None);
    }

    #[test]
    fn connected_channel_is_a_2_4_ghz_number() {
        assert_eq!(connected_channel_24ghz(Some(6)), Some(6));
        assert_eq!(connected_channel_24ghz(Some(1)), Some(1));
        assert_eq!(connected_channel_24ghz(Some(14)), Some(14));
        assert_eq!(connected_channel_24ghz(Some(36)), None);
        assert_eq!(connected_channel_24ghz(Some(149)), None);
        assert_eq!(connected_channel_24ghz(Some(0)), None);
        assert_eq!(connected_channel_24ghz(None), None);
    }

    #[test]
    fn connected_channels_come_from_connected_interfaces_only() {
        let interface =
            |connected, channel_24ghz| WifiInterface { description: String::new(), connected, channel_24ghz };
        // Disconnected (2026-10-17 case), or connected on 5 GHz: nothing
        for intf in [interface(false, None), interface(true, None)] {
            let s = WifiSurvey { interfaces: vec![intf] };
            assert!(s.connected_channels_24ghz().is_empty());
        }
        // Connected on 6; two interfaces on the same channel count once
        let s = WifiSurvey {
            interfaces: vec![interface(true, Some(6)), interface(true, Some(6)), interface(false, Some(1))],
        };
        assert_eq!(s.connected_channels_24ghz(), [6]);
    }

    #[test]
    #[ignore = "asks Windows for a scan and may meet the location setting"]
    fn scan_runs() {
        match scan(Duration::from_secs(4)) {
            Ok(aps) => {
                for ap in &aps {
                    eprintln!("{} MHz ch {:3} {:4} dBm {:?}", ap.mhz, ap.channel, ap.rssi, ap.ssid);
                }
            }
            Err(e) => eprintln!("scan: {e:?}"),
        }
    }

    #[test]
    fn survey_runs() {
        // Read-only, and asks for nothing that needs location permission
        let s = survey().unwrap();
        eprintln!("{} interface(s), connected 2.4 GHz {:?}", s.interfaces.len(), s.connected_channels_24ghz());
    }
}
