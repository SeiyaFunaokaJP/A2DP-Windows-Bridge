//! The Bluetooth devices Windows knows (paired, remembered or connected):
//! v1 `BtDeviceDiscovery::scan_paired_devices` (bt_device.cpp)
//!
//! Reads Windows' own list through BluetoothFindFirstDevice /
//! BluetoothFindNextDevice without an inquiry: nothing is sent over the
//! air and the USB adapter BlueCaster uses (WinUSB) is not touched. The list is
//! what Windows' Bluetooth stack remembers, so it also works when that
//! stack has no radio any more (the adapter was switched to WinUSB): the
//! names and addresses stay in the registry.
//!
//! Whether a device is an A2DP sink is guessed from its class of device as
//! v1 did (major class Audio/Video), not from an SDP query.

use windows::Win32::Devices::Bluetooth::{
    BLUETOOTH_DEVICE_INFO, BLUETOOTH_DEVICE_SEARCH_PARAMS, BluetoothFindDeviceClose, BluetoothFindFirstDevice,
    BluetoothFindNextDevice,
};
use windows::Win32::Foundation::{ERROR_NO_MORE_ITEMS, HANDLE};

/// Major device class Audio/Video (bits 12-8 of the class of device)
pub const MAJOR_CLASS_AUDIO_VIDEO: u32 = 0x04;

/// A device in Windows' Bluetooth device list
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedDevice {
    /// Display order, most significant byte first (as `bt_host::addr_str`
    /// prints it: `address[0]` is the "XX" of "XX:..")
    pub address: [u8; 6],
    pub name: String,
    pub class_of_device: u32,
    pub connected: bool,
    pub remembered: bool,
    pub authenticated: bool,
}

impl PairedDevice {
    /// v1's A2DP sink test on the class of device
    pub fn is_a2dp_sink(&self) -> bool {
        is_a2dp_sink_class(self.class_of_device)
    }

    /// "XX:XX:XX:XX:XX:XX"
    pub fn address_string(&self) -> String {
        format_address(&self.address)
    }
}

/// v1 `check_a2dp_sink`: a device whose major class is Audio/Video
/// (headphones, loudspeakers, car audio, ...) is taken for an A2DP sink
pub fn is_a2dp_sink_class(class_of_device: u32) -> bool {
    (class_of_device >> 8) & 0x1F == MAJOR_CLASS_AUDIO_VIDEO
}

/// "XX:XX:XX:XX:XX:XX" of an address in display order
pub fn format_address(a: &[u8; 6]) -> String {
    a.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(":")
}

/// BTH_ADDR (the 48-bit address in the low bytes of a u64, least
/// significant byte first in memory) to display order
pub fn address_from_bth_addr(bth: u64) -> [u8; 6] {
    let mut a = [0u8; 6];
    for (i, b) in a.iter_mut().enumerate() {
        *b = (bth >> (8 * (5 - i))) as u8;
    }
    a
}

fn name_from_wide(w: &[u16]) -> String {
    let len = w.iter().position(|&c| c == 0).unwrap_or(w.len());
    String::from_utf16_lossy(&w[..len])
}

fn device_from_info(info: &BLUETOOTH_DEVICE_INFO) -> PairedDevice {
    // SAFETY: both union fields are plain integers over the same 8 bytes;
    // ullLong is always fully initialised (the struct was zeroed).
    let bth = unsafe { info.Address.Anonymous.ullLong };
    PairedDevice {
        address: address_from_bth_addr(bth),
        name: name_from_wide(&info.szName),
        class_of_device: info.ulClassofDevice,
        connected: info.fConnected.as_bool(),
        remembered: info.fRemembered.as_bool(),
        authenticated: info.fAuthenticated.as_bool(),
    }
}

