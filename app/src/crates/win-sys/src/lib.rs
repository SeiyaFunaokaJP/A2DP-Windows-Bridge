//! Win32 APIs for BlueCaster that no Rust library covers. The only crate besides
//! the `*-sys` crates where `unsafe` is allowed; every function here is
//! safe to call.

pub mod audio_policy;
pub mod autostart;
pub mod bt_paired;
pub mod bt_radios;
pub mod com;
pub mod console;
pub mod crash;
pub mod driver;
pub mod elevation;
pub mod locale;
pub mod media_keys;
pub mod mmcss;
pub mod notify;
pub mod perf;
pub mod pipe;
pub mod power;
pub mod process;
pub mod single_instance;
pub mod usb_adapters;
pub mod webview;
pub mod wlan;
