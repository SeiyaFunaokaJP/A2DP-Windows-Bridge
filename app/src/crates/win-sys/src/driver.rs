//! Assigning Windows' WinUSB driver to a USB Bluetooth adapter, and putting
//! the adapter back on the driver Windows picks for it (design.md "ドライバ
//! （WinUSB）")
//!
//! What Zadig does with a generated, self-signed INF is done here with the
//! INF Windows ships: `%SystemRoot%\INF\winusb.inf` (Microsoft-signed, so no
//! certificate is installed). It is the same as Device Manager's "Update
//! driver > Browse > Let me pick > Have Disk > winusb.inf > WinUsb Device":
//!
//! 1. open the device by its instance ID
//! 2. write `DeviceInterfaceGUIDs` (REG_MULTI_SZ, BlueCaster's own GUID) into
//!    its `Device Parameters` key, the value Zadig's INF writes: WinUSB
//!    registers a device interface only for the GUIDs found there
//! 3. point the device's install parameters at winusb.inf alone
//!    (DI_ENUMSINGLEINF + DriverPath), build the class driver list from it
//!    and select its "WinUsb Device" model (install section WINUSB)
//! 4. DiInstallDevice
//!
//! Restoring: DiUninstallDevice, then CM_Reenumerate_DevNode on the parent
//! (the hub): the device comes back as new and Windows ranks its drivers
//! again, picking BTHUSB or the vendor driver. WinUSB was a manual choice
//! (winusb.inf matches only `USB\MS_COMP_WINUSB`), so it is not picked
//! again. The driver from before (INF name, version, service) is recorded
//! by `assign_winusb` and compared after the restore.
//!
//! Both need administrator rights (`bluecaster-driver.exe`, started elevated);
//! `driver_state` is read-only and needs none.
//!
//! Not yet verified on hardware (design.md phase 0): whether
//! SetupDiBuildDriverInfoList offers winusb.inf (class USBDevice) for a
//! device of another class (Bluetooth). If it does not, the device's class
//! is changed to USBDevice first (what installing a driver of another class
//! does anyway) and the list is built again; if that fails too, the device
//! is put back as by `restore`.

use std::time::{Duration, Instant};

use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_Parent, CM_LOCATE_DEVNODE_NORMAL, CM_Locate_DevNodeW, CM_REENUMERATE_SYNCHRONOUS, CM_Reenumerate_DevNode,
    CMP_WaitNoPendingInstallEvents, CR_SUCCESS, DI_ENUMSINGLEINF, DI_FLAGSEX_ALLOWEXCLUDEDDRVS, DI_NEEDREBOOT,
    DI_NEEDRESTART, DICS_FLAG_GLOBAL, DICS_PROPCHANGE, DIF_PROPERTYCHANGE, DIIDFLAG_NOFINISHINSTALLUI, DIREG_DEV,
    DiInstallDevice, DiUninstallDevice, SP_CLASSINSTALL_HEADER, SP_DEVINFO_DATA, SP_DEVINSTALL_PARAMS_W,
    SP_DRVINFO_DATA_V2_W, SP_DRVINFO_DETAIL_DATA_W, SP_PROPCHANGE_PARAMS, SPDIT_CLASSDRIVER, SPDRP_CLASSGUID,
    SetupDiBuildDriverInfoList, SetupDiCallClassInstaller, SetupDiCreateDevRegKeyW, SetupDiDestroyDriverInfoList,
    SetupDiEnumDriverInfoW, SetupDiGetDeviceInstallParamsW, SetupDiGetDriverInfoDetailW, SetupDiOpenDevRegKey,
    SetupDiSetClassInstallParamsW, SetupDiSetDeviceInstallParamsW, SetupDiSetDeviceRegistryPropertyW,
    SetupDiSetSelectedDriverW,
};
use windows::Win32::Foundation::{
    ERROR_INVALID_DEVINST_NAME, ERROR_NO_MORE_ITEMS, ERROR_NO_SUCH_DEVINST, HWND, WAIT_OBJECT_0,
};
use windows::Win32::System::Registry::{KEY_READ, REG_MULTI_SZ};
use windows::Win32::System::SystemInformation::GetWindowsDirectoryW;
use windows::core::{BOOL, GUID, HSTRING, PCWSTR};

use crate::autostart::RegKey;
use crate::elevation::is_elevated;
use crate::usb_adapters::{DevInfoSet, format_guid, parse_multi_sz, problem_code};

/// The device interface GUID BlueCaster registers for adapters it puts on
/// WinUSB (`DeviceInterfaceGUIDs`), so its own adapters can be told apart
/// from other WinUSB devices
pub const INTERFACE_GUID: &str = "{03D8E127-01B1-4A98-9F22-AC8E41AA0CFD}";
/// The registry value WinUSB reads its interface GUIDs from
pub const INTERFACE_GUIDS_VALUE: &str = "DeviceInterfaceGUIDs";
/// Setup class of winusb.inf ("USBDevice", Universal Serial Bus devices)
pub const USB_DEVICE_CLASS: GUID = GUID::from_u128(0x88BAE032_5A81_49F0_BC3D_A4FF138216D6);
/// Install section of winusb.inf's "WinUsb Device" model. Matched instead of
/// the model's description, which is translated on non-English Windows.
const WINUSB_SECTION: &str = "WINUSB";
/// How long a restore waits for Windows to install the adapter again
const RESTORE_WAIT: Duration = Duration::from_secs(30);

