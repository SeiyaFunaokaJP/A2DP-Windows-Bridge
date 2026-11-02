//! The Bluetooth radios Windows' own Bluetooth stack runs: the name Device
//! Manager shows, the address and the versions (its Advanced tab), and the
//! USB device each sits on
//!
//! Every radio registers GUID_BTHPORT_DEVICE_INTERFACE; IOCTL_BTH_GET_LOCAL_INFO
//! on it returns BTH_LOCAL_RADIO_INFO (bthioctl.h). A radio of a composite
//! device (MediaTek: `USB\VID_0E8D&PID_7925&MI_00\...`) is a child of the
//! USB device `usb_adapters` lists, so the device tree is walked up to it.
//!
//! Windows' stack also stores what it read as properties of the radio's
//! device node (DEVPKEY_BluetoothRadio_*: {A92F26CA-...}). A radio the IOCTL
//! cannot be asked on (a vendor's driver that does not pass it on, Windows'
//! Bluetooth switched off) or with no BTHPORT interface at all may still
//! have them, so they are read when the IOCTL gives nothing (issue #86).

use windows::Devices::Radios::{Radio, RadioKind, RadioState};
use windows::Win32::Devices::Bluetooth::GUID_BTHPORT_DEVICE_INTERFACE;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_Child, CM_Get_DevNode_PropertyW, CM_Get_Device_IDW, CM_Get_Parent, CM_Get_Sibling, CM_LOCATE_DEVNODE_NORMAL,
    CM_Locate_DevNodeW, CR_SUCCESS, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA, SPDRP_DEVICEDESC,
    SPDRP_FRIENDLYNAME, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
};
use windows::Win32::Devices::Properties::{DEVPROP_TYPE_BYTE, DEVPROP_TYPE_UINT64, DEVPROPTYPE};
use windows::Win32::Foundation::{DEVPROPKEY, ERROR_NO_MORE_ITEMS};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::core::{GUID, HSTRING, PCWSTR};

use crate::usb_adapters::{DevInfoSet, OwnedHandle};

/// A radio of Windows' Bluetooth stack
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsRadio {
    /// The radio's device node and its parents up to the root
    /// (`USB\VID_0E8D&PID_7925&MI_00\...`, `USB\VID_0E8D&PID_7925\...`, ...)
    pub instance_ids: Vec<String>,
    /// Device Manager's name ("MediaTek Bluetooth Adapter")
    pub name: String,
    /// "C0:E3:50:2E:00:09"; None if the radio could not be asked
    pub address: Option<String>,
    /// HCI version (12 = Bluetooth 5.3)
    pub hci_version: Option<u8>,
    /// LMP version (the same numbering)
    pub lmp_version: Option<u8>,
}

impl WindowsRadio {
    /// The radio sits on this device (itself or one of its parents)
    pub fn on_device(&self, instance_id: &str) -> bool {
        self.instance_ids.iter().any(|i| i.eq_ignore_ascii_case(instance_id))
    }
}

/// What Windows' stack stored on a radio's device node
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RadioProperties {
    /// BTH_ADDR
    pub address: Option<u64>,
    pub hci_version: Option<u8>,
    pub lmp_version: Option<u8>,
}

impl RadioProperties {
    fn is_empty(&self) -> bool {
        self.address.is_none() && self.hci_version.is_none() && self.lmp_version.is_none()
    }
}

impl WindowsRadio {
    /// The values the IOCTL did not give, from the device node's properties
    pub fn fill_from(&mut self, p: &RadioProperties) {
        if self.address.is_none() {
            self.address = p.address.map(format_address);
        }
        self.hci_version = self.hci_version.or(p.hci_version);
        self.lmp_version = self.lmp_version.or(p.lmp_version);
    }
}

