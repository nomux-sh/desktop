//! The reduced motion setting of the system. GPUI does not read it: the
//! client reads it one time at the start, on a background thread, and the
//! window has motion until the answer arrives.
//!
//! - macOS: `defaults read com.apple.universalaccess reduceMotion` (`1`).
//! - Linux: `gsettings get org.gnome.desktop.interface enable-animations`
//!   (`false`).

use std::process::{Command, Stdio};

use gpui::{App, WindowHandle};

use crate::workbench::Workbench;

/// Reads the setting on a background thread and gives it to the window.
pub fn follow_system(window: WindowHandle<Workbench>, cx: &mut App) {
    let reading = cx.background_executor().spawn(async { read() });
    cx.spawn(async move |cx| {
        let Some(reduce) = reading.await else {
            return;
        };
        cx.update(|cx| cx.set_reduce_motion(reduce));
        let _ = window.update(cx, |workbench, window, cx| {
            workbench.motion_changed(window, cx);
        });
    })
    .detach();
}

/// `Some(true)` if the user asked for reduced motion, `None` if the setting
/// cannot be read.
fn read() -> Option<bool> {
    let (program, arguments, reduce_value): (&str, &[&str], &str) = if cfg!(target_os = "macos") {
        (
            "defaults",
            &["read", "com.apple.universalaccess", "reduceMotion"],
            "1",
        )
    } else {
        (
            "gsettings",
            &["get", "org.gnome.desktop.interface", "enable-animations"],
            "false",
        )
    };
    let output = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim() == reduce_value)
}