/// Which kind of driver owns a device
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverKind {
    /// WINUSB: BlueCaster can use the adapter
    WinUsb,
    /// BTHUSB or another driver of the Bluetooth class: Windows uses it
    Bluetooth,
    /// Some other driver (a vendor driver, usbccgp for a composite device)
    Other,
    /// No driver installed
    None,
}

impl DriverKind {
    /// From the device's service name and setup class name
    pub fn classify(service: &str, class_name: &str) -> DriverKind {
        if service.eq_ignore_ascii_case("WinUSB") {
            DriverKind::WinUsb
        } else if service.eq_ignore_ascii_case("BTHUSB") || class_name.eq_ignore_ascii_case("Bluetooth") {
            DriverKind::Bluetooth
        } else if service.is_empty() {
            DriverKind::None
        } else {
            DriverKind::Other
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            DriverKind::WinUsb => "winusb",
            DriverKind::Bluetooth => "bluetooth",
            DriverKind::Other => "other",
            DriverKind::None => "none",
        }
    }
}

/// The driver installed on a device (empty strings where Windows has none)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DriverInfo {
    /// Service name: BTHUSB, WINUSB, a vendor's
    pub service: String,
    /// INF name in %SystemRoot%\INF: `bth.inf`, `winusb.inf`, `oem12.inf`
    pub inf_name: String,
    pub inf_section: String,
    /// `10.0.26100.1`
    pub version: String,
    pub provider: String,
    pub description: String,
    /// Setup class name: Bluetooth, USBDevice, USB
    pub class_name: String,
    /// Setup class GUID, `{...}`
    pub class_guid: String,
}

/// What `driver_state` reports
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverState {
    pub instance_id: String,
    /// Plugged in (a removed adapter keeps its settings in Windows)
    pub present: bool,
    pub kind: DriverKind,
    pub driver: DriverInfo,
    /// Device Manager's problem code, None if it works
    pub problem_code: Option<u32>,
    /// `DeviceInterfaceGUIDs` in its Device Parameters
    pub interface_guids: Vec<String>,
}

impl DriverState {
    /// On WinUSB with BlueCaster's interface GUID
    pub fn has_bluecaster_interface(&self) -> bool {
        self.interface_guids.iter().any(|g| g.eq_ignore_ascii_case(INTERFACE_GUID))
    }
}

/// The driver a device had before WinUSB was assigned; the caller keeps it
/// (the helper prints it as JSON) and passes it back to `restore`
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DriverRecord {
    pub inf_name: String,
    pub version: String,
    pub service: String,
    pub provider: String,
    pub description: String,
    pub class_guid: String,
}

impl DriverRecord {
    pub fn from_info(info: &DriverInfo) -> DriverRecord {
        DriverRecord {
            inf_name: info.inf_name.clone(),
            version: info.version.clone(),
            service: info.service.clone(),
            provider: info.provider.clone(),
            description: info.description.clone(),
            class_guid: info.class_guid.clone(),
        }
    }

    /// The same driver package is installed again: INF name, version and
    /// service (oemNN.inf names stay the same for one package in the store)
    pub fn matches(&self, info: &DriverInfo) -> bool {
        self.inf_name.eq_ignore_ascii_case(&info.inf_name)
            && self.version == info.version
            && self.service.eq_ignore_ascii_case(&info.service)
    }
}

/// Whether a failed assignment was undone
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rollback {
    /// Nothing had changed but BlueCaster's interface GUID, which was removed
    NotNeeded,
    /// The device was put back as by `restore`
    Restored,
    /// Putting it back failed too: the user must reinstall the driver in
    /// Device Manager (or unplug and replug the adapter)
    Failed,
}

impl Rollback {
    fn note(self) -> &'static str {
        match self {
            Rollback::NotNeeded => "",
            Rollback::Restored => " (the adapter was put back on its previous driver)",
            Rollback::Failed => " (putting the adapter back failed as well)",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("administrator rights are needed to change a device's driver")]
    NotElevated,
    #[error("no device with the instance ID {0}")]
    DeviceNotFound(String),
    #[error("{0} is not a USB device")]
    NotUsb(String),
    #[error("Windows' WinUSB driver is missing ({0})")]
    WinUsbInfMissing(String),
    #[error("winusb.inf offered no \"WinUsb Device\" driver for this device (class {class}){}", .rollback.note())]
    WinUsbModelNotFound { class: String, rollback: Rollback },
    #[error("{step} failed: {source}{}", .rollback.note())]
    Win32 {
        step: &'static str,
        #[source]
        source: windows::core::Error,
        rollback: Rollback,
    },
}

impl DriverError {
    fn win32(step: &'static str) -> impl FnOnce(windows::core::Error) -> DriverError {
        move |source| DriverError::Win32 { step, source, rollback: Rollback::NotNeeded }
    }
}

/// What `assign_winusb` did
#[derive(Debug, Clone)]
pub struct AssignOutcome {
    /// The driver before; pass it to `restore` later
    pub previous: DriverRecord,
    /// It was on WinUSB already; nothing was changed
    pub already_winusb: bool,
    /// The device's setup class had to be changed to USBDevice first
    pub class_changed: bool,
    pub reboot_required: bool,
    /// The driver now
    pub state: DriverState,
}