/// The devices Windows has paired, remembered or connected, in Windows'
/// order (v1's search parameters: no unknown devices, no inquiry).
/// Ok(empty) when there are none; Err with the Win32 error otherwise
/// (e.g. no Bluetooth support at all).
pub fn paired_devices() -> windows::core::Result<Vec<PairedDevice>> {
    let params = BLUETOOTH_DEVICE_SEARCH_PARAMS {
        dwSize: size_of::<BLUETOOTH_DEVICE_SEARCH_PARAMS>() as u32,
        fReturnAuthenticated: true.into(),
        fReturnRemembered: true.into(),
        fReturnUnknown: false.into(),
        fReturnConnected: true.into(),
        fIssueInquiry: false.into(),
        cTimeoutMultiplier: 0,
        // All radios
        hRadio: HANDLE::default(),
    };
    let mut info = BLUETOOTH_DEVICE_INFO { dwSize: size_of::<BLUETOOTH_DEVICE_INFO>() as u32, ..Default::default() };
    // SAFETY: both structs are initialised with their dwSize; the handle
    // returned is closed below.
    let find = match unsafe { BluetoothFindFirstDevice(&params, &mut info) } {
        Ok(h) => h,
        Err(e) if e.code() == ERROR_NO_MORE_ITEMS.to_hresult() => {
            tracing::debug!("bt_paired: Windows knows no Bluetooth devices");
            return Ok(Vec::new());
        }
        Err(e) => {
            tracing::debug!("bt_paired: BluetoothFindFirstDevice failed: {e}");
            return Err(e);
        }
    };
    let mut devices = vec![device_from_info(&info)];
    loop {
        info = BLUETOOTH_DEVICE_INFO { dwSize: size_of::<BLUETOOTH_DEVICE_INFO>() as u32, ..Default::default() };
        // SAFETY: `find` is the open search handle; `info` is initialised
        // with its dwSize.
        if let Err(e) = unsafe { BluetoothFindNextDevice(find, &mut info) } {
            if e.code() != ERROR_NO_MORE_ITEMS.to_hresult() {
                tracing::debug!("bt_paired: BluetoothFindNextDevice stopped: {e}");
            }
            break;
        }
        devices.push(device_from_info(&info));
    }
    // SAFETY: the handle was opened above and is closed once.
    let _ = unsafe { BluetoothFindDeviceClose(find) };
    tracing::debug!("bt_paired: {} device(s)", devices.len());
    for d in &devices {
        tracing::debug!(
            "bt_paired: {} \u{201C}{}\u{201D} CoD 0x{:06X} (A2DP sink {}), connected {}, remembered {}, authenticated {}",
            d.address_string(),
            d.name,
            d.class_of_device,
            d.is_a2dp_sink(),
            d.connected,
            d.remembered,
            d.authenticated
        );
    }
    Ok(devices)
}

/// The first A2DP sink in `devices` (v1's automatic choice without -d)
pub fn first_a2dp_sink(devices: &[PairedDevice]) -> Option<&PairedDevice> {
    devices.iter().find(|d| d.is_a2dp_sink())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(cod: u32) -> PairedDevice {
        PairedDevice {
            address: [0; 6],
            name: String::new(),
            class_of_device: cod,
            connected: false,
            remembered: true,
            authenticated: true,
        }
    }

    #[test]
    fn a2dp_sink_by_major_class() {
        // Headphones (Audio/Video, minor 6), rendering + audio services
        assert!(is_a2dp_sink_class(0x240418));
        // Wearable headset
        assert!(is_a2dp_sink_class(0x200404));
        // Loudspeaker, car audio
        assert!(is_a2dp_sink_class(0x240414));
        assert!(is_a2dp_sink_class(0x200420));
        // Phone (major 2), computer (1), peripheral (5), uncategorised
        assert!(!is_a2dp_sink_class(0x5A020C));
        assert!(!is_a2dp_sink_class(0x10010C));
        assert!(!is_a2dp_sink_class(0x002540));
        assert!(!is_a2dp_sink_class(0x001F00));
        assert!(!is_a2dp_sink_class(0));
    }

    #[test]
    fn first_sink_is_v1s_choice() {
        let mut phone = dev(0x5A020C);
        phone.name = "phone".into();
        let mut a = dev(0x240404);
        a.name = "first".into();
        let mut b = dev(0x240418);
        b.name = "second".into();
        let list = vec![phone.clone(), a, b];
        assert_eq!(first_a2dp_sink(&list).map(|d| d.name.as_str()), Some("first"));
        assert!(first_a2dp_sink(&[phone]).is_none());
        assert!(first_a2dp_sink(&[]).is_none());
    }

    #[test]
    fn bth_addr_to_display_order() {
        let a = address_from_bth_addr(0x0000_0011_2233_4455_u64 | (0xAA << 40));
        assert_eq!(a, [0xAA, 0x11, 0x22, 0x33, 0x44, 0x55]);
        assert_eq!(format_address(&a), "AA:11:22:33:44:55");
        // Bits above the 48-bit address are ignored
        assert_eq!(address_from_bth_addr(0xFFFF_0000_0000_0001), [0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn name_stops_at_nul() {
        let mut w = [0u16; 8];
        for (i, c) in "WH-1000".encode_utf16().enumerate() {
            w[i] = c;
        }
        assert_eq!(name_from_wide(&w), "WH-1000");
        assert_eq!(name_from_wide(&[]), "");
    }

    #[test]
    fn listing_does_not_fail_hard() {
        // Reads Windows' list only (no inquiry, no adapter access); a PC
        // without Bluetooth support may return an error, which is fine
        if let Ok(list) = paired_devices() {
            for d in list {
                assert_eq!(d.address_string().len(), 17);
            }
        }
    }
}
