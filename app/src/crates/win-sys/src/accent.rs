//! The Windows accent colour, for the buttons that Windows' own accent
//! buttons would be (issue #86: follow the system theme)

use windows::UI::ViewManagement::{UIColorType, UISettings};

/// The accent fills Windows' controls use: AccentDark1 on a light theme,
/// AccentLight2 on a dark one, as "#rrggbb"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccentColors {
    pub light: String,
    pub dark: String,
}

/// None if Windows does not give them (the UI keeps its default blue)
pub fn accent_colors() -> Option<AccentColors> {
    let settings = UISettings::new().ok()?;
    let hex = |kind| settings.GetColorValue(kind).ok().map(|c| format!("#{:02x}{:02x}{:02x}", c.R, c.G, c.B));
    Some(AccentColors { light: hex(UIColorType::AccentDark1)?, dark: hex(UIColorType::AccentLight2)? })
}
