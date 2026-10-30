//! USB devices that may be Bluetooth adapters (the Windows part of v1
//! bt_adapter_enum.cpp)
//!
//! Every present USB device (GUID_DEVINTERFACE_USB_DEVICE, the interface the
//! hub registers for each device, whatever its driver) is listed with what
//! decides whether BlueCaster can use it: VID:PID, the USB class from its
//! compatible IDs, the driver that owns it (BTHUSB = Windows' Bluetooth,
//! WINUSB = usable by BlueCaster, a vendor driver, or none) with the INF name
//! and version (to check a restore), and for a WinUSB device the class of
//! interface 0 read through WinUSB (v1 is_bluetooth_class: E0/01/01, or
//! the vendor class FF/01/01 with the HCI endpoints, as on the ASUS BT400).
//! A device whose driver failed to start has no such interface and is
//! found among the USB enumerator's device nodes instead.
//!
//! Which VID:PID are Realtek chips is bt-host's chips table; this module
//! knows nothing of chips. `UsbDevice::looks_like_bluetooth` is v1's
//! "worth a log line" test without the chip table.
//!
//! Also holds the SetupAPI helpers the driver module shares.

use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_DEVNODE_STATUS_FLAGS, CM_Get_DevNode_Status, CM_PROB, CR_SUCCESS, DIGCF_ALLCLASSES, DIGCF_DEVICEINTERFACE,
    DIGCF_PRESENT, DN_HAS_PROBLEM, HDEVINFO, SETUP_DI_REGISTRY_PROPERTY, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA, SPDRP_CLASS, SPDRP_COMPATIBLEIDS, SPDRP_DEVICEDESC,
    SPDRP_FRIENDLYNAME, SPDRP_HARDWAREID, SPDRP_MFG, SPDRP_SERVICE, SetupDiCreateDeviceInfoList,
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInstanceIdW, SetupDiGetDeviceInterfaceDetailW, SetupDiGetDevicePropertyW,
    SetupDiGetDeviceRegistryPropertyW, SetupDiOpenDeviceInfoW,
};
use windows::Win32::Devices::Properties::{
    DEVPKEY_Device_BusReportedDeviceDesc, DEVPKEY_Device_DriverDesc, DEVPKEY_Device_DriverInfPath,
    DEVPKEY_Device_DriverInfSection, DEVPKEY_Device_DriverProvider, DEVPKEY_Device_DriverVersion, DEVPROP_TYPE_STRING,
    DEVPROPTYPE,
};
use windows::Win32::Devices::Usb::{
    GUID_DEVINTERFACE_USB_DEVICE, USB_INTERFACE_DESCRIPTOR, USBD_PIPE_TYPE, UsbdPipeTypeBulk, UsbdPipeTypeInterrupt,
    WINUSB_INTERFACE_HANDLE, WINUSB_PIPE_INFORMATION, WinUsb_Free, WinUsb_Initialize, WinUsb_QueryInterfaceSettings,
    WinUsb_QueryPipe,
};
use windows::Win32::Foundation::{
    CloseHandle, DEVPROPKEY, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, GENERIC_READ, GENERIC_WRITE, HANDLE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::core::{GUID, HSTRING, PCWSTR, w};

use crate::driver::{DriverInfo, DriverKind};

/// Interface class / subclass / protocol of a USB device or interface
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbClass {
    pub class: u8,
    pub subclass: u8,
    pub protocol: u8,
}

impl UsbClass {
    /// E0/01/01: Bluetooth programming interface (the standard class)
    pub const BLUETOOTH: UsbClass = UsbClass { class: 0xE0, subclass: 0x01, protocol: 0x01 };
    /// FF/01/01: vendor class some Broadcom-based adapters report
    pub const VENDOR_BLUETOOTH: UsbClass = UsbClass { class: 0xFF, subclass: 0x01, protocol: 0x01 };
}

impl std::fmt::Display for UsbClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02X}/{:02X}/{:02X}", self.class, self.subclass, self.protocol)
    }
}

/// Interface 0 as WinUSB reports it (only for WinUSB-bound devices)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceProbe {
    pub class: UsbClass,
    pub num_endpoints: u8,
    /// Interrupt IN (events), bulk IN and bulk OUT (ACL data) are present
    pub hci_endpoints: bool,
}

