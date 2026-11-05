//! BAS client (Battery Service, 0x180F): the headset's battery level, read
//! once and followed by notifications (experimental). With a set of
//! earbuds each side has its own service; the lowest level is reported.

use std::collections::BTreeMap;
use std::sync::mpsc::Sender;

use tracing::{info, warn};

use super::super::LeDeviceId;
use super::super::core::LeCtx;
use super::super::gatt_client::ATT_ERROR_HCI_DISCONNECT_RECEIVED;
use super::super::module::{LeModule, LeModuleEntry, LeModuleSetup};
use crate::HostEvent;

const UUID_SERVICE_BAS: u16 = 0x180F;
const UUID_BATTERY_LEVEL: u16 = 0x2A19;

/// The levels of the devices on the link: what to report
#[derive(Default)]
struct Levels(BTreeMap<u32, u8>);

impl Levels {
    fn set(&mut self, dev: u32, percent: u8) -> u8 {
        self.0.insert(dev, percent.min(100));
        self.lowest().unwrap_or(0)
    }

    /// Drops a device; the lowest of those left
    fn remove(&mut self, dev: u32) -> Option<u8> {
        self.0.remove(&dev);
        self.lowest()
    }

    fn lowest(&self) -> Option<u8> {
        self.0.values().copied().min()
    }
}

pub struct BasModule {
    events: Sender<HostEvent>,
    /// The setting is on (else nothing is read)
    enabled: bool,
    levels: Levels,
}

impl BasModule {
    fn on_level(ctx: &mut LeCtx<'_>, id: LeDeviceId, value: &[u8]) {
        let Some(&percent) = value.first() else {
            tracing::debug!("BAS: device {}: empty Battery Level", id.0);
            return;
        };
        let Some(m) = ctx.module_mut::<BasModule>() else { return };
        let lowest = m.levels.set(id.0, percent);
        info!("BAS: device {}: battery {percent}% (reported {lowest}%)", id.0);
        let _ = m.events.send(HostEvent::Battery(lowest));
    }
}

impl LeModule for BasModule {
    fn name(&self) -> &'static str {
        "BAS"
    }

    fn on_gatt_ready(&mut self, ctx: &mut LeCtx<'_>, dev: LeDeviceId) {
        if !self.enabled {
            return;
        }
        let Some(d) = ctx.device(dev) else { return };
        let Some(chr) = d.find_char(UUID_SERVICE_BAS, UUID_BATTERY_LEVEL).cloned() else {
            tracing::debug!("BAS: {}: no Battery Level", d.addr);
            return;
        };
        let con = d.acl;
        let addr = d.addr.to_string();
        info!("BAS: {addr}: Battery Level (0x{:04x})", chr.value_handle);
        let gatt = ctx.gatt();
        gatt.listen(con, chr.value_handle, Box::new(move |ctx, value| BasModule::on_level(ctx, dev, value)));
        // Notifications are optional in BAS: a refusal is no problem
        gatt.subscribe(con, &chr, false, None);
        gatt.read(
            con,
            chr.value_handle,
            Box::new(move |ctx, status, value| {
                if status == 0 {
                    BasModule::on_level(ctx, dev, value);
                } else if status != ATT_ERROR_HCI_DISCONNECT_RECEIVED {
                    warn!("BAS: reading the Battery Level failed (0x{status:02x})");
                }
            }),
        );
    }

    fn on_disconnected(&mut self, _ctx: &mut LeCtx<'_>, dev: LeDeviceId, _reason: u8) {
        self.levels.remove(dev.0);
    }

    fn on_stack_deinit(&mut self) {
        self.levels = Levels::default();
    }
}

pub fn create(setup: &mut LeModuleSetup) -> LeModuleEntry {
    LeModuleEntry {
        module: Box::new(BasModule { events: setup.events.clone(), enabled: setup.battery, levels: Levels::default() }),
        hooks: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lowest_level_of_the_devices_is_reported() {
        let mut l = Levels::default();
        assert_eq!(l.lowest(), None);
        assert_eq!(l.set(0, 80), 80);
        assert_eq!(l.set(1, 60), 60);
        assert_eq!(l.set(0, 90), 60);
        assert_eq!(l.set(1, 255), 90, "levels above 100 are clamped");
        assert_eq!(l.remove(0), Some(100));
        assert_eq!(l.remove(1), None);
    }
}
