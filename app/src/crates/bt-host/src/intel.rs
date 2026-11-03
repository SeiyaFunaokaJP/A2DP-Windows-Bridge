//! Intel controller firmware download: the legacy and the TLV bootloader
//!
//! Intel USB controllers keep their operational firmware in RAM. After a
//! cold boot they start in a bootloader that takes only a few vendor
//! commands: the host sends the firmware (ibt-*.sfi from linux-firmware
//! intel/), boots it and applies the device configuration (ibt-*.ddc).
//! Windows' Intel driver does that, but not once the adapter is on WinUSB.
//! Firmware loaded earlier (by that driver, or in an earlier session) runs
//! until the controller loses power; then nothing is sent.
//!
//! Two generations of the bootloader:
//! - legacy (8260, 8265, 9260, 9560, AX200, AX201): the answer of Read
//!   Version has a fixed layout; ibt-<variant>-<revid>.sfi or
//!   ibt-<variant>-<hw rev>-<fw rev>.sfi
//! - TLV (AX210, AX211, AX411, BE200 and later): Read Version answers with
//!   type-length-value fields; ibt-<CNVi>-<CNVr>.sfi from the CNVi / CNVr
//!   top registers. BlazarI and later (variant 0x1E+) boot an intermediate
//!   loader first (ibt-<CNVi>-<CNVr>-iml.sfi), then the operational image
//!
//! The steps, as Linux's btintel driver takes them:
//! 1. Read Version (0xFC05, parameter 0xFF): legacy or TLV answer; the
//!    image type tells the bootloader from the operational firmware
//! 2. legacy: Read Secure Boot Params (0xFC0D)
//! 3. Secure Send (0xFC09) fragments of up to 252 bytes: the CSS header,
//!    public key and signature for the secure boot engine (RSA, ECDSA or
//!    hybrid RSA / LMS), then the command buffer in 4-byte aligned pieces.
//!    Every fragment is acknowledged with a Command Complete
//! 4. vendor event 0x06: the bootloader checked the image
//! 5. Intel Reset (0xFC01) with the boot address of the image's Write Boot
//!    Params (0xFC0E); vendor event 0x02: the firmware booted. An
//!    intermediate loader: Read Version, and 3 to 5 once more
//! 6. Write DDC (0xFC8B) per record of the .ddc, Intel event mask (0xFC52)
//!
//! In the bootloader, Secure Send goes out on the bulk OUT pipe and events
//! may come back on the bulk IN pipe, and HCI Reset must not be sent
//! ([`Action::Bootloader`]): from the first command on, as the image is not
//! known before the answer, until the operational firmware runs. A failed
//! download leaves it on: the controller is still in its bootloader.
//!
//! Not done (Linux takes them from ACPI / UEFI, or they are for debugging):
//! the unlocker image of API-locked controllers, SAR / PPAG / DSBR tables,
//! offload use cases, and a reset to the bootloader to replace a running
//! firmware of another version.
//!
//! Plain data and std only: [`Loader`] is a [`ChipLoader`] (crate::bringup)
//! fed with HCI events; the caller carries out its [`Action`]s.

use std::time::Instant;

pub use crate::bringup::{Action, ChipLoader, FileReader, LoadError, Outcome};

const OP_RESET: u16 = 0xFC01;
const OP_READ_VERSION: u16 = 0xFC05;
const OP_SECURE_SEND: u16 = 0xFC09;
const OP_READ_BOOT_PARAMS: u16 = 0xFC0D;
const OP_WRITE_BOOT_PARAMS: u16 = 0xFC0E;
const OP_SET_EVENT_MASK: u16 = 0xFC52;
const OP_WRITE_DDC: u16 = 0xFC8B;

const EVENT_COMMAND_COMPLETE: u8 = 0x0E;
const EVENT_COMMAND_STATUS: u8 = 0x0F;
const EVENT_VENDOR: u8 = 0xFF;
/// Vendor event: the firmware booted (6 bytes)
const VENDOR_BOOTUP: u8 = 0x02;
/// Vendor event: result of the Secure Send transaction (4 bytes)
const VENDOR_SECURE_SEND_RESULT: u8 = 0x06;

/// Intel's hardware platform, in both version formats
const HW_PLATFORM: u8 = 0x37;

/// TLV image types
pub const IMG_BOOTLOADER: u8 = 0x01;
pub const IMG_IML: u8 = 0x02;
pub const IMG_OPERATIONAL: u8 = 0x03;

/// Legacy firmware variants
const FW_VARIANT_BOOTLOADER: u8 = 0x06;
const FW_VARIANT_OPERATIONAL: u8 = 0x23;

/// CSS header version (bytes 8..12 of a header)
const CSS_VERSION_OFFSET: usize = 8;
const CSS_RSA: u32 = 0x0001_0000;
const CSS_ECDSA: u32 = 0x0002_0000;
const CSS_HYBRID: u32 = 0x0006_9700;
/// The ECDSA header follows the RSA header; the command buffer follows it
const ECDSA_HEADER_OFFSET: usize = 644;
/// Where the command buffer starts behind each header layout
const RSA_PAYLOAD: usize = 644;
const RSA_ECDSA_PAYLOAD: usize = 964;
const HYBRID_PAYLOAD: usize = 2116;
/// Data per Secure Send command (its parameter: type + 252 bytes)
const SECURE_SEND_MAX: usize = 252;

/// Secure Send fragment types
const FRAGMENT_INIT: u8 = 0x00;
const FRAGMENT_DATA: u8 = 0x01;
const FRAGMENT_SIGNATURE: u8 = 0x02;
const FRAGMENT_PUBLIC_KEY: u8 = 0x03;
const FRAGMENT_LMS_SIGNATURE: u8 = 0x04;
const FRAGMENT_LMS_PUBLIC_KEY: u8 = 0x05;