/// One present USB device
#[derive(Debug, Clone)]
pub struct UsbDevice {
    /// e.g. `USB\VID_0BDA&PID_8771\00E04C239987`; what the driver module and
    /// the helper take to name the device
    pub instance_id: String,
    /// Path of its GUID_DEVINTERFACE_USB_DEVICE interface (for CreateFile);
    /// empty for a device without an active one (its driver failed)
    pub device_path: String,
    pub vid: u16,
    pub pid: u16,
    /// FriendlyName, else the device description
    pub name: String,
    /// The product string the device itself reports, if any
    pub bus_reported_name: Option<String>,
    pub manufacturer: String,
    pub hardware_ids: Vec<String>,
    pub compatible_ids: Vec<String>,
    /// From the compatible IDs (`USB\Class_E0&SubClass_01&Prot_01`)
    pub usb_class: Option<UsbClass>,
    pub driver: DriverInfo,
    pub driver_kind: DriverKind,
    /// Device Manager's problem code (28 = no driver, ...), None if fine
    pub problem_code: Option<u32>,
    /// WinUSB-bound devices only: interface 0, or why it could not be
    /// read ("open failed (error 5)" = another program uses the adapter)
    pub probe: Option<Result<InterfaceProbe, String>>,
}

impl UsbDevice {
    /// The name to show: the product string the device reports, else
    /// FriendlyName / description. FriendlyName can be stale: a vendor
    /// driver's INF writes it and switching to WinUSB leaves it behind
    /// (a UB500 showed "TP-TP+ UB500 Adapter").
    pub fn display_name(&self) -> &str {
        self.bus_reported_name.as_deref().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&self.name)
    }

    /// The compatible IDs name the Bluetooth class (E0/01/01), whatever
    /// driver owns the device
    pub fn has_bluetooth_compat_id(&self) -> bool {
        self.compatible_ids.iter().any(|id| id.to_ascii_uppercase().starts_with(r"USB\CLASS_E0&SUBCLASS_01&PROT_01"))
    }

    /// Interface 0, read through WinUSB, is Bluetooth: Some(false) for the
    /// standard class, Some(true) for the vendor class FF/01/01 with the
    /// HCI endpoints (BTstack then needs the VID:PID registered), None if
    /// not Bluetooth or not probed
    pub fn winusb_bluetooth(&self) -> Option<bool> {
        match &self.probe {
            Some(Ok(p)) if p.class == UsbClass::BLUETOOTH => Some(false),
            Some(Ok(p)) if p.class == UsbClass::VENDOR_BLUETOOTH && p.hci_endpoints => Some(true),
            _ => None,
        }
    }

    /// WinUSB refused to open the device for the probe: another program
    /// (v1, another BlueCaster) holds it
    pub fn in_use(&self) -> bool {
        matches!(&self.probe, Some(Err(e)) if e.starts_with("open failed (error 5)"))
    }

    /// Worth listing as a possible adapter: WinUSB-bound, Bluetooth class
    /// or compatible ID, or Windows' Bluetooth setup class. A vendor-class
    /// adapter without a driver is caught only by a VID known to bt-host's
    /// chip table, which the caller adds.
    pub fn looks_like_bluetooth(&self) -> bool {
        self.driver_kind == DriverKind::WinUsb
            || self.driver_kind == DriverKind::Bluetooth
            || self.has_bluetooth_compat_id()
            || self.usb_class == Some(UsbClass::BLUETOOTH)
            || self.winusb_bluetooth().is_some()
    }
}

