//! BlueCaster's core, shared by the resident process (tray, IPC for the UI) and
//! the command line: the streaming state machine, the Bluetooth stack's
//! lifecycle, and the settings, profiles and devices of the settings folder
//! (v1's files, read and written in their format).

#![forbid(unsafe_code)]

pub mod adapter;
pub mod compat;
pub mod devices;
mod health;
mod le_media;
mod le_stream;
pub mod link;
pub mod messages;
pub mod migrate;
pub mod paths;
pub mod profiles;
mod service;
pub mod settings;
pub mod stream;
pub mod vendor;

pub use le_stream::{
    BROADCAST_PRESET_HQ, BROADCAST_PRESET_SQ, LeAudioPath, LeStreamDetails, broadcast_config, le_silence_set,
    le_stats_line,
};
pub use messages::Msg;
pub use service::{LeCliOptions, ScannedDevice, Service, ServiceEvent, ServiceOptions};
pub use stream::{
    AacDetails, AacDeviceCaps, CodecDetails, CodecDeviceCaps, KnownCodecs, SbcDetails, SbcDeviceCaps, SourceInfo,
    SourceRequest, StreamInfo, StreamRequest, StreamState, VolumeSync,
};
pub use vendor::huawei::is_huawei;
pub mod diag;
pub mod firmware;
pub mod mute_output;
pub mod probe_report;
pub mod receiver;
pub mod update;
pub mod volume_sync;
