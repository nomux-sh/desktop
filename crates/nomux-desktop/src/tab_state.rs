//! The file with the open tabs, read at the start and written after each
//! change, so that the next start of the client opens the same tabs.
//!
//! - macOS: `~/Library/Application Support/sh.nomux.desktop/tabs`
//! - Linux: `$XDG_STATE_HOME/nomux-desktop/tabs`, by default
//!   `~/.local/state/nomux-desktop/tabs`

use std::path::{Path, PathBuf};

use gpui::App;
use nomux_desktop_model::Tabs;

/// The path of the file, `None` without a home directory.
#[must_use]
pub(crate) fn path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let directory = if cfg!(target_os = "macos") {
        home?.join("Library/Application Support/sh.nomux.desktop")
    } else {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|directory| directory.is_absolute())
            .or_else(|| home.map(|home| home.join(".local/state")))?
            .join("nomux-desktop")
    };
    Some(directory.join("tabs"))
}

/// The saved tabs. A missing or unreadable file gives no tabs.
#[must_use]
pub(crate) fn load(path: Option<&Path>) -> Tabs {
    path.and_then(|path| std::fs::read_to_string(path).ok())
        .map_or_else(Tabs::default, |text| Tabs::from_text(&text))
}

/// Writes the tabs on a background thread. A failure is printed: the tabs
/// of this run are not lost, only the next start shows fewer.
pub(crate) fn save(path: Option<PathBuf>, tabs: &Tabs, cx: &App) {
    let Some(path) = path else {
        return;
    };
    let text = tabs.to_text();
    cx.background_executor()
        .spawn(async move {
            let written = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&path, text));
            if let Err(error) = written {
                eprintln!("nomux-desktop: cannot write {}: {error}", path.display());
            }
        })
        .detach();
}
