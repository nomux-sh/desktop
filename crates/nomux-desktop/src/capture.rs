//! The capture mode: the option `--capture <file>`, only in a build with the
//! feature `frame-capture`. The client waits until a row of the terminal tab
//! shows text, renders the frame of the window to an image, writes it as PNG and
//! ends.
//!
//! GPUI renders a window to an image only with its `test-support` feature,
//! thus a release package never has this mode.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{AsyncWindowContext, Entity};

use crate::terminal_tab::TerminalTab;

/// The longest wait for the text of a session.
const TIMEOUT: Duration = Duration::from_secs(20);
/// The time after the first text, for the frames that follow it.
const SETTLE: Duration = Duration::from_secs(1);

/// Waits for the text of `tab`, writes the frame to `file` and ends the
/// application.
pub async fn run(
    file: PathBuf,
    tab: impl Fn(&mut AsyncWindowContext) -> Option<Entity<TerminalTab>>,
    cx: &mut AsyncWindowContext,
) {
    let started = Instant::now();
    while started.elapsed() < TIMEOUT {
        let shown = tab(cx).is_some_and(|tab| {
            tab.read_with(cx, |tab, _| {
                let snapshot = tab.snapshot();
                (0..snapshot.screen_rows())
                    .any(|row| snapshot.row(row).iter().any(|cell| cell.character != ' '))
            })
        });
        if shown {
            break;
        }
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
    }
    cx.background_executor().timer(SETTLE).await;
    let image = cx.update(|window, _| window.render_to_image());
    match image {
        Ok(Ok(image)) => {
            if let Err(error) = image.save(&file) {
                eprintln!("nomux-desktop: cannot write {}: {error}", file.display());
            }
        }
        Ok(Err(error)) => eprintln!("nomux-desktop: the frame cannot be rendered: {error}"),
        Err(error) => eprintln!("nomux-desktop: the window is gone: {error}"),
    }
    let _ = cx.update(|_, cx| cx.quit());
}