/// The radio of a USB device as its device node (or a child's: a composite
/// device's Bluetooth function) stores it, for one `radios` did not find or
/// could not ask; None if Windows' stack never stored a radio there
pub fn device_radio(instance_id: &str) -> Option<WindowsRadio> {
    let mut devinst = 0u32;
    // SAFETY: NUL-terminated instance ID and out pointer.
    if unsafe { CM_Locate_DevNodeW(&mut devinst, &HSTRING::from(instance_id), CM_LOCATE_DEVNODE_NORMAL) } != CR_SUCCESS
    {
        return None;
    }
    let props = subtree_properties(devinst, 2)?;
    let mut radio = WindowsRadio {
        instance_ids: vec![instance_id.to_owned()],
        name: String::new(),
        address: None,
        hci_version: None,
        lmp_version: None,
    };
    radio.fill_from(&props);
    tracing::debug!("bt_radios: from the device node of {instance_id}: {radio:?}");
    Some(radio)
}

/// The first device node at or below `devinst` (`depth` levels of children)
/// with radio properties
fn subtree_properties(devinst: u32, depth: u32) -> Option<RadioProperties> {
    let own = node_properties(devinst);
    if !own.is_empty() {
        return Some(own);
    }
    if depth == 0 {
        return None;
    }
    let mut child = 0u32;
    // SAFETY: valid devinst and out pointer.
    if unsafe { CM_Get_Child(&mut child, devinst, 0) } != CR_SUCCESS {
        return None;
    }
    // A composite device has a few functions; the bound guards a broken tree
    for _ in 0..16 {
        if let Some(p) = subtree_properties(child, depth - 1) {
            return Some(p);
        }
        let mut next = 0u32;
        // SAFETY: valid devinst and out pointer.
        if unsafe { CM_Get_Sibling(&mut next, child, 0) } != CR_SUCCESS {
            break;
        }
        child = next;
    }
    None
}

/// DEVPKEY_BluetoothRadio_* (not in the SDK's headers; Device Manager's
/// Details tab lists them): 1 address, 4 LMP version, 6 HCI version
const RADIO_PROPERTIES: GUID = GUID::from_u128(0xA92F26CA_EDA7_4B1D_9DB2_27B68AA5A2EB);
const RADIO_ADDRESS: u32 = 1;
const RADIO_LMP_VERSION: u32 = 4;
const RADIO_HCI_VERSION: u32 = 6;

fn node_properties(devinst: u32) -> RadioProperties {
    RadioProperties {
        address: node_property::<8>(devinst, RADIO_ADDRESS, DEVPROP_TYPE_UINT64)
            .map(u64::from_le_bytes)
            .filter(|&a| a != 0),
        lmp_version: node_property::<1>(devinst, RADIO_LMP_VERSION, DEVPROP_TYPE_BYTE).map(|b| b[0]),
        hci_version: node_property::<1>(devinst, RADIO_HCI_VERSION, DEVPROP_TYPE_BYTE).map(|b| b[0]),
    }
}

/// A fixed-size property of the radio set, None if absent or of another type
fn node_property<const N: usize>(devinst: u32, pid: u32, want: DEVPROPTYPE) -> Option<[u8; N]> {
    let key = DEVPROPKEY { fmtid: RADIO_PROPERTIES, pid };
    let mut ty = DEVPROPTYPE::default();
    let mut buf = [0u8; N];
    let mut size = N as u32;
    // SAFETY: valid key, out type and a buffer of the size passed.
    let cr = unsafe { CM_Get_DevNode_PropertyW(devinst, &key, &mut ty, Some(buf.as_mut_ptr()), &mut size, 0) };
    (cr == CR_SUCCESS && ty == want && size as usize == N).then_some(buf)
}

/// BTH_DEVICE_INFO (bthdef.h)
#[repr(C)]
struct BthDeviceInfo {
    flags: u32,
    address: u64,
    class_of_device: u32,
    name: [u8; 248],
}

/// BTH_RADIO_INFO (bthdef.h)
#[repr(C)]
struct BthRadioInfo {
    lmp_supported_features: u64,
    mfg: u16,
    lmp_subversion: u16,
    lmp_version: u8,
}

/// BTH_LOCAL_RADIO_INFO (bthioctl.h)
#[repr(C)]
struct BthLocalRadioInfo {
    local_info: BthDeviceInfo,
    flags: u32,
    hci_revision: u16,
    hci_version: u8,
    radio_info: BthRadioInfo,
}

