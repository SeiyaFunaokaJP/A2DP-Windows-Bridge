//! LE Audio (experimental): the logic that needs neither BTstack nor
//! Windows, ported from v1's app/src/le_audio (branch feat/le-audio, commit
//! b819893). Everything here is plain data in, data out, so that it can be
//! unit-tested without an adapter or the emulator.
//!
//! - [`defs`]: assigned numbers (UUIDs, LTV types, contexts, ASE states)
//!   (v1 le_audio_defs.h)
//! - [`ltv`]: length-type-value helpers (v1 ltv_find / ltv_put)
//! - [`pacs_ascs`]: PAC records, ASE values and ASE Control Point PDUs
//!   (v1 le_audio_util.cpp, the PDU building of le_audio_bap.cpp)
//! - [`presets`]: the BAP / GMAP preset table (v1 le_audio_presets.cpp)
//! - [`plan`]: stream planning for a group of devices (v1 plan_stream)
//! - [`csip_crypto`]: CSIS sef / sih / RSI (v1 le_audio_csip.cpp)
//! - [`vcp_scale`]: VCS volume values and the 0-127 <-> 0-255 scale
//!   (v1 le_audio_vcp.{h,cpp})
//! - [`ccp_model`]: the GTBS call model (v1 le_audio_ccp.cpp)
//! - [`mcp_model`]: the GMCS media model and backend trait
//!   (v1 le_audio_mcp.cpp, le_audio_mcp_media.h)
//! - [`db_hash`]: the GATT Database Hash (v1 GattServer::compute_db_hash)
//! - [`sdu_queue`]: the jitter buffer of SDU sets and the send guard
//!   (v1 le_audio_stream.cpp)
//! - [`broadcast`]: the broadcast source (Auracast): announcements, BASE,
//!   presets, the plan of the BIS
//!
//! The BTstack side (scan, connect, GATT, BAP state machine, ISO sending,
//! the profile modules) is in bt-host's `le` module; LC3 is in lc3-sys.

#![forbid(unsafe_code)]

pub mod broadcast;
pub mod ccp_model;
pub mod csip_crypto;
pub mod db_hash;
pub mod defs;
pub mod ltv;
pub mod mcp_model;
pub mod pacs_ascs;
pub mod plan;
pub mod presets;
pub mod sdu_queue;
pub mod vcp_scale;
