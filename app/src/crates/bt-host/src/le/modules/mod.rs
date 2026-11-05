//! The profile modules (v1 le_audio_modules.cpp), one file each, one work
//! package each (WP5). A module file changes only itself; the registry
//! below is fixed.

pub mod bas;
pub mod ccp;
pub mod csip;
pub mod mcp;
pub mod tmap_gmap;
pub mod vcp;

use super::module::{LeModuleEntry, LeModuleSetup};

/// All modules in v1's fixed order: CSIP, VCP, MCP, CCP, TMAP / GMAP, then
/// BAS (the headset's battery level).
/// Called by the LE core on the BTstack thread when the stack starts with
/// LE Audio on; hooks run in this order.
pub fn create_all(setup: &mut LeModuleSetup) -> Vec<LeModuleEntry> {
    vec![
        csip::create(setup),
        vcp::create(setup),
        mcp::create(setup),
        ccp::create(setup),
        tmap_gmap::create(setup),
        bas::create(setup),
    ]
}