/// CTL_CODE(FILE_DEVICE_BLUETOOTH, BTH_IOCTL_BASE + 0, METHOD_BUFFERED,
/// FILE_ANY_ACCESS)
const IOCTL_BTH_GET_LOCAL_INFO: u32 = 0x0041_0000;

/// The present radios of Windows' Bluetooth (none if it runs none)
pub fn radios() -> Vec<WindowsRadio> {
    // SAFETY: interface class GUID and flags as documented; no window.
    let set = match unsafe {
        SetupDiGetClassDevsW(
            Some(&GUID_BTHPORT_DEVICE_INTERFACE),
            PCWSTR::null(),
            None,
            DIGCF_DEVICEINTERFACE | DIGCF_PRESENT,
        )
    } {
        Ok(s) => DevInfoSet(s),
        Err(e) => {
            tracing::debug!("bt_radios: SetupDiGetClassDevs failed: {e}");
            return Vec::new();
        }
    };
    let mut result = Vec::new();
    for index in 0.. {
        let mut intf =
            SP_DEVICE_INTERFACE_DATA { cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32, ..Default::default() };
        // SAFETY: valid set, GUID and out structure with its size set.
        let next =
            unsafe { SetupDiEnumDeviceInterfaces(set.0, None, &GUID_BTHPORT_DEVICE_INTERFACE, index, &mut intf) };
        if let Err(e) = next {
            if e.code() != ERROR_NO_MORE_ITEMS.to_hresult() {
                tracing::debug!("bt_radios: SetupDiEnumDeviceInterfaces stopped: {e}");
            }
            break;
        }
        let Some((path, data)) = set.interface_detail(&intf) else { continue };
        let name = set
            .registry_string(&data, SPDRP_FRIENDLYNAME)
            .or_else(|| set.registry_string(&data, SPDRP_DEVICEDESC))
            .unwrap_or_default();
        let info = local_info(&path);
        let mut radio = WindowsRadio {
            instance_ids: ancestors(data.DevInst),
            name,
            address: info.as_ref().map(|i| format_address(i.local_info.address)),
            hci_version: info.as_ref().map(|i| i.hci_version),
            lmp_version: info.as_ref().map(|i| i.radio_info.lmp_version),
        };
        if radio.address.is_none() {
            radio.fill_from(&node_properties(data.DevInst));
        }
        tracing::debug!("bt_radios: {radio:?}");
        result.push(radio);
    }
    result
}

/// Windows' Bluetooth is switched on (Settings' switch, which leaves the
/// radio's device in place); None if Windows would not say
pub fn switched_on() -> Option<bool> {
    crate::com::keep_mta();
    let radios = || -> windows::core::Result<bool> {
        let list = Radio::GetRadiosAsync()?.join()?;
        for r in &list {
            if r.Kind()? == RadioKind::Bluetooth && r.State()? == RadioState::On {
                return Ok(true);
            }
        }
        Ok(false)
    };
    radios().map_err(|e| tracing::debug!("bt_radios: GetRadiosAsync failed: {e}")).ok()
}