/// Header sections: fragment type, offset, length
const RSA_SECTIONS: [(u8, usize, usize); 3] =
    [(FRAGMENT_INIT, 0, 128), (FRAGMENT_PUBLIC_KEY, 128, 256), (FRAGMENT_SIGNATURE, 388, 256)];
const ECDSA_SECTIONS: [(u8, usize, usize); 3] =
    [(FRAGMENT_INIT, 644, 128), (FRAGMENT_PUBLIC_KEY, 772, 96), (FRAGMENT_SIGNATURE, 868, 96)];
const HYBRID_SECTIONS: [(u8, usize, usize); 5] = [
    (FRAGMENT_INIT, 0, 128),
    (FRAGMENT_PUBLIC_KEY, 128, 96),
    (FRAGMENT_SIGNATURE, 224, 96),
    (FRAGMENT_LMS_PUBLIC_KEY, 320, 52),
    (FRAGMENT_LMS_SIGNATURE, 372, 1744),
];

/// Answer to a command
pub const COMMAND_TIMEOUT_MS: u32 = 5000;
/// The first Read Version: sent again after this, up to VERSION_TRIES times.
/// A bootloader still holding the answer to a command of an earlier session
/// (the HCI Reset of the last close) sends that and drops the new command.
pub const VERSION_RETRY_MS: u32 = 1000;
const VERSION_TRIES: u8 = 3;
/// The bootloader's check of the whole image, after the last fragment
pub const CHECK_TIMEOUT_MS: u32 = 5000;
/// Intel Reset to the bootup event
pub const BOOT_TIMEOUT_MS: u32 = 5000;

