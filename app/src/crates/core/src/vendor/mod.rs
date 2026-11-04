//! What single vendors' devices need beyond the standard profiles, one module
//! per vendor: `channel` is the control channel they share.
//!
//! [`VENDORS`] lists them for the rest of the app: how their earbuds are
//! told apart, and the options developer mode offers for each. Developer
//! mode's overrides ([`set_overrides`]) are kept by vendor id, so a vendor
//! added here needs nothing more in the settings or the UI than the labels
//! of its options (issue #237).

mod channel;
pub mod huawei;
pub mod oppo;

use std::collections::BTreeMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

/// A vendor whose earbuds A2DPWB speaks to over their own protocol
pub struct Vendor {
    /// Key of its overrides (settings, IPC)
    pub id: &'static str,
    /// Shown, and in the log
    pub name: &'static str,
    /// Whether a device's name is one of its earbuds
    pub named: fn(&str) -> bool,
    /// Developer mode's switches of its own (ids; the UI labels them)
    pub options: &'static [&'static str],
}

pub const OPPO: &str = "oppo";
pub const HUAWEI: &str = "huawei";
/// OPPO: asked for Hi-Res on every connection, the record notwithstanding
pub const OPPO_EVERY_CONNECTION: &str = "every_connection";

pub const VENDORS: &[Vendor] = &[
    Vendor { id: HUAWEI, name: "Huawei", named: huawei::is_huawei, options: &[] },
    Vendor { id: OPPO, name: "OPPO", named: oppo::is_oppo, options: &[OPPO_EVERY_CONNECTION] },
];

pub fn vendor(id: &str) -> Option<&'static Vendor> {
    VENDORS.iter().find(|v| v.id == id)
}

/// When a vendor's protocol is used
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// For the earbuds whose name tells them
    #[default]
    Auto,
    /// For any earbuds, whatever their name
    Always,
    Never,
}

/// Developer mode's override of one vendor
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Override {
    pub mode: Mode,
    /// Its options turned on ([`Vendor::options`])
    pub options: Vec<String>,
}

impl Override {
    pub fn is_default(&self) -> bool {
        self.mode == Mode::Auto && self.options.is_empty()
    }
}

/// By vendor id; absent: the default
pub type Overrides = BTreeMap<String, Override>;

static OVERRIDES: RwLock<Overrides> = RwLock::new(BTreeMap::new());

/// Sets the overrides in force (none while developer mode is off); logs
/// when they change
pub fn set_overrides(mut o: Overrides) {
    o.retain(|id, v| vendor(id).is_some() && !v.is_default());
    let mut cur = OVERRIDES.write().unwrap_or_else(|e| e.into_inner());
    if *cur != o {
        if o.is_empty() {
            tracing::info!("vendor overrides: off");
        } else {
            tracing::warn!("vendor overrides: {o:?}");
        }
        *cur = o;
    }
}

/// When the vendor's protocol is used now
pub fn mode(id: &str) -> Mode {
    OVERRIDES.read().unwrap_or_else(|e| e.into_inner()).get(id).map_or(Mode::Auto, |o| o.mode)
}

/// Whether one of the vendor's options is on now
pub fn option(id: &str, option: &str) -> bool {
    OVERRIDES.read().unwrap_or_else(|e| e.into_inner()).get(id).is_some_and(|o| o.options.iter().any(|x| x == option))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_by_vendor_id() {
        let mut o = Overrides::new();
        o.insert(OPPO.into(), Override { mode: Mode::Always, options: vec![OPPO_EVERY_CONNECTION.into()] });
        o.insert(HUAWEI.into(), Override::default());
        o.insert("acme".into(), Override { mode: Mode::Never, options: Vec::new() });
        set_overrides(o);
        assert_eq!(mode(OPPO), Mode::Always);
        assert!(option(OPPO, OPPO_EVERY_CONNECTION));
        assert_eq!(mode(HUAWEI), Mode::Auto);
        // Unknown vendors and defaults are not kept
        assert_eq!(OVERRIDES.read().unwrap().len(), 1);
        set_overrides(Overrides::new());
        assert_eq!(mode(OPPO), Mode::Auto);
        assert!(!option(OPPO, OPPO_EVERY_CONNECTION));
    }

    #[test]
    fn each_vendor_once() {
        for v in VENDORS {
            assert_eq!(VENDORS.iter().filter(|x| x.id == v.id).count(), 1, "{}", v.id);
        }
        assert!((vendor(OPPO).unwrap().named)("OPPO Enco Free4"));
        assert!((vendor(HUAWEI).unwrap().named)("HUAWEI FreeBuds Pro 4"));
    }
}