/// What `restore` did
#[derive(Debug, Clone)]
pub struct RestoreOutcome {
    /// It was not on WinUSB; nothing was changed
    pub already_restored: bool,
    pub reboot_required: bool,
    /// The driver now; None if the device did not come back in time
    pub state: Option<DriverState>,
    /// The driver now is the recorded one (None without a record or state)
    pub matches_previous: Option<bool>,
}

/// The driver a device has now. Read-only; no administrator rights needed.
pub fn driver_state(instance_id: &str) -> Result<DriverState, DriverError> {
    let (set, data) = open(instance_id)?;
    let driver = set.driver_info(&data);
    let present = locate(instance_id).is_some();
    let state = DriverState {
        instance_id: instance_id.to_string(),
        present,
        kind: DriverKind::classify(&driver.service, &driver.class_name),
        problem_code: if present { problem_code(data.DevInst) } else { None },
        interface_guids: read_interface_guids(&set, &data),
        driver,
    };
    tracing::debug!(
        "driver: state of {instance_id}: present {}, {:?}, service {:?}, {} {} ({}), class {} {}, problem {:?}, interface GUIDs {:?}",
        state.present,
        state.kind,
        state.driver.service,
        state.driver.inf_name,
        state.driver.version,
        state.driver.provider,
        state.driver.class_name,
        state.driver.class_guid,
        state.problem_code,
        state.interface_guids
    );
    Ok(state)
}

/// Puts the device on WinUSB (see the module documentation). Needs
/// administrator rights; changes the driver of a real device, so it is
/// never called from tests.
pub fn assign_winusb(instance_id: &str) -> Result<AssignOutcome, DriverError> {
    if !is_elevated() {
        return Err(DriverError::NotElevated);
    }
    require_usb(instance_id)?;
    let (set, mut data) = open(instance_id)?;
    // Windows remembers unplugged devices; installing on one would only
    // take effect when it is plugged in, and could not be checked
    if locate(instance_id).is_none() {
        return Err(DriverError::DeviceNotFound(instance_id.to_string()));
    }
    let before = set.driver_info(&data);
    let previous = DriverRecord::from_info(&before);
    if DriverKind::classify(&before.service, &before.class_name) == DriverKind::WinUsb {
        tracing::info!("driver: {instance_id} is on WinUSB already");
        return Ok(AssignOutcome {
            previous,
            already_winusb: true,
            class_changed: false,
            reboot_required: false,
            state: driver_state(instance_id)?,
        });
    }
    let inf = winusb_inf_path()?;
    tracing::info!(
        "driver: assigning WinUSB to {instance_id} (now {} {} {})",
        before.service,
        before.inf_name,
        before.version
    );
    tracing::debug!(
        "driver: previous driver {:?} ({}), section {}, class {} {}; INF {}",
        before.description,
        before.provider,
        before.inf_section,
        before.class_name,
        before.class_guid,
        crate::autostart::utf16_string(&inf)
    );

    // Step 2: the interface GUID, kept beside any GUIDs already there
    let guids_before = write_interface_guid(&set, &data).map_err(DriverError::win32("writing DeviceInterfaceGUIDs"))?;

    // Steps 3 and 4
    let mut class_changed = false;
    let result = install(&set, &mut data, &inf, &mut class_changed);
    tracing::debug!(
        "driver: install on {instance_id}: {}, class changed {class_changed}",
        match &result {
            Ok(reboot) => format!("ok (reboot flag {reboot})"),
            Err(Failure::ModelNotFound(class)) => format!("no WINUSB model for class {class}"),
            Err(Failure::Win32(step, e)) =>
                format!("{step} failed: {e} (error {})", crate::usb_adapters::win32_code(e)),
        }
    );
    match result {
        Ok(mut reboot_required) => {
            if reboot_required {
                // Windows could not stop the device to load WinUSB (issue
                // #81: the core then fails with "failed to initialize WinUSB
                // (error 50)" until the adapter is replugged or Windows
                // restarts). Restart it as Device Manager does; only what
                // still needs a restart after that is reported
                reboot_required = restart_device(&set, &mut data);
            }
            let state = driver_state(instance_id)?;
            tracing::info!(
                "driver: {instance_id} is now on {} ({:?}){}",
                state.driver.service,
                state.kind,
                if reboot_required { ", restart needed" } else { "" }
            );
            Ok(AssignOutcome { previous, already_winusb: false, class_changed, reboot_required, state })
        }
        Err(failure) => {
            drop(set); // release the device before putting it back
            let rollback = undo_assign(instance_id, &previous, class_changed, guids_before);
            tracing::debug!("driver: rollback of {instance_id}: {rollback:?}");
            Err(match failure {
                Failure::ModelNotFound(class) => DriverError::WinUsbModelNotFound { class, rollback },
                Failure::Win32(step, source) => DriverError::Win32 { step, source, rollback },
            })
        }
    }
}