/// Intel vendor events enabled after a download (Linux: no debug events)
const EVENT_MASK: [u8; 8] = [0x87, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

/// Downloads per start: the intermediate loader and the operational image
const MAX_DOWNLOADS: u8 = 2;

// ==========================================================================
// Read Version
// ==========================================================================

/// The legacy answer to Read Version
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LegacyVersion {
    pub hw_platform: u8,
    pub hw_variant: u8,
    pub hw_revision: u8,
    /// 0x06 bootloader, 0x23 operational firmware
    pub fw_variant: u8,
    pub fw_revision: u8,
    pub fw_build_num: u8,
    pub fw_build_ww: u8,
    pub fw_build_yy: u8,
    pub fw_patch_num: u8,
}

/// The TLV answer to Read Version: the fields the download uses or logs
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TlvVersion {
    pub cnvi_top: u32,
    pub cnvr_top: u32,
    pub cnvi_bt: u32,
    pub cnvr_bt: u32,
    pub dev_rev_id: u16,
    /// [`IMG_BOOTLOADER`], [`IMG_IML`] or [`IMG_OPERATIONAL`]
    pub img_type: u8,
    pub timestamp: u16,
    pub build_type: u8,
    pub build_num: u32,
    pub secure_boot: u8,
    pub otp_lock: u8,
    pub api_lock: u8,
    pub debug_lock: u8,
    /// Oldest firmware the bootloader takes: build number, week, year - 2000
    pub min_fw: (u8, u8, u8),
    pub limited_cce: u8,
    /// Secure boot engine: 0 RSA, 1 ECDSA, 2 LMS, 3 ECDSA + LMS
    pub sbe_type: u8,
    pub unlock_policy: u8,
    /// Firmware ID of BlazarI and later (part of the file name)
    pub fw_id: String,
}

/// TLV types of the Read Version answer
const TLV_CNVI_TOP: u8 = 0x10;
const TLV_CNVR_TOP: u8 = 0x11;
const TLV_CNVI_BT: u8 = 0x12;
const TLV_CNVR_BT: u8 = 0x13;
const TLV_DEV_REV_ID: u8 = 0x16;
const TLV_IMAGE_TYPE: u8 = 0x1C;
const TLV_TIME_STAMP: u8 = 0x1D;
const TLV_BUILD_TYPE: u8 = 0x1E;
const TLV_BUILD_NUM: u8 = 0x1F;
const TLV_SECURE_BOOT: u8 = 0x28;
const TLV_OTP_LOCK: u8 = 0x2A;
const TLV_API_LOCK: u8 = 0x2B;
const TLV_DEBUG_LOCK: u8 = 0x2C;
const TLV_MIN_FW: u8 = 0x2D;
const TLV_LIMITED_CCE: u8 = 0x2E;
const TLV_SBE_TYPE: u8 = 0x2F;
const TLV_UNLOCK_POLICY: u8 = 0x3C;
const TLV_FW_ID: u8 = 0x50;

/// The shortest value a TLV type that is read may have (a shorter one
/// spoils the answer); the others are skipped whatever their length
fn tlv_min_len(t: u8) -> usize {
    match t {
        TLV_CNVI_TOP | TLV_CNVR_TOP | TLV_CNVI_BT | TLV_CNVR_BT | TLV_BUILD_NUM => 4,
        TLV_DEV_REV_ID | TLV_TIME_STAMP => 2,
        TLV_IMAGE_TYPE | TLV_BUILD_TYPE | TLV_SECURE_BOOT | TLV_OTP_LOCK | TLV_API_LOCK | TLV_DEBUG_LOCK
        | TLV_LIMITED_CCE | TLV_SBE_TYPE | TLV_UNLOCK_POLICY => 1,
        TLV_MIN_FW => 3,
        _ => 0,
    }
}

impl TlvVersion {
    /// The controller generation, e.g. 0x17 (AX210), 0x18 / 0x1C (AX211)
    pub fn hw_variant(&self) -> u8 {
        ((self.cnvi_bt >> 16) & 0x3F) as u8
    }

    pub fn hw_platform(&self) -> u8 {
        ((self.cnvi_bt >> 8) & 0xFF) as u8
    }

    /// The firmware file name without extension: "ibt-0040-0041",
    /// "ibt-0190-0291-iml" (BlazarI bootloader), "ibt-0190-0291-<fw id>"
    pub fn firmware_stem(&self) -> String {
        let (cnvi, cnvr) = (top_id(self.cnvi_top), top_id(self.cnvr_top));
        if self.hw_variant() >= 0x1E {
            if self.img_type == IMG_BOOTLOADER {
                return format!("ibt-{cnvi:04x}-{cnvr:04x}-iml");
            }
            if !self.fw_id.is_empty() {
                return format!("ibt-{cnvi:04x}-{cnvr:04x}-{}", self.fw_id);
            }
        }
        format!("ibt-{cnvi:04x}-{cnvr:04x}")
    }

    /// Fills in the TLVs of a Read Version answer (after the status byte).
    /// Fields the answer lacks keep their value, as in Linux: the answer
    /// after a boot has fewer fields than the bootloader's.
    pub fn update(&mut self, mut data: &[u8]) -> Result<(), VersionError> {
        while !data.is_empty() {
            let [t, len, rest @ ..] = data else { return Err(VersionError::Malformed) };
            let (t, len) = (*t, *len as usize);
            if rest.len() < len || len < tlv_min_len(t) {
                return Err(VersionError::Malformed);
            }
            let v = &rest[..len];
            let u16_at = || u16::from_le_bytes([v[0], v[1]]);
            let u32_at = || u32::from_le_bytes([v[0], v[1], v[2], v[3]]);
            match t {
                TLV_CNVI_TOP => self.cnvi_top = u32_at(),
                TLV_CNVR_TOP => self.cnvr_top = u32_at(),
                TLV_CNVI_BT => self.cnvi_bt = u32_at(),
                TLV_CNVR_BT => self.cnvr_bt = u32_at(),
                TLV_DEV_REV_ID => self.dev_rev_id = u16_at(),
                TLV_IMAGE_TYPE => self.img_type = v[0],
                TLV_TIME_STAMP => self.timestamp = u16_at(),
                TLV_BUILD_TYPE => self.build_type = v[0],
                TLV_BUILD_NUM => self.build_num = u32_at(),
                TLV_SECURE_BOOT => self.secure_boot = v[0],
                TLV_OTP_LOCK => self.otp_lock = v[0],
                TLV_API_LOCK => self.api_lock = v[0],
                TLV_DEBUG_LOCK => self.debug_lock = v[0],
                TLV_MIN_FW => self.min_fw = (v[0], v[1], v[2]),
                TLV_LIMITED_CCE => self.limited_cce = v[0],
                TLV_SBE_TYPE => self.sbe_type = v[0],
                TLV_UNLOCK_POLICY => self.unlock_policy = v[0],
                TLV_FW_ID => {
                    let id = v.split(|&b| b == 0).next().unwrap_or_default();
                    self.fw_id = String::from_utf8_lossy(id).into_owned();
                }
                _ => {}
            }
            data = &rest[len..];
        }
        Ok(())
    }
}

/// "ibt-XXXX" part of a CNVi / CNVr top register: type and step, byte-swapped
fn top_id(top: u32) -> u16 {
    let kind = (top & 0x0FFF) as u16;
    let step = ((top >> 24) & 0x0F) as u16;
    ((kind << 4) | step).swap_bytes()
}

/// A Read Version answer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Version {
    Legacy(LegacyVersion),
    Tlv(TlvVersion),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionError {
    /// Command Complete with this error status
    Status(u8),
    Malformed,
}

/// Parses the return parameters (status first) of Read Version with the
/// parameter 0xFF: legacy controllers ignore it and answer in their format
pub fn parse_version(ret: &[u8]) -> Result<Version, VersionError> {
    match ret.first() {
        None => return Err(VersionError::Malformed),
        Some(&status) if status != 0 => return Err(VersionError::Status(status)),
        Some(_) => {}
    }
    if let Some(legacy) = parse_legacy_version(ret) {
        return Ok(Version::Legacy(legacy));
    }
    let mut v = TlvVersion::default();
    v.update(&ret[1..])?;
    if v.cnvi_bt == 0 {
        return Err(VersionError::Malformed);
    }
    Ok(Version::Tlv(v))
}

/// The legacy answer: exactly 10 bytes with Intel's platform second
pub fn parse_legacy_version(ret: &[u8]) -> Option<LegacyVersion> {
    let &[
        0,
        hw_platform,
        hw_variant,
        hw_revision,
        fw_variant,
        fw_revision,
        fw_build_num,
        fw_build_ww,
        fw_build_yy,
        fw_patch_num,
    ] = ret
    else {
        return None;
    };
    (hw_platform == HW_PLATFORM).then_some(LegacyVersion {
        hw_platform,
        hw_variant,
        hw_revision,
        fw_variant,
        fw_revision,
        fw_build_num,
        fw_build_ww,
        fw_build_yy,
        fw_patch_num,
    })
}

/// Legacy Read Secure Boot Params: the fields the download uses
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BootParams {
    pub dev_revid: u16,
    pub secure_boot: u8,
    pub otp_lock: u8,
    pub api_lock: u8,
    pub debug_lock: u8,
    pub min_fw: (u8, u8, u8),
    /// Not 0: the bootloader does not acknowledge every fragment
    pub limited_cce: u8,
}

/// Parses the return parameters (status first) of Read Secure Boot Params
pub fn parse_boot_params(ret: &[u8]) -> Option<BootParams> {
    if ret.len() < 23 || ret[0] != 0 {
        return None;
    }
    Some(BootParams {
        dev_revid: u16::from_le_bytes([ret[4], ret[5]]),
        secure_boot: ret[6],
        otp_lock: ret[9],
        api_lock: ret[10],
        debug_lock: ret[11],
        min_fw: (ret[18], ret[19], ret[20]),
        limited_cce: ret[21],
    })
}

/// The firmware file name without extension of a legacy bootloader
/// controller, None for variants without one
pub fn legacy_firmware_stem(v: &LegacyVersion, dev_revid: u16) -> Option<String> {
    match v.hw_variant {
        // SfP, WsP
        0x0B | 0x0C => Some(format!("ibt-{}-{}", v.hw_variant, dev_revid)),
        // JfP, ThP, HrP, CcP
        0x11..=0x14 => Some(format!("ibt-{}-{}-{}", v.hw_variant, v.hw_revision, v.fw_revision)),
        _ => None,
    }
}

// ==========================================================================
// Firmware files
// ==========================================================================

/// The header the controller's secure boot engine checks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Header {
    Rsa,
    Ecdsa,
    /// RSA / ECDSA with LMS (Scorpius Peak 2 and later)
    Hybrid,
}

