//! One attach to the terminal of a session, on the connection thread: the
//! output feeds the terminal state, and the input of the tab goes to the
//! daemon.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use nomux_client::{AttachEnd, AttachOptions, Host, TerminalEvent, WindowSize};
use nomux_term::{Size, Terminal};
use nomux_term_view::TerminalSnapshot;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::host_connection::KIND;

/// The rows that a terminal keeps above its screen. A row uses memory only
/// after it scrolled out of the screen.
const HISTORY_ROWS: usize = 5000;

/// The terminal state of an attach, shared by the connection thread, which
/// feeds it, and the UI thread, which takes a snapshot once for each frame.
#[derive(Debug)]
pub struct SharedScreen {
    terminal: Mutex<Terminal>,
    /// `true` after a change that the UI did not take yet. The connection
    /// thread wakes the UI only when it sets this flag, thus the UI gets at
    /// most one wake for each frame.
    changed: AtomicBool,
}

impl SharedScreen {
    /// A blank screen of `size`.
    #[must_use]
    pub fn new(size: Size) -> Self {
        Self {
            terminal: Mutex::new(Terminal::with_history(size, HISTORY_ROWS)),
            changed: AtomicBool::new(false),
        }
    }

    /// A snapshot of the view `scrolled` rows above the screen, if the
    /// screen changed since the last one or if `always`.
    pub fn take_snapshot(&self, scrolled: usize, always: bool) -> Option<TerminalSnapshot> {
        let changed = self.changed.swap(false, Ordering::AcqRel);
        (changed || always).then(|| TerminalSnapshot::capture(&self.terminal().screen(), scrolled))
    }

    fn terminal(&self) -> MutexGuard<'_, Terminal> {
        // A panic while the lock was held leaves a terminal that can still
        // be read and fed.
        self.terminal.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Applies bytes. Returns `true` if the UI must be woken.
    fn feed(&self, bytes: &[u8]) -> bool {
        self.terminal().feed(bytes, |_| {});
        !self.changed.swap(true, Ordering::AcqRel)
    }

    /// Puts the terminal into its initial state, before a snapshot.
    fn reset(&self) {
        let mut terminal = self.terminal();
        let size = terminal.screen().size();
        *terminal = Terminal::with_history(size, HISTORY_ROWS);
    }

    fn resize(&self, size: Size) -> bool {
        self.terminal().resize(size);
        !self.changed.swap(true, Ordering::AcqRel)
    }
}

/// What the tab sends to its attach.
#[derive(Debug)]
pub enum TabInput {
    /// Bytes for the program, as the keyboard makes them.
    Bytes(Vec<u8>),
    /// The new size of the tab, in cells.
    Resize(Size),
}

/// What the attach tells its tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabUpdate {
    /// The screen changed: take a snapshot at the next frame.
    Changed,
    /// The connection to the daemon ended; the attach connects again.
    ConnectionLost,
    /// The attach has a connection again.
    Reconnected,
    /// The attach ended.
    Ended(AttachEnd),
    /// The attach failed, in English for the user.
    Failed(String),
}

/// The request of a tab to attach to a session.
#[derive(Debug)]
pub struct AttachRequest {
    /// The session.
    pub session_id: String,
    /// The size of the tab. The session gets it.
    pub size: Size,
    /// The screen that the output feeds.
    pub screen: Arc<SharedScreen>,
    /// The input of the tab. When the tab drops its sender, the attach
    /// ends: that is the detach. The session continues.
    pub input: UnboundedReceiver<TabInput>,
    /// The updates for the tab.
    pub updates: UnboundedSender<TabUpdate>,
}

/// Runs one attach until it ends or the tab goes away.
pub(crate) async fn run(host: Host, request: AttachRequest) {
    let AttachRequest {
        session_id,
        size,
        screen,
        mut input,
        updates,
    } = request;
    let options = AttachOptions {
        size: Some(window_size(size)),
        resume: None,
    };
    let fail = |error: nomux_client::ClientError| {
        let _ = updates.send(TabUpdate::Failed(error.to_string()));
    };
    let (mut output, mut terminal_input) = match host.attach(KIND, &session_id, options).await {
        Ok(attached) => attached,
        Err(error) => return fail(error),
    };
    // The window size goes on a second connection, as the CLI does.
    let mut control = match host.client(KIND).await {
        Ok(control) => control,
        Err(error) => return fail(error),
    };
    let show_output = async {
        loop {
            let event = match output.next().await {
                Ok(event) => event,
                Err(error) => return fail(error),
            };
            let wake = match event {
                TerminalEvent::Snapshot { bytes, starts } => {
                    if starts {
                        screen.reset();
                    }
                    screen.feed(bytes)
                }
                TerminalEvent::Output(bytes) => screen.feed(bytes),
                // A snapshot follows; this version shows no mark for a restore.
                TerminalEvent::HistoryGap | TerminalEvent::Restored => false,
                TerminalEvent::ConnectionLost => {
                    let _ = updates.send(TabUpdate::ConnectionLost);
                    false
                }
                TerminalEvent::Reconnected => {
                    let _ = updates.send(TabUpdate::Reconnected);
                    false
                }
                TerminalEvent::End(end) => {
                    let _ = updates.send(TabUpdate::Ended(end));
                    return;
                }
            };
            if wake && updates.send(TabUpdate::Changed).is_err() {
                return;
            }
        }
    };
    let send_input = async {
        while let Some(message) = input.recv().await {
            let result = match message {
                TabInput::Bytes(bytes) => terminal_input.send(&bytes).await,
                TabInput::Resize(size) => {
                    if screen.resize(size) {
                        let _ = updates.send(TabUpdate::Changed);
                    }
                    control.resize_session(&session_id, window_size(size)).await
                }
            };
            if let Err(error) = result {
                return fail(error);
            }
        }
    };
    tokio::select! {
        () = show_output => {}
        () = send_input => {}
    }
}

fn window_size(size: Size) -> WindowSize {
    WindowSize {
        rows: size.rows(),
        columns: size.columns(),
    }
}