/// Puts a WinUSB device back on the driver Windows picks for it (see the
/// module documentation); `previous` from `assign_winusb` is compared with
/// the result. Needs administrator rights; never called from tests.
///
/// An adapter Zadig put on WinUSB is different: Zadig's generated INF stays
/// in the driver store and matches the adapter's hardware ID, which ranks
/// above Windows' Bluetooth driver (a compatible-ID match), so Windows picks
/// WinUSB again. The outcome then shows kind WinUsb; removing that package
/// (`pnputil /delete-driver oemNN.inf`) is not done here.
pub fn restore(instance_id: &str, previous: Option<&DriverRecord>) -> Result<RestoreOutcome, DriverError> {
    if !is_elevated() {
        return Err(DriverError::NotElevated);
    }
    require_usb(instance_id)?;
    let (set, data) = open(instance_id)?;
    let now = set.driver_info(&data);
    if DriverKind::classify(&now.service, &now.class_name) != DriverKind::WinUsb {
        // Not ours to undo (also when the user already fixed it by hand)
        tracing::info!("driver: {instance_id} is not on WinUSB ({}); nothing to restore", now.service);
        return Ok(RestoreOutcome {
            already_restored: true,
            reboot_required: false,
            matches_previous: previous.map(|p| p.matches(&now)),
            state: Some(driver_state(instance_id)?),
        });
    }
    tracing::info!("driver: restoring {instance_id} from WinUSB");
    tracing::debug!(
        "driver: now {} {} {} (class {}), record {:?}",
        now.service,
        now.inf_name,
        now.version,
        now.class_guid,
        previous
    );
    let reboot_required = uninstall_and_reenumerate(&set, &data, instance_id)?;
    drop(set);
    let state = wait_for_driver(instance_id);
    let matches_previous = match (previous, &state) {
        (Some(p), Some(s)) => Some(p.matches(&s.driver)),
        _ => None,
    };
    if let Some(s) = &state {
        tracing::info!(
            "driver: {instance_id} is now on {} {} {} (matches the record: {matches_previous:?})",
            s.driver.service,
            s.driver.inf_name,
            s.driver.version
        );
    } else {
        tracing::warn!("driver: {instance_id} did not come back within {RESTORE_WAIT:?}");
    }
    Ok(RestoreOutcome { already_restored: false, reboot_required, state, matches_previous })
}

/// A REG_MULTI_SZ value: each string NUL-terminated, then one more NUL
pub fn multi_sz<S: AsRef<str>>(strings: &[S]) -> Vec<u16> {
    let mut out = Vec::new();
    for s in strings {
        out.extend(s.as_ref().encode_utf16());
        out.push(0);
    }
    out.push(0);
    out
}

/// `a.b.c.d` from a driver version packed in 64 bits (SP_DRVINFO_DATA)
pub fn format_driver_version(v: u64) -> String {
    format!("{}.{}.{}.{}", v >> 48, (v >> 32) & 0xFFFF, (v >> 16) & 0xFFFF, v & 0xFFFF)
}