/// A firmware image as Secure Send commands
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Complete HCI commands: 0xFC09, length, fragment type, data
    pub commands: Vec<Vec<u8>>,
    pub header: Header,
    /// From the image's Write Boot Params (0 without one, as in Linux)
    pub boot_address: u32,
    /// Firmware build number, week, year - 2000 (Write Boot Params)
    pub build: Option<(u8, u8, u8)>,
}

fn le32_at(data: &[u8], at: usize) -> Option<u32> {
    data.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn command(opcode: u16, params: &[u8]) -> Vec<u8> {
    let [lo, hi] = opcode.to_le_bytes();
    let mut c = Vec::with_capacity(3 + params.len());
    c.extend_from_slice(&[lo, hi, params.len() as u8]);
    c.extend_from_slice(params);
    c
}

/// Secure Send commands for `data`, in pieces of up to 252 bytes
fn secure_send(commands: &mut Vec<Vec<u8>>, fragment_type: u8, data: &[u8]) {
    for piece in data.chunks(SECURE_SEND_MAX) {
        let mut c = Vec::with_capacity(4 + piece.len());
        c.extend_from_slice(&[0x09, 0xFC, 1 + piece.len() as u8, fragment_type]);
        c.extend_from_slice(piece);
        commands.push(c);
    }
}

/// Cuts an .sfi into Secure Send commands for a controller of `hw_variant`
/// whose secure boot engine is `sbe_type` (legacy controllers: 0)
pub fn parse_sfi(data: &[u8], hw_variant: u8, sbe_type: u8) -> Result<Image, &'static str> {
    let css = le32_at(data, CSS_VERSION_OFFSET).ok_or("too short for a firmware header")?;
    let (header, sections, payload): (Header, &[(u8, usize, usize)], usize) = if hw_variant <= 0x14 {
        if css != CSS_RSA {
            return Err("not an RSA image (the file is not for this controller)");
        }
        if sbe_type != 0 {
            return Err("the controller wants another secure boot engine than RSA");
        }
        (Header::Rsa, &RSA_SECTIONS, RSA_PAYLOAD)
    } else if hw_variant >= 0x17 && css == CSS_RSA {
        // The ECDSA header follows the RSA header; the engine takes one
        if data.get(ECDSA_HEADER_OFFSET) != Some(&0x06)
            || le32_at(data, ECDSA_HEADER_OFFSET + CSS_VERSION_OFFSET) != Some(CSS_ECDSA)
        {
            return Err("no ECDSA header after the RSA header");
        }
        match sbe_type {
            0 => (Header::Rsa, &RSA_SECTIONS, RSA_ECDSA_PAYLOAD),
            1 => (Header::Ecdsa, &ECDSA_SECTIONS, RSA_ECDSA_PAYLOAD),
            _ => return Err("unknown secure boot engine type"),
        }
    } else if hw_variant >= 0x20 && css == CSS_HYBRID {
        (Header::Hybrid, &HYBRID_SECTIONS, HYBRID_PAYLOAD)
    } else {
        return Err("header version does not match the controller (the file is not for it)");
    };
    if data.len() <= payload {
        return Err("no command buffer after the header");
    }
    let mut commands = Vec::new();
    for &(fragment_type, at, len) in sections {
        secure_send(&mut commands, fragment_type, &data[at..at + len]);
    }
    // The command buffer: HCI commands, sent in sets whose length is a
    // multiple of 4 (the file has Intel NOPs to make it so)
    let (mut boot_address, mut build) = (0, None);
    let (mut start, mut at) = (payload, payload);
    while at < data.len() {
        let Some(&[lo, hi, plen]) = data.get(at..at + 3) else {
            return Err("command buffer ends inside a command");
        };
        let end = at + 3 + plen as usize;
        if end > data.len() {
            return Err("command buffer ends inside a command");
        }
        // Each SKU boots from its own address, which only the image knows
        if u16::from_le_bytes([lo, hi]) == OP_WRITE_BOOT_PARAMS && plen >= 7 && build.is_none() {
            boot_address = le32_at(data, at + 3).unwrap_or(0);
            build = Some((data[at + 7], data[at + 8], data[at + 9]));
        }
        at = end;
        if (at - start) % 4 == 0 {
            secure_send(&mut commands, FRAGMENT_DATA, &data[start..at]);
            start = at;
        }
    }
    if start != at {
        return Err("command buffer is not 4-byte aligned");
    }
    Ok(Image { commands, header, boot_address, build })
}

