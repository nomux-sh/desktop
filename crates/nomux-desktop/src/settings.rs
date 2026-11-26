//! The settings file of the client: one `key = value` for each line.
//!
//! - macOS: `~/Library/Application Support/sh.nomux.desktop/settings`
//! - Linux: `$XDG_CONFIG_HOME/nomux-desktop/settings`, by default
//!   `~/.config/nomux-desktop/settings`
//!
//! The keys are `notify.needs-you` and `notify.done`; the values are `on`
//! (the default) and `off`.

use std::path::PathBuf;

use nomux_desktop_model::notification::NotificationSettings;

fn path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let directory = if cfg!(target_os = "macos") {
        home?.join("Library/Application Support/sh.nomux.desktop")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|directory| directory.is_absolute())
            .or_else(|| home.map(|home| home.join(".config")))?
            .join("nomux-desktop")
    };
    Some(directory.join("settings"))
}

/// The notification settings. A missing or unreadable file gives the
/// defaults: all kinds on.
pub(crate) fn notifications() -> NotificationSettings {
    path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map_or_else(NotificationSettings::default, |text| {
            NotificationSettings::from_text(&text)
        })
}