enum Failure {
    ModelNotFound(String),
    Win32(&'static str, windows::core::Error),
}

fn step(name: &'static str) -> impl FnOnce(windows::core::Error) -> Failure {
    move |e| Failure::Win32(name, e)
}

/// Steps 3 and 4; returns whether Windows asks for a restart
fn install(
    set: &DevInfoSet,
    data: &mut SP_DEVINFO_DATA,
    inf: &[u16; 260],
    class_changed: &mut bool,
) -> Result<bool, Failure> {
    let mut found = find_winusb_driver(set, data, inf)?;
    if found.is_none() && data.ClassGuid != USB_DEVICE_CLASS {
        // The class driver list may hold only drivers of the device's own
        // class. Move the device to winusb.inf's class first, as
        // installing it would; undone by the rollback if this fails.
        tracing::info!("driver: winusb.inf not offered for class {}; changing the class", format_guid(&data.ClassGuid));
        // SAFETY: valid set and element; the list is rebuilt below.
        unsafe { SetupDiDestroyDriverInfoList(set.0, Some(data), SPDIT_CLASSDRIVER) }
            .map_err(step("SetupDiDestroyDriverInfoList"))?;
        let class: Vec<u16> = multi_sz(&[format_guid(&USB_DEVICE_CLASS)]);
        let bytes = u16_bytes(&class[..class.len() - 1]); // one string, one terminator
        // SAFETY: valid set and element; REG_SZ data with its terminator.
        unsafe { SetupDiSetDeviceRegistryPropertyW(set.0, data, SPDRP_CLASSGUID, Some(&bytes)) }
            .map_err(step("changing the device class"))?;
        *class_changed = true;
        tracing::debug!(
            "driver: class GUID set to {} (element class now {})",
            format_guid(&USB_DEVICE_CLASS),
            format_guid(&data.ClassGuid)
        );
        found = find_winusb_driver(set, data, inf)?;
    }
    let Some(mut driver) = found else {
        return Err(Failure::ModelNotFound(format_guid(&data.ClassGuid)));
    };
    tracing::info!(
        "driver: selected \"{}\" {} from winusb.inf",
        crate::autostart::utf16_string(&driver.Description),
        format_driver_version(driver.DriverVersion)
    );
    // SAFETY: the driver came from this element's class driver list.
    unsafe { SetupDiSetSelectedDriverW(set.0, Some(data), Some(&mut driver)) }
        .map_err(step("SetupDiSetSelectedDriver"))?;
    tracing::debug!("driver: driver selected; calling DiInstallDevice");
    let started = Instant::now();
    let mut reboot = BOOL(0);
    // SAFETY: valid set, element and its selected driver; no window (no
    // UI is shown with DIIDFLAG_NOFINISHINSTALLUI).
    unsafe { DiInstallDevice(None, set.0, data, Some(&driver), DIIDFLAG_NOFINISHINSTALLUI, Some(&mut reboot)) }
        .map_err(step("DiInstallDevice"))?;
    tracing::debug!("driver: DiInstallDevice done in {:?}, reboot {}", started.elapsed(), reboot.as_bool());
    Ok(reboot.as_bool())
}

/// Restarts the device (DIF_PROPERTYCHANGE / DICS_PROPCHANGE, as Device
/// Manager and devcon restart do) so that the driver just installed loads.
/// Returns whether Windows still needs a restart for it.
fn restart_device(set: &DevInfoSet, data: &mut SP_DEVINFO_DATA) -> bool {
    tracing::debug!("driver: Windows asks for a restart; restarting the device (DIF_PROPERTYCHANGE)");
    let params = SP_PROPCHANGE_PARAMS {
        ClassInstallHeader: SP_CLASSINSTALL_HEADER {
            cbSize: size_of::<SP_CLASSINSTALL_HEADER>() as u32,
            InstallFunction: DIF_PROPERTYCHANGE,
        },
        StateChange: DICS_PROPCHANGE,
        Scope: DICS_FLAG_GLOBAL,
        HwProfile: 0,
    };
    // SAFETY: valid set and element; the header starts the parameters,
    // which are passed with their full size.
    let restarted = unsafe {
        SetupDiSetClassInstallParamsW(
            set.0,
            Some(data),
            Some(&params.ClassInstallHeader),
            size_of::<SP_PROPCHANGE_PARAMS>() as u32,
        )
    }
    .and_then(|()| {
        // SAFETY: valid set and element with its class install parameters.
        unsafe { SetupDiCallClassInstaller(DIF_PROPERTYCHANGE, set.0, Some(data)) }
    });
    if let Err(e) = restarted {
        tracing::warn!("driver: restarting the device failed: {e}");
        return true;
    }
    let mut install =
        SP_DEVINSTALL_PARAMS_W { cbSize: size_of::<SP_DEVINSTALL_PARAMS_W>() as u32, ..Default::default() };
    // SAFETY: valid set and element; out structure with its size set.
    if let Err(e) = unsafe { SetupDiGetDeviceInstallParamsW(set.0, Some(data), &mut install) } {
        tracing::warn!("driver: reading the install parameters failed: {e}");
        return true;
    }
    let still = (install.Flags & (DI_NEEDRESTART | DI_NEEDREBOOT)).0 != 0;
    tracing::debug!("driver: install flags after the restart 0x{:08X}", install.Flags.0);
    tracing::info!("driver: device restarted{}", if still { ", Windows still asks for a restart" } else { "" });
    still
}

/// Step 3: the class driver list from winusb.inf alone; its WINUSB model
fn find_winusb_driver(
    set: &DevInfoSet,
    data: &mut SP_DEVINFO_DATA,
    inf: &[u16; 260],
) -> Result<Option<SP_DRVINFO_DATA_V2_W>, Failure> {
    let mut params =
        SP_DEVINSTALL_PARAMS_W { cbSize: size_of::<SP_DEVINSTALL_PARAMS_W>() as u32, ..Default::default() };
    // SAFETY: valid set and element; out structure with its size set.
    unsafe { SetupDiGetDeviceInstallParamsW(set.0, Some(data), &mut params) }
        .map_err(step("SetupDiGetDeviceInstallParams"))?;
    params.Flags |= DI_ENUMSINGLEINF;
    // winusb.inf's models are not ExcludeFromSelect today; harmless if so
    params.FlagsEx |= DI_FLAGSEX_ALLOWEXCLUDEDDRVS;
    params.DriverPath = *inf;
    // SAFETY: as above, with the structure filled in.
    unsafe { SetupDiSetDeviceInstallParamsW(set.0, Some(data), &params) }
        .map_err(step("SetupDiSetDeviceInstallParams"))?;
    tracing::debug!(
        "driver: building the driver list from {} for class {} (flags 0x{:08X}, ex 0x{:08X})",
        crate::autostart::utf16_string(inf),
        format_guid(&data.ClassGuid),
        params.Flags.0,
        params.FlagsEx.0
    );
    // SAFETY: valid set and element.
    unsafe { SetupDiBuildDriverInfoList(set.0, Some(data), SPDIT_CLASSDRIVER) }
        .map_err(step("SetupDiBuildDriverInfoList"))?;
    for index in 0.. {
        let mut driver =
            SP_DRVINFO_DATA_V2_W { cbSize: size_of::<SP_DRVINFO_DATA_V2_W>() as u32, ..Default::default() };
        // SAFETY: valid set and element; out structure with its size set.
        if let Err(e) = unsafe { SetupDiEnumDriverInfoW(set.0, Some(data), SPDIT_CLASSDRIVER, index, &mut driver) } {
            if e.code() == ERROR_NO_MORE_ITEMS.to_hresult() {
                break;
            }
            return Err(Failure::Win32("SetupDiEnumDriverInfo", e));
        }
        let section = driver_section(set, data, &driver);
        tracing::debug!(
            "driver: winusb.inf offers \"{}\" (section {section:?})",
            crate::autostart::utf16_string(&driver.Description)
        );
        if section.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(WINUSB_SECTION)) {
            return Ok(Some(driver));
        }
    }
    tracing::debug!("driver: no {WINUSB_SECTION} section in the list for class {}", format_guid(&data.ClassGuid));
    Ok(None)
}