/// Write DDC commands for the records of a .ddc: length (1 byte, of what
/// follows), DDC ID (2 bytes), value
pub fn parse_ddc(data: &[u8]) -> Result<Vec<Vec<u8>>, &'static str> {
    let mut commands = Vec::new();
    let mut at = 0;
    while at < data.len() {
        let len = 1 + data[at] as usize;
        if len < 3 || len > u8::MAX as usize || at + len > data.len() {
            return Err("malformed DDC record");
        }
        commands.push(command(OP_WRITE_DDC, &data[at..at + len]));
        at += len;
    }
    Ok(commands)
}

// ==========================================================================
// The loader
// ==========================================================================

/// After a boot: the legacy DDC (its name is known before), or Read Version
/// (TLV: an intermediate loader or the operational firmware)
#[derive(Debug, Clone)]
enum AfterBoot {
    Ddc(String),
    ReadVersion(TlvVersion),
}

#[derive(Debug)]
struct Download {
    file: String,
    commands: Vec<Vec<u8>>,
    /// The next command to send
    next: usize,
    boot_address: u32,
    /// Vendor event 0x06 seen: its result
    result: Option<u8>,
    then: AfterBoot,
    started: Instant,
}

#[derive(Debug)]
enum State {
    Idle,
    /// Read Version (0xFF) sent, this many times
    Version(u8),
    /// Legacy Read Version sent (a legacy controller answered in TLV)
    LegacyVersion,
    BootParams(LegacyVersion),
    /// Secure Send commands, one after the other's Command Complete
    Sending(Download),
    /// Every command acknowledged: vendor event 0x06 next
    Checking(Download),
    /// Intel Reset sent: vendor event 0x02 next
    Booting(Download),
    /// Read Version after a boot
    VersionAfterBoot(TlvVersion),
    Ddc {
        commands: Vec<Vec<u8>>,
        next: usize,
    },
    EventMask {
        ddc: bool,
    },
    Done,
}

impl State {
    /// The command whose answer the state waits for
    fn awaited(&self) -> Option<u16> {
        Some(match self {
            State::Version(_) | State::LegacyVersion | State::VersionAfterBoot(_) => OP_READ_VERSION,
            State::BootParams(_) => OP_READ_BOOT_PARAMS,
            State::Sending(_) => OP_SECURE_SEND,
            State::Booting(_) => OP_RESET,
            State::Ddc { .. } => OP_WRITE_DDC,
            State::EventMask { .. } => OP_SET_EVENT_MASK,
            State::Idle | State::Checking(_) | State::Done => return None,
        })
    }

    fn step(&self) -> &'static str {
        match self {
            State::Idle | State::Done => "idle",
            State::Version(_) | State::LegacyVersion | State::VersionAfterBoot(_) => "Read Version",
            State::BootParams(_) => "Read Secure Boot Params",
            State::Sending(_) => "Secure Send",
            State::Checking(_) => "firmware check",
            State::Booting(_) => "boot",
            State::Ddc { .. } => "Write DDC",
            State::EventMask { .. } => "Set Event Mask",
        }
    }
}

/// An HCI event, as far as the loader cares
enum Event<'a> {
    Complete { opcode: u16, ret: &'a [u8] },
    Status { opcode: u16, status: u8 },
    Vendor { code: u8, data: &'a [u8] },
    Other,
}

fn classify(event: &[u8]) -> Event<'_> {
    let Some(p) = event.get(2..2 + *event.get(1).unwrap_or(&0) as usize) else { return Event::Other };
    match (event[0], p) {
        (EVENT_COMMAND_COMPLETE, [_, lo, hi, ret @ ..]) => {
            Event::Complete { opcode: u16::from_le_bytes([*lo, *hi]), ret }
        }
        (EVENT_COMMAND_STATUS, [status, _, lo, hi, ..]) => {
            Event::Status { opcode: u16::from_le_bytes([*lo, *hi]), status: *status }
        }
        (EVENT_VENDOR, [code, data @ ..]) => Event::Vendor { code: *code, data },
        _ => Event::Other,
    }
}

/// The firmware download as a state machine: [`start`](Self::start), then
/// every HCI event to [`on_event`](Self::on_event) and the timer to
/// [`on_timeout`](Self::on_timeout), until [`Action::Done`]
pub struct Loader {
    read_file: FileReader,
    state: State,
    /// Images booted this start
    downloads: u8,
    /// The last .sfi sent
    last_firmware: Option<String>,
    /// [`Action::Bootloader`] is on
    bootloader: bool,
    started: Instant,
}

impl Loader {
    pub fn new(read_file: FileReader) -> Loader {
        Loader {
            read_file,
            state: State::Idle,
            downloads: 0,
            last_firmware: None,
            bootloader: false,
            started: Instant::now(),
        }
    }

    pub fn done(&self) -> bool {
        matches!(self.state, State::Done)
    }

    pub fn start(&mut self) -> Vec<Action> {
        self.started = Instant::now();
        self.downloads = 0;
        self.last_firmware = None;
        self.bootloader = false;
        self.state = State::Version(1);
        vec![self.bootloader(true), Action::Command(command(OP_READ_VERSION, &[0xFF])), Action::Timer(VERSION_RETRY_MS)]
    }

