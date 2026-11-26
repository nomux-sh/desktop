//! The decision to show a system notification for an event of the host.
//!
//! The rules:
//!
//! - A notification is for a session that needs the user and for a result
//!   that is done and not seen. Only a status that a source reported counts:
//!   a guess from the prompt or from the foreground program, and the state
//!   that a session gets when it loses its process, do not.
//! - No notification while the window is active and shows the session.
//! - One notification for each session: the tag is the session identifier,
//!   thus a new one replaces the old one.
//! - No notification for a state that began before the floor: the start of
//!   the client, or a reconnect. The daemon sends an event for each change
//!   of the message too, and a state that was there at the start must not
//!   notify at such an event.
//! - No notification repeats for the same state of a session. The daemon
//!   sends an event for each change of the message or of the progress; the
//!   time at which the state began (`since_ms`) tells the events of one
//!   state from a new state.
//! - The user can turn off each kind.

use std::collections::HashMap;

use nomux_client::{AgentRun, AgentState, EventBody, HostEvent, NeedsYouKind, StatusSource};

use crate::host_state::HostState;
use crate::sidebar::session_name;

/// Why a notification is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoticeKind {
    /// The session waits for the user.
    NeedsYou,
    /// The session finished its work and the user did not see the result.
    Done,
}

/// Which kinds of notification are on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationSettings {
    /// A session needs you.
    pub needs_you: bool,
    /// A session is done.
    pub done: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            needs_you: true,
            done: true,
        }
    }
}

impl NotificationSettings {
    /// The settings of a text with one `key = value` for each line. The keys
    /// are `notify.needs-you` and `notify.done`, the values `on` and `off`.
    /// A blank line, a line that starts with `#`, an unknown key and a
    /// value that is not `on` or `off` change nothing.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = match value.trim() {
                "on" => true,
                "off" => false,
                _ => continue,
            };
            match key.trim() {
                "notify.needs-you" => settings.needs_you = value,
                "notify.done" => settings.done = value,
                _ => {}
            }
        }
        settings
    }

    fn enabled(self, kind: NoticeKind) -> bool {
        match kind {
            NoticeKind::NeedsYou => self.needs_you,
            NoticeKind::Done => self.done,
        }
    }
}

/// What the user looks at when the event comes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewing<'a> {
    /// `true` if a window of the client is the active window.
    pub window_active: bool,
    /// The session of the active tab.
    pub session: Option<&'a str>,
}

/// A notification to show. The session identifier is its tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The session. A click on the notification goes to it.
    pub session_id: String,
    /// Why the notification shows.
    pub kind: NoticeKind,
    /// The name of the session.
    pub title: String,
    /// What the session waits for or did.
    pub body: String,
}

/// Decides for each event of the host if a notification shows, and keeps
/// the last state of each session for that.
#[derive(Debug, Clone)]
pub struct Notifier {
    floor_ms: u64,
    settings: NotificationSettings,
    last: HashMap<String, (NoticeKind, u64)>,
}

impl Notifier {
    /// A notifier that ignores the states that began before `floor_ms`
    /// (milliseconds after the Unix epoch): the start of the client or the
    /// time of a reconnect.
    #[must_use]
    pub fn new(floor_ms: u64, settings: NotificationSettings) -> Self {
        Self {
            floor_ms,
            settings,
            last: HashMap::new(),
        }
    }

    /// The notification for `event`, if one shows. `host` is the state after
    /// the event.
    pub fn on_event(
        &mut self,
        event: &HostEvent,
        host: &HostState,
        viewing: &Viewing<'_>,
    ) -> Option<Notice> {
        match &event.body {
            EventBody::AgentRunRemoved { session_id } => {
                self.last.remove(session_id);
                None
            }
            EventBody::AgentRunChanged(run) => self.on_run(run, host, viewing),
            _ => None,
        }
    }

    fn on_run(
        &mut self,
        run: &AgentRun,
        host: &HostState,
        viewing: &Viewing<'_>,
    ) -> Option<Notice> {
        let Some(kind) = notice_kind(run) else {
            self.last.remove(&run.session_id);
            return None;
        };
        // An event that is not shown still counts: a later change of the
        // message in the same state is not a new event.
        let state = (kind, run.status.since_ms);
        if self.last.insert(run.session_id.clone(), state) == Some(state) {
            return None;
        }
        let looking = viewing.window_active && viewing.session == Some(run.session_id.as_str());
        if run.status.since_ms < self.floor_ms || !self.settings.enabled(kind) || looking {
            return None;
        }
        Some(Notice {
            session_id: run.session_id.clone(),
            kind,
            title: title(run, host),
            body: body(run, kind),
        })
    }
}

/// The kind of a run that can notify: a state that a source reported.
fn notice_kind(run: &AgentRun) -> Option<NoticeKind> {
    let reported = matches!(
        run.status.source,
        StatusSource::Adapter | StatusSource::ProgramStatus | StatusSource::SelfReport
    );
    if !reported {
        return None;
    }
    match run.status.state {
        AgentState::NeedsYou(_) => Some(NoticeKind::NeedsYou),
        AgentState::Done if !run.status.seen => Some(NoticeKind::Done),
        _ => None,
    }
}

/// The title of the agent, else the name of the session.
fn title(run: &AgentRun, host: &HostState) -> String {
    run.status
        .title
        .as_ref()
        .map(|title| title.value.clone())
        .or_else(|| {
            host.session(&run.session_id)
                .map(|session| session_name(session, None))
        })
        .unwrap_or_else(|| "Session".to_owned())
}

/// The message of the run, else a text for the state.
fn body(run: &AgentRun, kind: NoticeKind) -> String {
    if let Some(message) = &run.status.message {
        return message.value.clone();
    }
    match (kind, run.status.state) {
        (NoticeKind::NeedsYou, AgentState::NeedsYou(Some(NeedsYouKind::Permission))) => {
            "Waiting for a permission"
        }
        (NoticeKind::NeedsYou, AgentState::NeedsYou(Some(NeedsYouKind::Question))) => {
            "Has a question"
        }
        (NoticeKind::NeedsYou, AgentState::NeedsYou(Some(NeedsYouKind::Auth))) => "Needs a login",
        (NoticeKind::NeedsYou, _) => "Needs you",
        (NoticeKind::Done, _) => "Done",
    }
    .to_owned()
}
