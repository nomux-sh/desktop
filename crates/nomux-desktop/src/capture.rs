//! The capture mode: the option `--capture <file>`, only in a build with the
//! feature `frame-capture`. The client waits until a row of the terminal tab
//! shows text, renders the frame of the window to an image, writes it as PNG and
//! ends.
//!
//! GPUI renders a window to an image only with its `test-support` feature,
//! thus a release package never has this mode.
//!
//! The option `--capture-menu session|tab|project` opens the context menu of
//! the first row, the active tab or the first Project tile before the frame,
//! because a screenshot of the screen is not possible without a permission.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gpui::{AsyncWindowContext, Entity};

use crate::terminal_tab::TerminalTab;

/// The context menu that the frame shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMenu {
    /// The menu of the selected Session row.
    Session,
    /// The menu of the active tab.
    Tab,
    /// The menu of the tile of the shown Project.
    Project,
}

impl CaptureMenu {
    /// The menu that a word of the command line names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "session" => Some(Self::Session),
            "tab" => Some(Self::Tab),
            "project" => Some(Self::Project),
            _ => None,
        }
    }
}

/// The longest wait for the text of a session.
const TIMEOUT: Duration = Duration::from_secs(20);
/// The time after the first text, for the frames that follow it.
const SETTLE: Duration = Duration::from_secs(1);

/// Waits for the text of `tab`, calls `before_frame`, writes the frame to
/// `file` and ends the application.
pub async fn run(
    file: PathBuf,
    tab: impl Fn(&mut AsyncWindowContext) -> Option<Entity<TerminalTab>>,
    before_frame: impl FnOnce(&mut AsyncWindowContext),
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
    before_frame(cx);
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