fn local_info(path: &str) -> Option<BthLocalRadioInfo> {
    // SAFETY: NUL-terminated path; no access rights asked (the IOCTL is
    // FILE_ANY_ACCESS), shared with Windows' stack.
    let handle = unsafe {
        CreateFileW(
            &HSTRING::from(path),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    };
    let handle = match handle {
        Ok(h) => OwnedHandle(h),
        Err(e) => {
            tracing::debug!("bt_radios: cannot open {path}: {e}");
            return None;
        }
    };
    // SAFETY: plain-old-data structure, all zero is valid.
    let mut info: BthLocalRadioInfo = unsafe { std::mem::zeroed() };
    let mut returned = 0u32;
    // SAFETY: valid handle; output buffer is the structure with its size.
    let r = unsafe {
        DeviceIoControl(
            handle.0,
            IOCTL_BTH_GET_LOCAL_INFO,
            None,
            0,
            Some(std::ptr::from_mut(&mut info).cast()),
            size_of::<BthLocalRadioInfo>() as u32,
            Some(&mut returned),
            None,
        )
    };
    if let Err(e) = r {
        tracing::debug!("bt_radios: IOCTL_BTH_GET_LOCAL_INFO on {path} failed: {e}");
        return None;
    }
    (returned as usize >= size_of::<BthLocalRadioInfo>()).then_some(info)
}

/// The device node's instance ID and its parents' up to the root
fn ancestors(devinst: u32) -> Vec<String> {
    let mut ids = Vec::new();
    let mut dev = devinst;
    // A few levels reach the USB device; the bound guards a broken tree
    for _ in 0..8 {
        let mut buf = [0u16; 512];
        // SAFETY: valid devinst; the buffer length is passed.
        if unsafe { CM_Get_Device_IDW(dev, &mut buf, 0) } != CR_SUCCESS {
            break;
        }
        ids.push(crate::autostart::utf16_string(&buf));
        let mut parent = 0u32;
        // SAFETY: valid devinst and out pointer.
        if unsafe { CM_Get_Parent(&mut parent, dev, 0) } != CR_SUCCESS {
            break;
        }
        dev = parent;
    }
    ids
}

/// "C0:E3:50:2E:00:09" from BTH_ADDR (the address in its low 48 bits)
pub fn format_address(addr: u64) -> String {
    (0..6).rev().map(|i| format!("{:02X}", (addr >> (i * 8)) & 0xFF)).collect::<Vec<_>>().join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_from_bth_addr() {
        assert_eq!(format_address(0xC0E3_502E_0009), "C0:E3:50:2E:00:09");
        assert_eq!(format_address(0x0000_0000_0001), "00:00:00:00:00:01");
    }

    #[test]
    fn local_radio_info_layout_as_bthioctl_h() {
        // x64 and arm64: BTH_DEVICE_INFO 272 bytes, the whole 296
        assert_eq!(size_of::<BthDeviceInfo>(), 272);
        assert_eq!(std::mem::offset_of!(BthLocalRadioInfo, hci_version), 278);
        assert_eq!(std::mem::offset_of!(BthLocalRadioInfo, radio_info), 280);
        assert_eq!(size_of::<BthLocalRadioInfo>(), 296);
    }

    /// This PC's radios (cargo test -p win-sys list_radios -- --ignored
    /// --nocapture)
    #[test]
    #[ignore = "reads this PC's Bluetooth radios"]
    fn list_radios() {
        for r in radios() {
            println!("{r:?}");
        }
        println!("switched on: {:?}", switched_on());
    }

    /// This PC's adapters' radio properties (cargo test -p win-sys
    /// list_device_radios -- --ignored --nocapture)
    #[test]
    #[ignore = "reads this PC's USB devices"]
    fn list_device_radios() {
        for d in crate::usb_adapters::enumerate(false).unwrap() {
            println!("{} {:?}", d.instance_id, device_radio(&d.instance_id));
        }
    }

    #[test]
    fn properties_fill_what_the_ioctl_lacks() {
        let mut r = WindowsRadio {
            instance_ids: Vec::new(),
            name: "MediaTek Bluetooth Adapter".into(),
            address: None,
            hci_version: None,
            lmp_version: Some(12),
        };
        r.fill_from(&RadioProperties { address: Some(0xC0E3_502E_0009), hci_version: Some(13), lmp_version: Some(13) });
        assert_eq!(r.address.as_deref(), Some("C0:E3:50:2E:00:09"));
        assert_eq!(r.hci_version, Some(13));
        // What the IOCTL read stays
        assert_eq!(r.lmp_version, Some(12));
        assert!(RadioProperties::default().is_empty());
    }

    #[test]
    fn radio_on_its_usb_device() {
        let r = WindowsRadio {
            instance_ids: vec![r"USB\VID_0E8D&PID_7925&MI_00\7&1".into(), r"USB\VID_0E8D&PID_7925\000000000".into()],
            name: "MediaTek Bluetooth Adapter".into(),
            address: None,
            hci_version: None,
            lmp_version: None,
        };
        assert!(r.on_device(r"usb\vid_0e8d&pid_7925\000000000"));
        assert!(!r.on_device(r"USB\VID_0BDA&PID_A729\00E04C000001"));
    }
}