    pub fn on_timeout(&mut self) -> Vec<Action> {
        if self.done() {
            return Vec::new();
        }
        if let State::Version(tries) = self.state
            && tries < VERSION_TRIES
        {
            tracing::info!("Intel Read Version: no answer, sending it again ({}/{VERSION_TRIES})", tries + 1);
            self.state = State::Version(tries + 1);
            return vec![Action::Command(command(OP_READ_VERSION, &[0xFF])), Action::Timer(VERSION_RETRY_MS)];
        }
        let step = self.state.step();
        self.finish(Err(LoadError::Timeout(step)))
    }

    /// An HCI event from the controller (either pipe)
    pub fn on_event(&mut self, event: &[u8]) -> Vec<Action> {
        match classify(event) {
            Event::Complete { opcode, ret } if Some(opcode) == self.state.awaited() => self.on_complete(opcode, ret),
            Event::Status { opcode, status } if status != 0 && Some(opcode) == self.state.awaited() => {
                self.finish(Err(LoadError::Command { opcode, status }))
            }
            Event::Vendor { code: VENDOR_SECURE_SEND_RESULT, data } if data.len() == 4 => self.on_check_result(data[0]),
            Event::Vendor { code: VENDOR_BOOTUP, data } if data.len() == 6 => self.on_bootup(),
            _ => Vec::new(),
        }
    }

    fn finish(&mut self, result: Result<Outcome, LoadError>) -> Vec<Action> {
        let ms = self.started.elapsed().as_millis();
        tracing::debug!("Intel firmware loader finished after {ms} ms: {result:?}");
        self.state = State::Done;
        let mut actions = Vec::new();
        // A failure leaves the controller in its loader
        if self.bootloader && result.is_ok() {
            actions.push(self.bootloader(false));
        }
        actions.push(Action::Done(result));
        actions
    }

    fn bootloader(&mut self, on: bool) -> Action {
        self.bootloader = on;
        Action::Bootloader(on)
    }

    fn on_complete(&mut self, opcode: u16, ret: &[u8]) -> Vec<Action> {
        let status = ret.first().copied().unwrap_or(0xFF);
        match std::mem::replace(&mut self.state, State::Idle) {
            State::Version(_) => match parse_version(ret) {
                Ok(Version::Legacy(v)) => self.legacy(v),
                Ok(Version::Tlv(v)) => self.tlv(v),
                Err(VersionError::Status(status)) => self.finish(Err(LoadError::Command { opcode, status })),
                Err(VersionError::Malformed) => self.finish(Err(LoadError::BadVersion)),
            },
            State::LegacyVersion => match parse_legacy_version(ret) {
                Some(v) => self.legacy(v),
                None if status != 0 => self.finish(Err(LoadError::Command { opcode, status })),
                None => self.finish(Err(LoadError::BadVersion)),
            },
            State::BootParams(v) => self.legacy_download(v, ret),
            State::Sending(mut d) => {
                // Linux reads no status here either: vendor event 0x06 has
                // the verdict on the whole image
                if status != 0 {
                    tracing::warn!("Intel Secure Send {} of {}: status 0x{status:02X}", d.next, d.commands.len());
                }
                if let Some(next) = d.commands.get(d.next) {
                    let next = next.clone();
                    d.next += 1;
                    self.state = State::Sending(d);
                    return vec![Action::BulkCommand(next), Action::Timer(COMMAND_TIMEOUT_MS)];
                }
                tracing::debug!(
                    "Intel firmware {}: {} commands sent in {} ms",
                    d.file,
                    d.commands.len(),
                    d.started.elapsed().as_millis()
                );
                match d.result {
                    Some(result) => {
                        self.state = State::Checking(d);
                        self.on_check_result(result)
                    }
                    None => {
                        self.state = State::Checking(d);
                        vec![Action::Timer(CHECK_TIMEOUT_MS)]
                    }
                }
            }
            State::Booting(d) => {
                self.state = State::Booting(d);
                if status != 0 {
                    return self.finish(Err(LoadError::Command { opcode, status }));
                }
                Vec::new()
            }
            State::VersionAfterBoot(mut v) => {
                if status != 0 {
                    return self.finish(Err(LoadError::Command { opcode, status }));
                }
                if v.update(&ret[1..]).is_err() {
                    return self.finish(Err(LoadError::BadVersion));
                }
                log_tlv(&v);
                self.tlv_image(v)
            }
            State::Ddc { commands, next } => {
                if status != 0 {
                    // The controller works without its DDC, as in Linux
                    tracing::warn!("Intel Write DDC {next} of {}: status 0x{status:02X}", commands.len());
                    return self.event_mask(false);
                }
                match commands.get(next) {
                    Some(c) => {
                        let c = c.clone();
                        self.state = State::Ddc { commands, next: next + 1 };
                        vec![Action::Command(c), Action::Timer(COMMAND_TIMEOUT_MS)]
                    }
                    None => {
                        tracing::info!("Intel DDC applied ({} records)", commands.len());
                        self.event_mask(true)
                    }
                }
            }
            State::EventMask { ddc } => {
                if status != 0 {
                    tracing::warn!("Intel Set Event Mask: status 0x{status:02X}");
                }
                let firmware = self.last_firmware.take().unwrap_or_default();
                let detail = if ddc { "DDC applied" } else { "DDC not applied" };
                self.finish(Ok(Outcome::Loaded { firmware, detail: Some(detail) }))
            }
            state @ (State::Idle | State::Checking(_) | State::Done) => {
                self.state = state;
                Vec::new()
            }
        }
    }

    // ---- legacy ----