/// Install section name of a driver in the list
fn driver_section(set: &DevInfoSet, data: &SP_DEVINFO_DATA, driver: &SP_DRVINFO_DATA_V2_W) -> Option<String> {
    let mut size = 0u32;
    // SAFETY: size query without a buffer.
    let _ = unsafe { SetupDiGetDriverInfoDetailW(set.0, Some(data), driver, None, 0, Some(&mut size)) };
    let size = (size as usize).max(size_of::<SP_DRVINFO_DETAIL_DATA_W>());
    // u64 units keep the structure aligned
    let mut buf = vec![0u64; size.div_ceil(8)];
    let detail = buf.as_mut_ptr().cast::<SP_DRVINFO_DETAIL_DATA_W>();
    // SAFETY: the buffer holds `size` bytes, aligned; cbSize is the fixed
    // structure size, as the API requires.
    unsafe { (*detail).cbSize = size_of::<SP_DRVINFO_DETAIL_DATA_W>() as u32 };
    // SAFETY: buffer of `size` bytes.
    unsafe { SetupDiGetDriverInfoDetailW(set.0, Some(data), driver, Some(detail), size as u32, None) }.ok()?;
    // SAFETY: filled in by the call above.
    Some(crate::autostart::utf16_string(unsafe { &(*detail).SectionName }))
}

/// Adds INTERFACE_GUID to DeviceInterfaceGUIDs in the hardware key
/// (Device Parameters); returns the value as it was, None if absent
fn write_interface_guid(set: &DevInfoSet, data: &SP_DEVINFO_DATA) -> windows::core::Result<Option<Vec<u16>>> {
    // SAFETY: valid set and element; the hardware key of the device
    // (created if missing), no INF section to run.
    let key = RegKey(unsafe {
        SetupDiCreateDevRegKeyW(set.0, data, DICS_FLAG_GLOBAL.0, 0, DIREG_DEV, None, PCWSTR::null())
    }?);
    let before = key.query(INTERFACE_GUIDS_VALUE)?.map(|(_, d)| d);
    let mut guids = before.as_deref().map(parse_multi_sz).unwrap_or_default();
    if !guids.iter().any(|g| g.eq_ignore_ascii_case(INTERFACE_GUID)) {
        guids.push(INTERFACE_GUID.to_string());
        key.set(INTERFACE_GUIDS_VALUE, REG_MULTI_SZ, &multi_sz(&guids))?;
        tracing::debug!("driver: {INTERFACE_GUIDS_VALUE} written: {guids:?}");
    } else {
        tracing::debug!("driver: {INTERFACE_GUIDS_VALUE} has BlueCaster's GUID already: {guids:?}");
    }
    Ok(before)
}

fn read_interface_guids(set: &DevInfoSet, data: &SP_DEVINFO_DATA) -> Vec<String> {
    // SAFETY: valid set and element; read access only. Fails (nothing to
    // read) when the device has no hardware key.
    let Ok(key) = (unsafe { SetupDiOpenDevRegKey(set.0, data, DICS_FLAG_GLOBAL.0, 0, DIREG_DEV, KEY_READ.0) }) else {
        return Vec::new();
    };
    let key = RegKey(key);
    match key.query(INTERFACE_GUIDS_VALUE) {
        Ok(Some((ty, d))) if ty == REG_MULTI_SZ => parse_multi_sz(&d),
        Ok(Some((_, d))) => vec![crate::autostart::utf16_string(&d)], // REG_SZ written by hand
        _ => Vec::new(),
    }
}

/// After a failed assignment: if the driver is still the one from before
/// (nothing but the interface GUID changed), put the GUID value back as it
/// was; otherwise uninstall and re-enumerate as `restore` does
fn undo_assign(
    instance_id: &str,
    previous: &DriverRecord,
    class_changed: bool,
    guids_before: Option<Vec<u16>>,
) -> Rollback {
    let (set, data) = match open(instance_id) {
        Ok(o) => o,
        Err(e) => {
            tracing::debug!("driver: undo: cannot open {instance_id}: {e}");
            return Rollback::Failed;
        }
    };
    let now = set.driver_info(&data);
    tracing::debug!(
        "driver: undo: now {} {} {}, class changed {class_changed}, same driver as before {}",
        now.service,
        now.inf_name,
        now.version,
        previous.matches(&now)
    );
    if !class_changed && previous.matches(&now) {
        // SAFETY: valid set and element; the existing hardware key.
        let key =
            unsafe { SetupDiCreateDevRegKeyW(set.0, &data, DICS_FLAG_GLOBAL.0, 0, DIREG_DEV, None, PCWSTR::null()) };
        match key {
            Ok(key) => {
                let key = RegKey(key);
                let had = guids_before.is_some();
                let r = match guids_before {
                    Some(d) => key.set(INTERFACE_GUIDS_VALUE, REG_MULTI_SZ, &d).map(|_| true),
                    None => key.delete_value(INTERFACE_GUIDS_VALUE),
                };
                tracing::debug!(
                    "driver: undo: {INTERFACE_GUIDS_VALUE} {}: {r:?}",
                    if had { "put back" } else { "removed" }
                );
            }
            Err(e) => tracing::debug!("driver: undo: cannot open the device key: {e}"),
        }
        return Rollback::NotNeeded;
    }
    tracing::warn!("driver: assignment failed half way; putting {instance_id} back");
    match uninstall_and_reenumerate(&set, &data, instance_id) {
        Ok(_) => {
            drop(set);
            if wait_for_driver(instance_id).is_some() { Rollback::Restored } else { Rollback::Failed }
        }
        Err(e) => {
            tracing::error!("driver: putting {instance_id} back failed: {e}");
            Rollback::Failed
        }
    }
}

