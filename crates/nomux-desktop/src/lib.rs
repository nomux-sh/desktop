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
#[cfg(feature = "frame-capture")]
pub mod capture;
pub mod fonts;
mod host_connection;
pub mod measure;
pub mod reduced_motion;
mod tab_bar;
mod tab_state;
mod terminal_tab;
mod terminal_tabs;
mod workbench;

use gpui::{
    App, AppContext, KeyBinding, TitlebarOptions, WindowBounds, WindowHandle, WindowOptions,
    actions, px, size,
};

pub use host_connection::{HostConnection, HostRequest, HostUpdate};
pub use terminal_tab::{Copy, Paste, TerminalTab};
pub use workbench::{Appearance, Workbench};

use crate::measure::Measurement;

/// The size of a new window, in logical pixels.
pub const WINDOW_SIZE: (f32, f32) = (1280.0, 800.0);

actions!(
    workbench,
    [
        /// Starts a session in the directory of the shown Project and opens
        /// it in a new tab.
        NewSession,
        /// Closes the active tab. The session continues.
        CloseTab,
        /// Activates the next tab.
        NextTab,
        /// Activates the previous tab.
        PreviousTab,
        /// Moves the active tab one place to the left.
        MoveTabLeft,
        /// Moves the active tab one place to the right.
        MoveTabRight,
        /// Activates the first tab.
        ActivateTab1,
        /// Activates the second tab.
        ActivateTab2,
        /// Activates the third tab.
        ActivateTab3,
        /// Activates the fourth tab.
        ActivateTab4,
        /// Activates the fifth tab.
        ActivateTab5,
        /// Activates the sixth tab.
        ActivateTab6,
        /// Activates the seventh tab.
        ActivateTab7,
        /// Activates the eighth tab.
        ActivateTab8,
        /// Activates the last tab.
        ActivateLastTab
    ]
);

/// The key bindings of the tabs on one platform.
struct TabKeys {
    new: &'static str,
    close: &'static str,
    next: &'static str,
    previous: &'static str,
    left: &'static str,
    right: &'static str,
    /// The tabs 1 to 8, then the last tab.
    numbers: [&'static str; 9],
}

/// macOS: the conventions of Safari and Terminal.
const MACOS_TAB_KEYS: TabKeys = TabKeys {
    new: "cmd-t",
    close: "cmd-w",
    next: "cmd-shift-]",
    previous: "cmd-shift-[",
    left: "cmd-shift-left",
    right: "cmd-shift-right",
    numbers: [
        "cmd-1", "cmd-2", "cmd-3", "cmd-4", "cmd-5", "cmd-6", "cmd-7", "cmd-8", "cmd-9",
    ],
};

/// Linux: the conventions of GNOME Terminal.
const LINUX_TAB_KEYS: TabKeys = TabKeys {
    new: "ctrl-shift-t",
    close: "ctrl-shift-w",
    next: "ctrl-pagedown",
    previous: "ctrl-pageup",
    left: "ctrl-shift-pageup",
    right: "ctrl-shift-pagedown",
    numbers: [
        "alt-1", "alt-2", "alt-3", "alt-4", "alt-5", "alt-6", "alt-7", "alt-8", "alt-9",
    ],
};

/// Registers the key bindings of the client.
pub fn bind_keys(cx: &mut App) {
    let macos = cfg!(target_os = "macos");
    let (copy, paste) = if macos {
        ("cmd-c", "cmd-v")
    } else {
        ("ctrl-shift-c", "ctrl-shift-v")
    };
    let terminal = Some(terminal_tab::KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new(copy, Copy, terminal),
        KeyBinding::new(paste, Paste, terminal),
    ]);
    let keys = if macos {
        MACOS_TAB_KEYS
    } else {
        LINUX_TAB_KEYS
    };
    let [one, two, three, four, five, six, seven, eight, last] = keys.numbers;
    let context = Some(workbench::KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new(keys.new, NewSession, context),
        KeyBinding::new(keys.close, CloseTab, context),
        KeyBinding::new(keys.next, NextTab, context),
        KeyBinding::new(keys.previous, PreviousTab, context),
        KeyBinding::new(keys.left, MoveTabLeft, context),
        KeyBinding::new(keys.right, MoveTabRight, context),
        KeyBinding::new(one, ActivateTab1, context),
        KeyBinding::new(two, ActivateTab2, context),
        KeyBinding::new(three, ActivateTab3, context),
        KeyBinding::new(four, ActivateTab4, context),
        KeyBinding::new(five, ActivateTab5, context),
        KeyBinding::new(six, ActivateTab6, context),
        KeyBinding::new(seven, ActivateTab7, context),
        KeyBinding::new(eight, ActivateTab8, context),
        KeyBinding::new(last, ActivateLastTab, context),
    ]);
}

/// Starts the connection to the daemon and opens the window with
/// `window_size` in logical pixels.
///
/// # Errors
///
/// Returns an error if the connection thread cannot start or the window
/// cannot open.
pub fn open_window(
    appearance: Appearance,
    window_size: (f32, f32),
    measurement: Option<Measurement>,
    cx: &mut App,
) -> gpui::Result<WindowHandle<Workbench>> {
    let (connection, updates) = HostConnection::start()?;
    let bounds = gpui::Bounds::centered(None, size(px(window_size.0), px(window_size.1)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("nomux".into()),
            ..TitlebarOptions::default()
        }),
        app_id: Some("sh.nomux.desktop".to_owned()),
        ..WindowOptions::default()
    };
    cx.open_window(options, |window, cx| {
        cx.new(|cx| Workbench::new(connection, updates, appearance, measurement, window, cx))
    })
}
