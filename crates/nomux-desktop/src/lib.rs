//! The desktop client of nomux: one window with the project rail, the
//! sidebar and one terminal tab attached to a session of the daemon of the
//! local host.
//!
//! - [`HostConnection`]: the connection thread, with the requests to the
//!   daemon and the attach.
//! - [`Workbench`]: the root view of the window.
//! - [`TerminalTab`]: the terminal tab.
//! - [`open_window`]: opens the window, as the binary does.

mod attach;
pub mod fonts;
mod host_connection;
pub mod measure;
mod terminal_tab;
mod workbench;

use gpui::{App, AppContext, KeyBinding, WindowBounds, WindowHandle, WindowOptions, px, size};

pub use host_connection::{HostConnection, HostRequest, HostUpdate};
pub use terminal_tab::{Copy, Paste, TerminalTab};
pub use workbench::{Appearance, Workbench};

use crate::measure::Measurement;

/// The size of a new window, in logical pixels.
const WINDOW_SIZE: (f32, f32) = (1280.0, 800.0);

/// Registers the key bindings of the client.
pub fn bind_keys(cx: &mut App) {
    let (copy, paste) = if cfg!(target_os = "macos") {
        ("cmd-c", "cmd-v")
    } else {
        ("ctrl-shift-c", "ctrl-shift-v")
    };
    cx.bind_keys([
        KeyBinding::new(copy, Copy, Some(terminal_tab::KEY_CONTEXT)),
        KeyBinding::new(paste, Paste, Some(terminal_tab::KEY_CONTEXT)),
    ]);
}

/// Starts the connection to the daemon and opens the window.
///
/// # Errors
///
/// Returns an error if the connection thread cannot start or the window
/// cannot open.
pub fn open_window(
    appearance: Appearance,
    measurement: Option<Measurement>,
    cx: &mut App,
) -> gpui::Result<WindowHandle<Workbench>> {
    let (connection, updates) = HostConnection::start()?;
    let bounds = gpui::Bounds::centered(None, size(px(WINDOW_SIZE.0), px(WINDOW_SIZE.1)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        app_id: Some("sh.nomux.desktop".to_owned()),
        ..WindowOptions::default()
    };
    cx.open_window(options, |window, cx| {
        cx.new(|cx| Workbench::new(connection, updates, appearance, measurement, window, cx))
    })
}