    fn legacy(&mut self, v: LegacyVersion) -> Vec<Action> {
        tracing::info!(
            "Intel legacy controller: variant 0x{:02X}, revision 0x{:02X}, {} (fw variant 0x{:02X}), firmware build {}-{}.{} revision 0x{:02X}",
            v.hw_variant,
            v.hw_revision,
            match v.fw_variant {
                FW_VARIANT_BOOTLOADER => "bootloader",
                FW_VARIANT_OPERATIONAL => "operational firmware",
                _ => "unknown image",
            },
            v.fw_variant,
            v.fw_build_num,
            v.fw_build_ww,
            2000 + v.fw_build_yy as u32,
            v.fw_revision
        );
        match v.hw_variant {
            // WP, StP: ROM firmware (Linux patches it with a .bseq; not done)
            0x07 | 0x08 => self.finish(Ok(Outcome::Rom)),
            0x0B | 0x0C | 0x11..=0x14 => match v.fw_variant {
                FW_VARIANT_OPERATIONAL => self.finish(Ok(Outcome::AlreadyRunning)),
                FW_VARIANT_BOOTLOADER => {
                    self.state = State::BootParams(v);
                    vec![Action::Command(command(OP_READ_BOOT_PARAMS, &[])), Action::Timer(COMMAND_TIMEOUT_MS)]
                }
                _ => self.finish(Err(LoadError::Unsupported("unknown legacy firmware variant"))),
            },
            variant => self.finish(Err(LoadError::UnsupportedVariant(variant))),
        }
    }

    fn legacy_download(&mut self, v: LegacyVersion, ret: &[u8]) -> Vec<Action> {
        let Some(params) = parse_boot_params(ret) else {
            let status = ret.first().copied().unwrap_or(0xFF);
            return self.finish(Err(LoadError::Command { opcode: OP_READ_BOOT_PARAMS, status }));
        };
        tracing::info!(
            "Intel boot params: device revision {}, secure boot {}, OTP lock {}, API lock {}, debug lock {}, minimum firmware {}-{}.{}",
            params.dev_revid,
            params.secure_boot,
            params.otp_lock,
            params.api_lock,
            params.debug_lock,
            params.min_fw.0,
            params.min_fw.1,
            2000 + params.min_fw.2 as u32
        );
        if params.limited_cce != 0 {
            return self.finish(Err(LoadError::Unsupported("bootloader that does not acknowledge every fragment")));
        }
        let Some(stem) = legacy_firmware_stem(&v, params.dev_revid) else {
            return self.finish(Err(LoadError::UnsupportedVariant(v.hw_variant)));
        };
        self.download(&stem, v.hw_variant, 0, AfterBoot::Ddc(stem.clone()))
    }

    // ---- TLV ----

    fn tlv(&mut self, v: TlvVersion) -> Vec<Action> {
        log_tlv(&v);
        if v.hw_platform() != HW_PLATFORM {
            return self.finish(Err(LoadError::Unsupported("not an Intel hardware platform")));
        }
        match v.hw_variant() {
            // JfP .. CcP: legacy bootloaders whose operational firmware
            // also answers in TLV; the legacy answer names their files
            0x11..=0x14 => {
                self.state = State::LegacyVersion;
                vec![Action::Command(command(OP_READ_VERSION, &[])), Action::Timer(COMMAND_TIMEOUT_MS)]
            }
            0x17..=0x19 | 0x1B..=0x23 => self.tlv_image(v),
            variant => self.finish(Err(LoadError::UnsupportedVariant(variant))),
        }
    }

    /// The next step for the image the controller runs
    fn tlv_image(&mut self, v: TlvVersion) -> Vec<Action> {
        match v.img_type {
            IMG_OPERATIONAL if self.downloads == 0 => self.finish(Ok(Outcome::AlreadyRunning)),
            IMG_OPERATIONAL => {
                let mut actions = vec![self.bootloader(false)];
                actions.extend(self.ddc(&v.firmware_stem()));
                actions
            }
            IMG_BOOTLOADER if v.limited_cce != 0 => {
                self.finish(Err(LoadError::Unsupported("bootloader that does not acknowledge every fragment")))
            }
            IMG_BOOTLOADER if v.sbe_type > 0x03 => self.finish(Err(LoadError::Unsupported("secure boot engine type"))),
            IMG_BOOTLOADER | IMG_IML if self.downloads < MAX_DOWNLOADS => {
                if v.api_lock != 0 && v.unlock_policy == 0 && matches!(v.hw_variant(), 0x1C..=0x1E | 0x22) {
                    tracing::warn!(
                        "Intel controller is API-locked; the unlocker image (intel/unlocker.sfi) is not loaded"
                    );
                }
                let stem = v.firmware_stem();
                let (variant, sbe_type) = (v.hw_variant(), v.sbe_type);
                self.download(&stem, variant, sbe_type, AfterBoot::ReadVersion(v))
            }
            IMG_BOOTLOADER | IMG_IML => self.finish(Err(LoadError::Unsupported("the controller stays in its loader"))),
            _ => self.finish(Err(LoadError::Unsupported("unknown image type"))),
        }
    }

    // ---- download and boot ----

    fn download(&mut self, stem: &str, hw_variant: u8, sbe_type: u8, then: AfterBoot) -> Vec<Action> {
        let file = format!("{stem}.sfi");
        let Some(data) = (self.read_file)(&file) else { return self.finish(Err(LoadError::MissingFile(file))) };
        let image = match parse_sfi(&data, hw_variant, sbe_type) {
            Ok(image) => image,
            Err(why) => return self.finish(Err(LoadError::BadFile { file, why })),
        };
        let build = image
            .build
            .map_or_else(|| "unknown".to_owned(), |(num, ww, yy)| format!("{num}-{ww}.{}", 2000 + yy as u32));
        tracing::info!(
            "Intel firmware {file}: {} bytes, {:?} header, {} Secure Send commands, boot address 0x{:08X}, build {build}",
            data.len(),
            image.header,
            image.commands.len(),
            image.boot_address
        );
        if image.build.is_none() {
            tracing::warn!("Intel firmware {file} has no Write Boot Params: booting from address 0");
        }
        self.last_firmware = Some(file.clone());
        let first = image.commands[0].clone();
        self.state = State::Sending(Download {
            file,
            commands: image.commands,
            next: 1,
            boot_address: image.boot_address,
            result: None,
            then,
            started: Instant::now(),
        });
        let mut actions = Vec::new();
        if !self.bootloader {
            actions.push(self.bootloader(true));
        }
        actions.extend([Action::BulkCommand(first), Action::Timer(COMMAND_TIMEOUT_MS)]);
        actions
    }