/// Removes the device and lets Windows find it again; returns whether a
/// restart is needed
fn uninstall_and_reenumerate(set: &DevInfoSet, data: &SP_DEVINFO_DATA, instance_id: &str) -> Result<bool, DriverError> {
    // The parent (the hub port) must be found before the device is gone
    let parent = locate(instance_id).and_then(|dev| {
        let mut parent = 0u32;
        // SAFETY: valid devinst and out pointer.
        (unsafe { CM_Get_Parent(&mut parent, dev, 0) } == CR_SUCCESS).then_some(parent)
    });
    tracing::debug!("driver: uninstalling {instance_id} (parent devnode {parent:?})");
    let mut reboot = BOOL(0);
    // SAFETY: valid set and element; no window, default flags (the driver
    // package stays in the driver store).
    unsafe { DiUninstallDevice(HWND::default(), set.0, data, 0, Some(&mut reboot)) }
        .map_err(DriverError::win32("DiUninstallDevice"))?;
    tracing::debug!("driver: DiUninstallDevice done, reboot {}", reboot.as_bool());
    let reenumerated = parent.is_some_and(reenumerate);
    tracing::debug!("driver: re-enumerating the parent: {reenumerated}");
    if !reenumerated {
        // No parent known (the device was not present) or it failed: the
        // whole tree, as "Scan for hardware changes" does
        let mut root = 0u32;
        // SAFETY: a NULL device ID locates the root of the device tree.
        let cr = unsafe { CM_Locate_DevNodeW(&mut root, PCWSTR::null(), CM_LOCATE_DEVNODE_NORMAL) };
        if cr != CR_SUCCESS || !reenumerate(root) {
            tracing::warn!("driver: re-enumerating devices failed; replugging the adapter brings it back");
        }
    }
    Ok(reboot.as_bool())
}

fn reenumerate(devinst: u32) -> bool {
    // SAFETY: a devinst from CM_Locate_DevNode / CM_Get_Parent.
    let cr = unsafe { CM_Reenumerate_DevNode(devinst, CM_REENUMERATE_SYNCHRONOUS) };
    if cr != CR_SUCCESS {
        tracing::debug!("driver: CM_Reenumerate_DevNode({devinst}) returned CR {}", cr.0);
    }
    cr == CR_SUCCESS
}

