//! The content of the sidebar: the header and one row for each session of
//! the shown Project.
//!
//! The protocol has no worktrees yet. Thus the rows are the Sessions of the
//! Project, at level 1 of the tree of the specification, without a worktree
//! row above them.

use std::path::Path;

use nomux_client::{AgentRun, Session, SessionState, Sourced, StatusSource};

use crate::age::format_age;
use crate::glyph_state::GlyphState;
use crate::host_state::HostState;
use crate::selection::{ProjectView, Selection};
use crate::tabs::Tabs;

/// The header and the rows of the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarContent {
    /// The name of the Project, or the name of the sessions without one.
    pub title: String,
    /// The number of rows that need the user.
    pub needs_you: usize,
    /// The rows: the sessions that need the user first.
    pub rows: Vec<SessionRow>,
}

/// What runs in a session: the row has a different name font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    /// An agent tool reports its status.
    Agent,
    /// A shell or a program without a status report.
    Shell,
}

/// One Session row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRow {
    /// The session identifier.
    pub session_id: String,
    /// What runs in the session.
    pub kind: SessionKind,
    /// The glyph in the slot.
    pub state: GlyphState,
    /// The name.
    pub name: String,
    /// The secondary text, after the name.
    pub secondary: Option<String>,
    /// The most urgent state of the Child work and its number, for the tail
    /// of a row whose children are closed.
    pub children: Option<(GlyphState, usize)>,
    /// The age, at the end of the tail.
    pub age: Option<String>,
    /// `true` for the selected row.
    pub selected: bool,
    /// `true` if the session has an open tab.
    pub has_tab: bool,
}

impl SessionRow {
    /// `true` if the row shows the working glyph: the glyph turns.
    #[must_use]
    pub fn animates(&self) -> bool {
        self.state == GlyphState::Working
            || self
                .children
                .is_some_and(|(state, _)| state == GlyphState::Working)
    }
}

/// The title of the sidebar when it shows the sessions without a Project.
pub const UNASSIGNED_TITLE: &str = "No project";

/// The sidebar of the selected Project. `None` if nothing is selected.
/// `now_ms` is the current time, for the ages, and `home` the home directory
/// of the user, which a row shows as `~`. `tabs` tells which rows have an
/// open tab.
#[must_use]
pub fn sidebar(
    host: &HostState,
    selection: &Selection,
    tabs: &Tabs,
    now_ms: u64,
    home: Option<&Path>,
) -> Option<SidebarContent> {
    let view = selection.project()?;
    let title = match view {
        ProjectView::Project(id) => host.workspace().tree.get(id)?.name.clone(),
        ProjectView::Unassigned => UNASSIGNED_TITLE.to_owned(),
    };
    let rows: Vec<SessionRow> = ordered_sessions(host, view)
        .into_iter()
        .map(|session| {
            let context = RowContext {
                now_ms,
                home,
                selection,
                tabs,
            };
            session_row(session, host.run(&session.id), &context)
        })
        .collect();
    let needs_you = rows
        .iter()
        .filter(|row| row.state == GlyphState::NeedsYou)
        .count();
    Some(SidebarContent {
        title,
        needs_you,
        rows,
    })
}

/// The sessions of `view` in the sequence of the rows: the sessions that
/// need the user first, the others in their sequence of creation. The
/// daemon lists the sessions by identifier, thus the start time gives the
/// sequence; a session without a start time comes last.
#[must_use]
pub fn ordered_sessions<'a>(host: &'a HostState, view: &'a ProjectView) -> Vec<&'a Session> {
    let mut sessions: Vec<&Session> = host.sessions_of(view).collect();
    sessions.sort_by_key(|session| {
        let needs_you =
            GlyphState::of_session(session, host.run(&session.id)) == GlyphState::NeedsYou;
        (!needs_you, session.started_at.unwrap_or(u64::MAX))
    });
    sessions
}

struct RowContext<'a> {
    now_ms: u64,
    home: Option<&'a Path>,
    selection: &'a Selection,
    tabs: &'a Tabs,
}

fn session_row(session: &Session, run: Option<&AgentRun>, context: &RowContext<'_>) -> SessionRow {
    let kind = if run.is_some_and(is_agent) {
        SessionKind::Agent
    } else {
        SessionKind::Shell
    };
    let reported = |field: fn(&AgentRun) -> Option<&Sourced<String>>| {
        run.and_then(field).map(|sourced| sourced.value.clone())
    };
    let name = display_name(session, run, context.home);
    let secondary = if session.state == SessionState::Dormant {
        Some("dormant · restores on open".to_owned())
    } else {
        run.and_then(|run| {
            run.status
                .message
                .as_ref()
                .map(|message| message.value.clone())
        })
        .or_else(|| match kind {
            SessionKind::Shell => Some(
                reported(|run| run.status.tool.as_ref()).unwrap_or_else(|| program_name(session)),
            ),
            SessionKind::Agent => None,
        })
    };
    let children = run.filter(|run| !run.children.is_empty()).map(|run| {
        let states = run
            .children
            .iter()
            .map(|child| GlyphState::of_status(&child.aggregate));
        let most_urgent = GlyphState::most_urgent(states).unwrap_or(GlyphState::Idle);
        (most_urgent, run.children.len())
    });
    SessionRow {
        session_id: session.id.clone(),
        kind,
        state: GlyphState::of_session(session, run),
        name,
        secondary,
        children,
        age: session
            .started_at
            .map(|started_at| format_age(started_at.saturating_mul(1000), context.now_ms)),
        selected: context.selection.session() == Some(session.id.as_str()),
        has_tab: context.tabs.sessions().contains(&session.id),
    }
}

/// The name that a row shows: the title of an agent, else the name of the
/// session, else the name of its working directory, else its program.
fn display_name(session: &Session, run: Option<&AgentRun>, home: Option<&Path>) -> String {
    let title = run
        .filter(|run| is_agent(run))
        .and_then(|run| run.status.title.as_ref().or(run.status.tool.as_ref()))
        .map(|sourced| sourced.value.clone());
    title.unwrap_or_else(|| session_name(session, home))
}

/// The name that the row of the session `id` shows, for a question about
/// the session.
#[must_use]
pub fn session_label(host: &HostState, id: &str, home: Option<&Path>) -> Option<String> {
    let session = host.session(id)?;
    Some(display_name(session, host.run(id), home))
}

/// An agent reports its status. A guess from the shell or a session
/// without a source is a shell.
fn is_agent(run: &AgentRun) -> bool {
    run.details.is_some()
        || matches!(
            run.status.source,
            StatusSource::Adapter | StatusSource::ProgramStatus | StatusSource::SelfReport
        )
}

/// The name of the session, else the name of its working directory (`~` for
/// the home directory), else the file name of its program.
pub fn session_name(session: &Session, home: Option<&Path>) -> String {
    session
        .name
        .clone()
        .or_else(|| directory_name(session, home))
        .unwrap_or_else(|| program_name(session))
}

/// The last component of the working directory: `~` for the home directory
/// and `/` for the root.
fn directory_name(session: &Session, home: Option<&Path>) -> Option<String> {
    let directory = session
        .resolved_working_directory
        .as_deref()
        .or(session.working_directory.as_deref())?;
    if home == Some(directory) {
        return Some("~".to_owned());
    }
    Some(directory.file_name().map_or_else(
        || directory.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    ))
}

/// The file name of the program of the session.
fn program_name(session: &Session) -> String {
    session
        .command
        .first()
        .and_then(|program| Path::new(program).file_name())
        .map_or_else(
            || "shell".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        )
}