/// All present USB devices (hubs included; filter with
/// `looks_like_bluetooth` and the chip table). With `probe_winusb`, each
/// WinUSB-bound device is opened for a moment to read interface 0; one in
/// use by another program (v1, or a running BlueCaster) cannot be opened and
/// says so in `probe`, nothing else happens to it.
///
/// A device whose function driver failed to start (Code 31, as a vendor
/// Bluetooth driver after a restore) has no active USB device interface;
/// those come from a second pass over the USB enumerator's device nodes,
/// with an empty `device_path` and their `problem_code`, so the adapter
/// can still be given WinUSB.
pub fn enumerate(probe_winusb: bool) -> windows::core::Result<Vec<UsbDevice>> {
    // SAFETY: interface class GUID and flags as documented; no window.
    let set = DevInfoSet(unsafe {
        SetupDiGetClassDevsW(
            Some(&GUID_DEVINTERFACE_USB_DEVICE),
            PCWSTR::null(),
            None,
            DIGCF_DEVICEINTERFACE | DIGCF_PRESENT,
        )
    }?);
    let mut result = Vec::new();
    for index in 0.. {
        let mut intf =
            SP_DEVICE_INTERFACE_DATA { cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32, ..Default::default() };
        // SAFETY: valid set, GUID and out structure with its size set.
        let next = unsafe { SetupDiEnumDeviceInterfaces(set.0, None, &GUID_DEVINTERFACE_USB_DEVICE, index, &mut intf) };
        if let Err(e) = next {
            if e.code() != ERROR_NO_MORE_ITEMS.to_hresult() {
                tracing::debug!("usb_adapters: SetupDiEnumDeviceInterfaces stopped: {e}");
            }
            break;
        }
        let Some((path, data)) = set.interface_detail(&intf) else { continue };
        let Some(instance_id) = set.instance_id(&data) else { continue };
        if let Some(d) = set.usb_device(&data, instance_id, path, probe_winusb) {
            result.push(d);
        }
    }
    match devices_without_interface(&result) {
        Ok(extra) => result.extend(extra),
        Err(e) => tracing::debug!("usb_adapters: listing USB device nodes failed: {e}"),
    }
    tracing::debug!(
        "usb_adapters: {} USB device(s), {} look like Bluetooth (probe {probe_winusb})",
        result.len(),
        result.iter().filter(|d| d.looks_like_bluetooth()).count()
    );
    Ok(result)
}

/// Present device nodes of the USB enumerator that `listed` does not have
/// (no active USB device interface: a failed or missing driver)
fn devices_without_interface(listed: &[UsbDevice]) -> windows::core::Result<Vec<UsbDevice>> {
    // SAFETY: enumerator name and flags as documented; no window.
    let set = DevInfoSet(unsafe { SetupDiGetClassDevsW(None, w!("USB"), None, DIGCF_ALLCLASSES | DIGCF_PRESENT) }?);
    let listed: Vec<&str> = listed.iter().map(|d| d.instance_id.as_str()).collect();
    let mut result = Vec::new();
    for index in 0.. {
        let mut data = SP_DEVINFO_DATA { cbSize: size_of::<SP_DEVINFO_DATA>() as u32, ..Default::default() };
        // SAFETY: valid set; out structure with its size set.
        if let Err(e) = unsafe { SetupDiEnumDeviceInfo(set.0, index, &mut data) } {
            if e.code() != ERROR_NO_MORE_ITEMS.to_hresult() {
                tracing::debug!("usb_adapters: SetupDiEnumDeviceInfo stopped: {e}");
            }
            break;
        }
        let Some(instance_id) = set.instance_id(&data) else { continue };
        if !is_unlisted_usb_device(&instance_id, &listed) {
            continue;
        }
        if let Some(d) = set.usb_device(&data, instance_id, String::new(), false) {
            tracing::debug!(
                "usb_adapters: {} has no USB device interface (problem code {:?})",
                d.instance_id,
                d.problem_code
            );
            result.push(d);
        }
    }
    Ok(result)
}

