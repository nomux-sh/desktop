//! Opens the window against a real daemon in a temporary HOME, attaches to
//! a session that printed a known text, and takes a screenshot with the
//! headless renderer (lavapipe on a computer without a GPU).
//!
//! The test needs the nomux binaries of the pinned revision on `PATH` and a
//! Vulkan driver. CI runs it; `README.md` shows how to run it locally.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{AppContext, HeadlessAppContext, px, size};
use nomux_desktop::{Appearance, HostConnection, HostRequest, HostUpdate, Workbench, fonts};

const TEXT: &str = "nomux-smoke-text";
/// The time without a change in which the window must ask for no frame.
const IDLE: Duration = Duration::from_secs(2);
/// The interval of the checks of the frame demand.
const POLL: Duration = Duration::from_millis(20);
/// Set in the child process that runs the test with a temporary HOME.
const IN_TEMPORARY_HOME: &str = "NOMUX_DESKTOP_SMOKE_HOME";

/// Runs the test `test_name` again in a child process whose HOME is a new
/// directory: the daemon, its sessions and its files stay in that directory.
fn run_in_temporary_home(test_name: &str) {
    let home = std::env::temp_dir().join(format!(
        "nomux-desktop-smoke-{}-{test_name}",
        std::process::id()
    ));
    let runtime = home.join("run");
    std::fs::create_dir_all(&runtime).expect("a temporary HOME");
    let status = Command::new(std::env::current_exe().expect("the test binary"))
        .args(["--exact", test_name, "--ignored", "--nocapture"])
        .env(IN_TEMPORARY_HOME, &home)
        .env("HOME", &home)
        .env("XDG_RUNTIME_DIR", &runtime)
        .status()
        .expect("the test binary starts");
    let _ = std::fs::remove_dir_all(&home);
    assert!(
        status.success(),
        "the smoke test failed in its temporary HOME"
    );
}

/// The number of checks in `duration` at which the window wanted a frame:
/// a view was dirty or a callback waited for the next frame. The session
/// prints nothing more and its cursor does not blink.
fn frames_requested_in_idle(cx: &mut HeadlessAppContext, duration: Duration) -> usize {
    cx.run_until_parked();
    let started = Instant::now();
    let mut requested = 0;
    while started.elapsed() < duration {
        cx.run_until_parked();
        // GPUI shows the frame demand of a window only in its diagnostic
        // state.
        let state = cx.update(|cx| cx.debug_state().to_string());
        if state.contains("\"dirty\":true") || !state.contains("\"next_frame_callbacks\":0") {
            requested += 1;
        }
        std::thread::sleep(POLL);
    }
    requested
}

fn nomux(arguments: &[&str]) {
    let status = Command::new("nomux")
        .args(arguments)
        .status()
        .expect("the nomux CLI is on PATH");
    assert!(status.success(), "nomux {arguments:?} failed");
}

#[test]
#[ignore = "needs the nomux binaries on PATH and a Vulkan driver"]
fn the_window_shows_the_screen_of_a_session() {
    if std::env::var_os(IN_TEMPORARY_HOME).is_none() {
        return run_in_temporary_home("the_window_shows_the_screen_of_a_session");
    }
    nomux(&[
        "new",
        "--",
        "sh",
        "-c",
        &format!("printf '{TEXT}\\n'; exec cat"),
    ]);

    let platform = gpui_platform::current_platform(true);
    let mut cx = HeadlessAppContext::with_platform(
        platform.text_system(),
        Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    cx.allow_parking();
    cx.update(|cx| fonts::load(cx).expect("the shipped fonts load"));
    let (connection, updates) = HostConnection::start().expect("the connection thread starts");
    let window = cx
        .open_window(size(px(1280.0), px(800.0)), |window, cx| {
            cx.new(|cx| Workbench::new(connection, updates, Appearance::Dark, None, window, cx))
        })
        .expect("the window opens");

    let started = Instant::now();
    let mut screenshot = None;
    while started.elapsed() < Duration::from_secs(20) {
        cx.run_until_parked();
        // Each capture draws a frame: the tab attaches after its first layout.
        screenshot = Some(cx.capture_screenshot(window.into()).expect("a screenshot"));
        let shown = cx.update(|cx| {
            window
                .read(cx)
                .ok()
                .and_then(|workbench| workbench.tab().cloned())
                .is_some_and(|tab| {
                    let tab = tab.read(cx);
                    let first_row: String = tab
                        .snapshot()
                        .row(0)
                        .iter()
                        .map(|cell| cell.character)
                        .collect();
                    first_row.starts_with(TEXT)
                })
        });
        if shown {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let screenshot = screenshot.expect("at least one frame");
    let frames = frames_requested_in_idle(&mut cx, IDLE);
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    screenshot
        .save(directory.join("smoke.png"))
        .expect("the screenshot is written");
    nomux(&["daemon", "stop"]);

    assert!(
        started.elapsed() < Duration::from_secs(20),
        "the tab did not show the text of the session"
    );
    assert_eq!(
        frames, 0,
        "the window asked for frames without a change in {IDLE:?}"
    );
    // The rail, the sidebar and the terminal have different tones: a blank
    // frame has one colour.
    let first = screenshot.get_pixel(0, 0);
    assert!(screenshot.pixels().any(|pixel| pixel != first));
}

/// The output of `nomux` with `arguments`.
fn nomux_output(arguments: &[&str]) -> String {
    let output = Command::new("nomux")
        .args(arguments)
        .output()
        .expect("the nomux CLI is on PATH");
    assert!(output.status.success(), "nomux {arguments:?} failed");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The first session of a `Sessions` update, waiting at most 20 seconds.
fn next_sessions(updates: &mut tokio::sync::mpsc::UnboundedReceiver<HostUpdate>) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        match updates.try_recv() {
            Ok(HostUpdate::Sessions(sessions)) => {
                return sessions.into_iter().map(|session| session.id).collect();
            }
            Ok(_) => {}
            Err(_) => std::thread::sleep(POLL),
        }
    }
    panic!("no list of the sessions in 20 seconds");
}

#[test]
#[ignore = "needs the nomux binaries on PATH"]
fn a_replaced_daemon_keeps_the_sessions_and_their_processes() {
    if std::env::var_os(IN_TEMPORARY_HOME).is_none() {
        return run_in_temporary_home("a_replaced_daemon_keeps_the_sessions_and_their_processes");
    }
    nomux(&["new", "--", "sh", "-c", "exec cat"]);
    let before = nomux_output(&["ls"]);
    let (connection, mut updates) = HostConnection::start().expect("the connection thread starts");
    let first = next_sessions(&mut updates);
    assert_eq!(first.len(), 1);

    connection.send(HostRequest::ReplaceDaemon);
    let second = next_sessions(&mut updates);
    let after = nomux_output(&["ls"]);
    nomux(&["daemon", "stop"]);

    assert_eq!(second, first, "the client lists the same session again");
    // `nomux ls` shows the process identifier of the session: the process
    // of the session did not end with the daemon.
    assert_eq!(after, before);
}
