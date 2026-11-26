//! The "About nomux" dialog.

use gpui::{PromptLevel, Window};

/// Shows the version of the application in a dialog of the system.
pub(crate) fn show(window: &mut Window, cx: &mut gpui::App) {
    let detail = format!(
        "Version {}\n\nThe sessions run in the nomux daemon. They continue when you quit.",
        env!("CARGO_PKG_VERSION")
    );
    drop(window.prompt(PromptLevel::Info, "nomux", Some(&detail), &["OK"], cx));
}