    fn on_check_result(&mut self, result: u8) -> Vec<Action> {
        match std::mem::replace(&mut self.state, State::Idle) {
            State::Sending(mut d) => {
                // Before the last Command Complete: kept for then
                d.result = Some(result);
                self.state = State::Sending(d);
                Vec::new()
            }
            State::Checking(d) => {
                if result != 0 {
                    self.state = State::Checking(d);
                    return self.finish(Err(LoadError::Rejected(result)));
                }
                tracing::info!("Intel firmware {} accepted in {} ms", d.file, d.started.elapsed().as_millis());
                // Reset type soft, patch enable, no DDC reload, boot from
                // the address given
                let mut params = vec![0x00, 0x01, 0x00, 0x01];
                params.extend_from_slice(&d.boot_address.to_le_bytes());
                self.state = State::Booting(Download { started: Instant::now(), ..d });
                vec![Action::Command(command(OP_RESET, &params)), Action::Timer(BOOT_TIMEOUT_MS)]
            }
            state => {
                self.state = state;
                Vec::new()
            }
        }
    }

    fn on_bootup(&mut self) -> Vec<Action> {
        let State::Booting(d) = std::mem::replace(&mut self.state, State::Idle) else {
            // Not waiting for one (e.g. the controller restarted on its own)
            tracing::debug!("Intel bootup event outside a boot: ignored");
            return Vec::new();
        };
        self.downloads += 1;
        tracing::info!("Intel firmware {} booted in {} ms", d.file, d.started.elapsed().as_millis());
        match d.then {
            // Legacy: the operational firmware runs
            AfterBoot::Ddc(stem) => {
                let mut actions = vec![self.bootloader(false)];
                actions.extend(self.ddc(&stem));
                actions
            }
            // TLV: the intermediate loader or the operational firmware, as
            // Read Version tells; bootloader mode stays until the latter
            AfterBoot::ReadVersion(v) => {
                self.state = State::VersionAfterBoot(v);
                vec![Action::Command(command(OP_READ_VERSION, &[0xFF])), Action::Timer(COMMAND_TIMEOUT_MS)]
            }
        }
    }

    // ---- configuration ----

    fn ddc(&mut self, stem: &str) -> Vec<Action> {
        let file = format!("{stem}.ddc");
        let commands = match (self.read_file)(&file).map(|d| parse_ddc(&d)) {
            Some(Ok(c)) if !c.is_empty() => c,
            Some(Ok(_)) => {
                tracing::warn!("Intel DDC {file} is empty");
                return self.event_mask(false);
            }
            Some(Err(why)) => {
                tracing::warn!("Intel DDC {file}: {why}; the controller runs with its defaults");
                return self.event_mask(false);
            }
            None => {
                tracing::warn!("Intel DDC {file} not found; the controller runs with its defaults");
                return self.event_mask(false);
            }
        };
        let first = commands[0].clone();
        self.state = State::Ddc { commands, next: 1 };
        vec![Action::Command(first), Action::Timer(COMMAND_TIMEOUT_MS)]
    }

    fn event_mask(&mut self, ddc: bool) -> Vec<Action> {
        self.state = State::EventMask { ddc };
        vec![Action::Command(command(OP_SET_EVENT_MASK, &EVENT_MASK)), Action::Timer(COMMAND_TIMEOUT_MS)]
    }
}

impl ChipLoader for Loader {
    fn vendor(&self) -> &'static str {
        "Intel"
    }

    fn start(&mut self) -> Vec<Action> {
        Loader::start(self)
    }

    fn on_event(&mut self, event: &[u8]) -> Vec<Action> {
        Loader::on_event(self, event)
    }

    fn on_timeout(&mut self) -> Vec<Action> {
        Loader::on_timeout(self)
    }

    /// An intermediate loader has no .ddc
    fn missing_file_hint(&self, file: &str) -> String {
        let stem = file.strip_suffix(".sfi").unwrap_or(file);
        let files = if stem.ends_with("-iml") { format!("{stem}.sfi") } else { format!("{stem}.sfi and {stem}.ddc") };
        format!("{files} from linux-firmware (intel/)")
    }
}

fn log_tlv(v: &TlvVersion) {
    tracing::info!(
        "Intel TLV controller: variant 0x{:02X}, {}, CNVi 0x{:08X}, CNVr 0x{:08X}, secure boot engine {}, minimum firmware {}-{}.{}, firmware file {}.sfi",
        v.hw_variant(),
        match v.img_type {
            IMG_BOOTLOADER => "bootloader",
            IMG_IML => "intermediate loader",
            IMG_OPERATIONAL => "operational firmware",
            _ => "unknown image",
        },
        v.cnvi_top,
        v.cnvr_top,
        match v.sbe_type {
            0 => "RSA",
            1 => "ECDSA",
            _ => "unknown",
        },
        v.min_fw.0,
        v.min_fw.1,
        2000 + v.min_fw.2 as u32,
        v.firmware_stem()
    );
}