/// A device node of the USB enumerator worth adding to the interface list:
/// a device with a VID:PID (not a root hub, not one interface of a
/// composite device, `&MI_xx`) not listed already
fn is_unlisted_usb_device(instance_id: &str, listed: &[&str]) -> bool {
    let upper = instance_id.to_ascii_uppercase();
    let Some(hw) = upper.strip_prefix(r"USB\").and_then(|rest| rest.split('\\').next()) else { return false };
    !hw.starts_with("ROOT_HUB")
        && !hw.contains("&MI_")
        && parse_vid_pid(hw).is_some()
        && !listed.iter().any(|l| l.eq_ignore_ascii_case(instance_id))
}

/// VID and PID from an instance ID or device path (`VID_0BDA&PID_8771`,
/// either case)
pub fn parse_vid_pid(s: &str) -> Option<(u16, u16)> {
    let upper = s.to_ascii_uppercase();
    let hex4 = |tag: &str| -> Option<u16> {
        let at = upper.find(tag)? + tag.len();
        u16::from_str_radix(upper.get(at..at + 4)?, 16).ok()
    };
    Some((hex4("VID_")?, hex4("PID_")?))
}

/// The class triple of a compatible ID `USB\Class_E0&SubClass_01&Prot_01`
/// (None for `USB\Class_E0&SubClass_01`, `USB\COMPOSITE`, ...)
pub fn parse_usb_class_id(id: &str) -> Option<UsbClass> {
    let upper = id.to_ascii_uppercase();
    let rest = upper.strip_prefix(r"USB\CLASS_")?;
    let mut parts = rest.split('&');
    let class = u8::from_str_radix(parts.next()?, 16).ok()?;
    let subclass = u8::from_str_radix(parts.next()?.strip_prefix("SUBCLASS_")?, 16).ok()?;
    let protocol = u8::from_str_radix(parts.next()?.strip_prefix("PROT_")?, 16).ok()?;
    Some(UsbClass { class, subclass, protocol })
}

/// Device Manager's problem code of a present device node, None if it has
/// none (or is not present)
pub(crate) fn problem_code(devinst: u32) -> Option<u32> {
    let mut status = CM_DEVNODE_STATUS_FLAGS::default();
    let mut problem = CM_PROB::default();
    // SAFETY: valid out pointers; an unknown devinst only returns an error.
    let cr = unsafe { CM_Get_DevNode_Status(&mut status, &mut problem, devinst, 0) };
    (cr == CR_SUCCESS && (status.0 & DN_HAS_PROBLEM.0) != 0).then_some(problem.0)
}

/// Interface 0 of a WinUSB device: its class and whether it has the HCI
/// endpoints (v1 is_bluetooth_class / has_hci_endpoints)
fn probe_interface(path: &str) -> Result<InterfaceProbe, String> {
    tracing::debug!("usb_adapters: probing {path}");
    // SAFETY: NUL-terminated path; the handle is closed below.
    let file = unsafe {
        CreateFileW(
            &HSTRING::from(path),
            (GENERIC_READ | GENERIC_WRITE).0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED,
            None,
        )
    }
    .map_err(|e| format!("open failed (error {})", win32_code(&e)))?;
    let file = OwnedHandle(file);
    let mut usb = WINUSB_INTERFACE_HANDLE::default();
    // SAFETY: an open, overlapped handle of a WinUSB device; out pointer.
    unsafe { WinUsb_Initialize(file.0, &mut usb) }
        .map_err(|e| format!("WinUsb_Initialize failed (error {})", win32_code(&e)))?;
    let mut desc = USB_INTERFACE_DESCRIPTOR::default();
    // SAFETY: initialised WinUSB handle; out structure.
    let settings = unsafe { WinUsb_QueryInterfaceSettings(usb, 0, &mut desc) };
    let result = match settings {
        Err(e) => Err(format!("interface query failed (error {})", win32_code(&e))),
        Ok(()) => {
            let (mut intr_in, mut bulk_in, mut bulk_out) = (false, false, false);
            for i in 0..desc.bNumEndpoints {
                let mut pipe = WINUSB_PIPE_INFORMATION::default();
                // SAFETY: initialised WinUSB handle; index below the count.
                if let Err(e) = unsafe { WinUsb_QueryPipe(usb, 0, i, &mut pipe) } {
                    tracing::debug!("usb_adapters: probe: WinUsb_QueryPipe({i}) failed (error {})", win32_code(&e));
                    continue;
                }
                tracing::debug!(
                    "usb_adapters: probe: pipe 0x{:02x} type {}, max packet {}, interval {}",
                    pipe.PipeId,
                    pipe.PipeType.0,
                    pipe.MaximumPacketSize,
                    pipe.Interval
                );
                let input = pipe.PipeId & 0x80 != 0;
                match pipe.PipeType {
                    t if t == UsbdPipeTypeInterrupt && input => intr_in = true,
                    t if t == UsbdPipeTypeBulk => {
                        if input {
                            bulk_in = true
                        } else {
                            bulk_out = true
                        }
                    }
                    USBD_PIPE_TYPE(_) => {}
                }
            }
            Ok(InterfaceProbe {
                class: UsbClass {
                    class: desc.bInterfaceClass,
                    subclass: desc.bInterfaceSubClass,
                    protocol: desc.bInterfaceProtocol,
                },
                num_endpoints: desc.bNumEndpoints,
                hci_endpoints: intr_in && bulk_in && bulk_out,
            })
        }
    };
    // SAFETY: initialised above and freed only here (before the file).
    let _ = unsafe { WinUsb_Free(usb) };
    tracing::debug!(
        "usb_adapters: probe: interface 0 alt {}, {} endpoint(s): {result:?}",
        desc.bAlternateSetting,
        desc.bNumEndpoints
    );
    result
}

/// The Win32 error code inside an HRESULT_FROM_WIN32 (or the HRESULT)
pub(crate) fn win32_code(e: &windows::core::Error) -> u32 {
    let hr = e.code().0 as u32;
    if hr & 0xFFFF_0000 == 0x8007_0000 { hr & 0xFFFF } else { hr }
}

/// `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`, as the registry writes GUIDs
pub fn format_guid(g: &GUID) -> String {
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        g.data1,
        g.data2,
        g.data3,
        g.data4[0],
        g.data4[1],
        g.data4[2],
        g.data4[3],
        g.data4[4],
        g.data4[5],
        g.data4[6],
        g.data4[7]
    )
}