/// After a re-enumeration: waits until Windows has installed the device
/// again and returns its state, None if it did not come back in time
fn wait_for_driver(instance_id: &str) -> Option<DriverState> {
    let start = Instant::now();
    // SAFETY: plain wait with a timeout in milliseconds.
    let waited = unsafe { CMP_WaitNoPendingInstallEvents(RESTORE_WAIT.as_millis() as u32) };
    if waited != WAIT_OBJECT_0.0 {
        tracing::debug!("driver: CMP_WaitNoPendingInstallEvents returned {waited}");
    }
    loop {
        // A device back without a driver (problem 28) may only be waiting
        // for its install to start, so that is accepted only at the end
        if let Ok(state) = driver_state(instance_id)
            && state.present
            && state.kind != DriverKind::None
        {
            tracing::debug!("driver: {instance_id} back after {:?}", start.elapsed());
            return Some(state);
        }
        if start.elapsed() >= RESTORE_WAIT {
            let last = driver_state(instance_id);
            tracing::debug!("driver: waited {:?} for {instance_id}; last state {last:?}", start.elapsed());
            return last.ok().filter(|s| s.present);
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// The devinst of a present device
fn locate(instance_id: &str) -> Option<u32> {
    let mut devinst = 0u32;
    // SAFETY: NUL-terminated ID and out pointer.
    let cr = unsafe { CM_Locate_DevNodeW(&mut devinst, &HSTRING::from(instance_id), CM_LOCATE_DEVNODE_NORMAL) };
    (cr == CR_SUCCESS).then_some(devinst)
}

fn open(instance_id: &str) -> Result<(DevInfoSet, SP_DEVINFO_DATA), DriverError> {
    DevInfoSet::open(instance_id).map_err(|e| {
        tracing::debug!(
            "driver: SetupDiOpenDeviceInfo({instance_id}): {e} (error {})",
            crate::usb_adapters::win32_code(&e)
        );
        if e.code() == ERROR_NO_SUCH_DEVINST.to_hresult() || e.code() == ERROR_INVALID_DEVINST_NAME.to_hresult() {
            DriverError::DeviceNotFound(instance_id.to_string())
        } else {
            DriverError::Win32 { step: "SetupDiOpenDeviceInfo", source: e, rollback: Rollback::NotNeeded }
        }
    })
}

/// Only USB devices: a guard against an instance ID of anything else
/// (a PCI controller, a system device) reaching the install code
fn require_usb(instance_id: &str) -> Result<(), DriverError> {
    if instance_id.len() > 4 && instance_id[..4].eq_ignore_ascii_case("USB\\") {
        Ok(())
    } else {
        Err(DriverError::NotUsb(instance_id.to_string()))
    }
}

/// `%SystemRoot%\INF\winusb.inf` as SP_DEVINSTALL_PARAMS.DriverPath wants it
fn winusb_inf_path() -> Result<[u16; 260], DriverError> {
    let mut dir = [0u16; 260];
    // SAFETY: the buffer length is passed.
    let n = unsafe { GetWindowsDirectoryW(Some(&mut dir)) } as usize;
    if n == 0 || n >= dir.len() {
        return Err(DriverError::WinUsbInfMissing("%SystemRoot% not found".into()));
    }
    let path = format!(r"{}\INF\winusb.inf", String::from_utf16_lossy(&dir[..n]));
    if !std::path::Path::new(&path).is_file() {
        return Err(DriverError::WinUsbInfMissing(path));
    }
    let mut out = [0u16; 260];
    let units: Vec<u16> = path.encode_utf16().collect();
    if units.len() >= out.len() {
        return Err(DriverError::WinUsbInfMissing(path));
    }
    out[..units.len()].copy_from_slice(&units);
    Ok(out)
}

fn u16_bytes(data: &[u16]) -> Vec<u8> {
    data.iter().flat_map(|u| u.to_le_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_sz_layout() {
        let v = multi_sz(&[INTERFACE_GUID]);
        assert_eq!(v.len(), INTERFACE_GUID.len() + 2);
        assert_eq!(&v[v.len() - 2..], &[0, 0]);
        assert_eq!(parse_multi_sz(&v), [INTERFACE_GUID]);
        let two = multi_sz(&["{A}", "{B}"]);
        assert_eq!(String::from_utf16(&two).unwrap(), "{A}\0{B}\0\0");
        assert_eq!(multi_sz::<&str>(&[]), [0]);
    }

    #[test]
    fn interface_guid_is_well_formed() {
        let inner = INTERFACE_GUID.trim_start_matches('{').trim_end_matches('}');
        let g = GUID::try_from(inner).expect("valid GUID");
        assert_eq!(format_guid(&g), INTERFACE_GUID);
    }

    #[test]
    fn driver_version_unpacks() {
        let v = (10u64 << 48) | (26100u64 << 16) | 9444; // 10.0.26100.9444
        assert_eq!(format_driver_version(v), "10.0.26100.9444");
    }

    #[test]
    fn classify_drivers() {
        assert_eq!(DriverKind::classify("WINUSB", "USBDevice"), DriverKind::WinUsb);
        assert_eq!(DriverKind::classify("WinUSB", "Bluetooth"), DriverKind::WinUsb);
        assert_eq!(DriverKind::classify("BTHUSB", "Bluetooth"), DriverKind::Bluetooth);
        assert_eq!(DriverKind::classify("ibtusb", "Bluetooth"), DriverKind::Bluetooth);
        assert_eq!(DriverKind::classify("usbccgp", "USB"), DriverKind::Other);
        assert_eq!(DriverKind::classify("", ""), DriverKind::None);
    }

    #[test]
    fn record_matching() {
        let info = DriverInfo {
            service: "BTHUSB".into(),
            inf_name: "bth.inf".into(),
            version: "10.0.26100.1".into(),
            class_name: "Bluetooth".into(),
            ..Default::default()
        };
        let rec = DriverRecord::from_info(&info);
        assert!(rec.matches(&info));
        assert!(rec.matches(&DriverInfo { inf_name: "BTH.INF".into(), service: "bthusb".into(), ..info.clone() }));
        assert!(!rec.matches(&DriverInfo { version: "10.0.26100.2".into(), ..info.clone() }));
        assert!(!rec.matches(&DriverInfo { service: "WINUSB".into(), inf_name: "winusb.inf".into(), ..info }));
    }

    #[test]
    fn only_usb_instance_ids_pass_the_guard() {
        assert!(require_usb(r"USB\VID_0BDA&PID_8771\00E04C239987").is_ok());
        assert!(require_usb(r"usb\vid_0bda&pid_8771\1").is_ok());
        assert!(matches!(require_usb(r"PCI\VEN_8086&DEV_A0F0\3"), Err(DriverError::NotUsb(_))));
        assert!(matches!(require_usb("USB\\"), Err(DriverError::NotUsb(_))));
    }

    #[test]
    fn winusb_inf_is_found() {
        let p = winusb_inf_path().expect("winusb.inf ships with Windows 8 and later");
        let s = crate::autostart::utf16_string(&p);
        assert!(s.to_ascii_lowercase().ends_with(r"\inf\winusb.inf"), "{s}");
    }

    #[test]
    fn unknown_device_is_not_found() {
        assert!(matches!(driver_state(r"USB\VID_FFFF&PID_FFFF\BLUECASTER_TEST"), Err(DriverError::DeviceNotFound(_))));
    }

    /// Read-only: the driver state of every present USB device that looks
    /// like Bluetooth
    #[test]
    fn driver_state_of_present_devices() {
        for d in crate::usb_adapters::enumerate(false).unwrap().iter().filter(|d| d.looks_like_bluetooth()) {
            let s = driver_state(&d.instance_id).unwrap();
            assert!(s.present);
            assert_eq!(s.driver, d.driver);
            eprintln!(
                "{} -> {:?} {:?} guids={:?} bluecaster={}",
                s.instance_id,
                s.kind,
                s.driver,
                s.interface_guids,
                s.has_bluecaster_interface()
            );
        }
    }
}