/// Strings of a REG_MULTI_SZ buffer (u16 units)
pub(crate) fn parse_multi_sz(data: &[u16]) -> Vec<String> {
    data.split(|&c| c == 0).take_while(|s| !s.is_empty()).map(String::from_utf16_lossy).collect()
}

pub(crate) struct OwnedHandle(pub(crate) HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: opened by CreateFileW and closed only here.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// A SetupAPI device information set, destroyed on drop (with any driver
/// lists built on it)
pub(crate) struct DevInfoSet(pub(crate) HDEVINFO);

impl Drop for DevInfoSet {
    fn drop(&mut self) {
        // SAFETY: the set was created by this wrapper's owner and is
        // destroyed only here.
        let _ = unsafe { SetupDiDestroyDeviceInfoList(self.0) };
    }
}

impl DevInfoSet {
    /// A set holding just the device `instance_id` (present or not)
    pub(crate) fn open(instance_id: &str) -> windows::core::Result<(DevInfoSet, SP_DEVINFO_DATA)> {
        // SAFETY: no class, no window.
        let set = DevInfoSet(unsafe { SetupDiCreateDeviceInfoList(None, None) }?);
        let mut data = SP_DEVINFO_DATA { cbSize: size_of::<SP_DEVINFO_DATA>() as u32, ..Default::default() };
        // SAFETY: valid set, NUL-terminated ID, out structure with its size.
        unsafe { SetupDiOpenDeviceInfoW(set.0, &HSTRING::from(instance_id), None, 0, Some(&mut data)) }?;
        Ok((set, data))
    }

    pub(crate) fn interface_detail(&self, intf: &SP_DEVICE_INTERFACE_DATA) -> Option<(String, SP_DEVINFO_DATA)> {
        let mut size = 0u32;
        // SAFETY: size query (no buffer); fails with INSUFFICIENT_BUFFER.
        let _ = unsafe { SetupDiGetDeviceInterfaceDetailW(self.0, intf, None, 0, Some(&mut size), None) };
        if (size as usize) < size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() {
            return None;
        }
        // u32 units keep the structure aligned
        let mut buf = vec![0u32; (size as usize).div_ceil(4)];
        let detail = buf.as_mut_ptr().cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
        // SAFETY: the buffer is at least `size` bytes and aligned; cbSize is
        // the fixed structure size, as the API requires.
        unsafe { (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32 };
        let mut data = SP_DEVINFO_DATA { cbSize: size_of::<SP_DEVINFO_DATA>() as u32, ..Default::default() };
        // SAFETY: buffer of `size` bytes as queried; valid out structure.
        unsafe { SetupDiGetDeviceInterfaceDetailW(self.0, intf, Some(detail), size, None, Some(&mut data)) }.ok()?;
        // SAFETY: DevicePath is a NUL-terminated string inside the buffer,
        // which has `size` bytes in u16 units after its offset.
        let path = unsafe {
            let start = std::ptr::addr_of!((*detail).DevicePath).cast::<u16>();
            let units = (size as usize - (start as usize - detail as usize)) / 2;
            let s = std::slice::from_raw_parts(start, units);
            crate::autostart::utf16_string(s)
        };
        Some((path, data))
    }

    /// A device of the set as `enumerate` lists it; None without a VID:PID
    /// or for VMware's virtual USB controller adapters (as v1 skips them).
    /// The probe needs `path`, so runs only for a device with an interface.
    fn usb_device(
        &self,
        data: &SP_DEVINFO_DATA,
        instance_id: String,
        path: String,
        probe_winusb: bool,
    ) -> Option<UsbDevice> {
        let (vid, pid) = parse_vid_pid(&instance_id).or_else(|| parse_vid_pid(&path))?;
        if vid == 0x0E0F {
            tracing::debug!("usb_adapters: {instance_id}: VMware virtual device, skipped");
            return None;
        }
        let driver = self.driver_info(data);
        let driver_kind = DriverKind::classify(&driver.service, &driver.class_name);
        let compatible_ids = self.registry_multi_sz(data, SPDRP_COMPATIBLEIDS);
        let usb_class = compatible_ids.iter().find_map(|id| parse_usb_class_id(id));
        let probe =
            (probe_winusb && driver_kind == DriverKind::WinUsb && !path.is_empty()).then(|| probe_interface(&path));
        let device = UsbDevice {
            name: self
                .registry_string(data, SPDRP_FRIENDLYNAME)
                .or_else(|| self.registry_string(data, SPDRP_DEVICEDESC))
                .unwrap_or_default(),
            bus_reported_name: self.property_string(data, &DEVPKEY_Device_BusReportedDeviceDesc),
            manufacturer: self.registry_string(data, SPDRP_MFG).unwrap_or_default(),
            hardware_ids: self.registry_multi_sz(data, SPDRP_HARDWAREID),
            problem_code: problem_code(data.DevInst),
            instance_id,
            device_path: path,
            vid,
            pid,
            compatible_ids,
            usb_class,
            driver,
            driver_kind,
            probe,
        };
        tracing::debug!(
            "usb_adapters: {} {:04X}:{:04X} {:?}: service {:?} ({:?}, {} {}), class {:?}, problem {:?}, \
             compatible Bluetooth ID {}, probe {:?} -> {}",
            device.instance_id,
            device.vid,
            device.pid,
            device.display_name(),
            device.driver.service,
            device.driver_kind,
            device.driver.inf_name,
            device.driver.version,
            device.usb_class.map(|c| c.to_string()),
            device.problem_code,
            device.has_bluetooth_compat_id(),
            device.probe,
            if device.looks_like_bluetooth() { "looks like Bluetooth" } else { "not Bluetooth by itself" }
        );
        Some(device)
    }

    pub(crate) fn instance_id(&self, data: &SP_DEVINFO_DATA) -> Option<String> {
        let mut buf = [0u16; 512];
        let mut needed = 0u32;
        // SAFETY: valid set and element; the buffer length is passed.
        unsafe { SetupDiGetDeviceInstanceIdW(self.0, data, Some(&mut buf), Some(&mut needed)) }.ok()?;
        Some(crate::autostart::utf16_string(&buf))
    }

    fn registry_raw(&self, data: &SP_DEVINFO_DATA, prop: SETUP_DI_REGISTRY_PROPERTY) -> Option<Vec<u16>> {
        let mut needed = 0u32;
        // SAFETY: size query without a buffer.
        let first = unsafe { SetupDiGetDeviceRegistryPropertyW(self.0, data, prop, None, None, Some(&mut needed)) };
        match first {
            Err(e) if e.code() == ERROR_INSUFFICIENT_BUFFER.to_hresult() => {}
            _ => return None, // absent (or empty)
        }
        let mut buf = vec![0u8; needed as usize + 4];
        // SAFETY: buffer of the size queried (plus spare).
        unsafe { SetupDiGetDeviceRegistryPropertyW(self.0, data, prop, None, Some(&mut buf), Some(&mut needed)) }
            .ok()?;
        buf.truncate(needed as usize);
        Some(buf.as_chunks::<2>().0.iter().map(|b| u16::from_le_bytes(*b)).collect())
    }

    pub(crate) fn registry_string(&self, data: &SP_DEVINFO_DATA, prop: SETUP_DI_REGISTRY_PROPERTY) -> Option<String> {
        let s = crate::autostart::utf16_string(&self.registry_raw(data, prop)?);
        (!s.is_empty()).then_some(s)
    }

    pub(crate) fn registry_multi_sz(&self, data: &SP_DEVINFO_DATA, prop: SETUP_DI_REGISTRY_PROPERTY) -> Vec<String> {
        self.registry_raw(data, prop).map(|d| parse_multi_sz(&d)).unwrap_or_default()
    }

    /// A string device property (DEVPKEY_*), None if absent
    pub(crate) fn property_string(&self, data: &SP_DEVINFO_DATA, key: &DEVPROPKEY) -> Option<String> {
        let mut ty = DEVPROPTYPE::default();
        let mut buf = vec![0u8; 1024];
        let mut needed = 0u32;
        loop {
            // SAFETY: valid set, element and key; the buffer length is
            // passed.
            let r =
                unsafe { SetupDiGetDevicePropertyW(self.0, data, key, &mut ty, Some(&mut buf), Some(&mut needed), 0) };
            match r {
                Ok(()) => break,
                Err(e) if e.code() == ERROR_INSUFFICIENT_BUFFER.to_hresult() && needed as usize > buf.len() => {
                    buf.resize(needed as usize, 0)
                }
                Err(_) => return None,
            }
        }
        if ty != DEVPROP_TYPE_STRING {
            return None;
        }
        buf.truncate(needed as usize);
        let units: Vec<u16> = buf.as_chunks::<2>().0.iter().map(|b| u16::from_le_bytes(*b)).collect();
        let s = crate::autostart::utf16_string(&units);
        (!s.is_empty()).then_some(s)
    }

    /// The driver that owns the device now (all fields empty if none)
    pub(crate) fn driver_info(&self, data: &SP_DEVINFO_DATA) -> DriverInfo {
        DriverInfo {
            service: self.registry_string(data, SPDRP_SERVICE).unwrap_or_default(),
            inf_name: self.property_string(data, &DEVPKEY_Device_DriverInfPath).unwrap_or_default(),
            inf_section: self.property_string(data, &DEVPKEY_Device_DriverInfSection).unwrap_or_default(),
            version: self.property_string(data, &DEVPKEY_Device_DriverVersion).unwrap_or_default(),
            provider: self.property_string(data, &DEVPKEY_Device_DriverProvider).unwrap_or_default(),
            description: self.property_string(data, &DEVPKEY_Device_DriverDesc).unwrap_or_default(),
            class_name: self.registry_string(data, SPDRP_CLASS).unwrap_or_default(),
            class_guid: if data.ClassGuid == GUID::zeroed() { String::new() } else { format_guid(&data.ClassGuid) },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vid_pid_from_ids_and_paths() {
        assert_eq!(parse_vid_pid(r"USB\VID_0BDA&PID_8771\00E04C239987"), Some((0x0BDA, 0x8771)));
        assert_eq!(
            parse_vid_pid(r"\\?\usb#vid_2357&pid_0604#00e04c000001#{a5dcbf10-6530-11d2-901f-00c04fb951ed}"),
            Some((0x2357, 0x0604))
        );
        assert_eq!(parse_vid_pid(r"USB\ROOT_HUB30\4&1234"), None);
        assert_eq!(parse_vid_pid(r"USB\VID_0BDA&PID_87"), None);
    }

    #[test]
    fn class_from_compatible_ids() {
        assert_eq!(parse_usb_class_id(r"USB\Class_E0&SubClass_01&Prot_01"), Some(UsbClass::BLUETOOTH));
        assert_eq!(parse_usb_class_id(r"USB\Class_ff&SubClass_01&Prot_01"), Some(UsbClass::VENDOR_BLUETOOTH));
        assert_eq!(parse_usb_class_id(r"USB\Class_E0&SubClass_01"), None);
        assert_eq!(parse_usb_class_id(r"USB\COMPOSITE"), None);
        assert_eq!(UsbClass::BLUETOOTH.to_string(), "E0/01/01");
    }

    #[test]
    fn guid_and_multi_sz_formats() {
        let g = GUID::from_u128(0x88BAE032_5A81_49F0_BC3D_A4FF138216D6);
        assert_eq!(format_guid(&g), "{88BAE032-5A81-49F0-BC3D-A4FF138216D6}");
        let data: Vec<u16> = "a\0bc\0\0".encode_utf16().collect();
        assert_eq!(parse_multi_sz(&data), ["a", "bc"]);
        assert!(parse_multi_sz(&[0, 0]).is_empty());
    }

    #[test]
    fn devnodes_added_without_interface() {
        let listed = [r"USB\VID_0BDA&PID_8771\00E04C239987"];
        // TP-Link UB500 whose vendor driver failed (Code 31): no interface
        assert!(is_unlisted_usb_device(r"USB\VID_2357&PID_0604\00E04C000001", &listed));
        // already listed through its interface (any case)
        assert!(!is_unlisted_usb_device(r"usb\vid_0bda&pid_8771\00e04c239987", &listed));
        // root hubs, a composite device's interfaces, other enumerators
        assert!(!is_unlisted_usb_device(r"USB\ROOT_HUB30\4&1234&0&0", &listed));
        assert!(!is_unlisted_usb_device(r"USB\VID_8087&PID_0026&MI_00\6&2B0A&0&0000", &listed));
        assert!(!is_unlisted_usb_device(r"BTH\MS_BTHPAN\7&1234", &listed));
        assert!(!is_unlisted_usb_device(r"USB\VID_8087&PID", &listed));
    }

    /// A UB500 with TP-Link's BTHUSB driver failed (Code 31), as the second
    /// pass lists it: still offered as an adapter
    #[test]
    fn failed_driver_still_looks_like_bluetooth() {
        let driver = DriverInfo {
            service: "BTHUSB".into(),
            inf_name: "oem91.inf".into(),
            class_name: "Bluetooth".into(),
            ..Default::default()
        };
        let mut d = UsbDevice {
            instance_id: r"USB\VID_2357&PID_0604\00E04C000001".into(),
            device_path: String::new(),
            vid: 0x2357,
            pid: 0x0604,
            name: "TP-Link Bluetooth USB Adapter".into(),
            bus_reported_name: None,
            manufacturer: String::new(),
            hardware_ids: vec![],
            compatible_ids: vec![r"USB\Class_E0&SubClass_01&Prot_01".into(), r"USB\Class_E0&SubClass_01".into()],
            usb_class: Some(UsbClass::BLUETOOTH),
            driver_kind: DriverKind::classify(&driver.service, &driver.class_name),
            driver,
            problem_code: Some(31),
            probe: None,
        };
        assert_eq!(d.driver_kind, DriverKind::Bluetooth);
        assert!(d.looks_like_bluetooth());
        // no driver left at all: the compatible ID still tells
        d.driver = DriverInfo::default();
        d.driver_kind = DriverKind::None;
        assert!(d.looks_like_bluetooth());
    }

    /// Read-only enumeration; WinUSB devices in use (e.g. by v1) are only
    /// reported as such
    #[test]
    fn enumerate_runs() {
        let devices = enumerate(true).unwrap();
        for d in devices.iter().filter(|d| d.looks_like_bluetooth()) {
            eprintln!(
                "{:04X}:{:04X} {} [{}] driver={} {} {} kind={:?} class={:?} probe={:?}",
                d.vid,
                d.pid,
                d.name,
                d.instance_id,
                d.driver.service,
                d.driver.inf_name,
                d.driver.version,
                d.driver_kind,
                d.usb_class,
                d.probe
            );
        }
    }
}
