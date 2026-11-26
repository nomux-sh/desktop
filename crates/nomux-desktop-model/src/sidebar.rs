//! The content of the sidebar: the header and one row for each session of
//! the shown Project.
//!
//! The protocol has no worktrees yet. Thus the rows are the Sessions of the
//! Project, at level 1 of the tree of the specification, without a worktree
//! row above them.

use std::path::Path;

use nomux_client::{AgentRun, NodeKind, Session, SessionState, StatusSource};

use crate::age::format_age;
use crate::glyph_state::GlyphState;
use crate::host_state::HostState;
use crate::selection::{ProjectView, Selection};

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
/// `now_ms` is the current time and `home` the home directory of the user,
/// for the ages and the directories.
#[must_use]
pub fn sidebar(
    host: &HostState,
    selection: &Selection,
    now_ms: u64,
    home: Option<&Path>,
) -> Option<SidebarContent> {
    let view = selection.project()?;
    let (title, project_directory) = match view {
        ProjectView::Project(id) => {
            let node = host.workspace().tree.get(id)?;
            let directory = match &node.kind {
                NodeKind::Project { directory } => Some(directory.as_path()),
                NodeKind::Group => None,
            };
            (node.name.clone(), directory)
        }
        ProjectView::Unassigned => (UNASSIGNED_TITLE.to_owned(), None),
    };
    let rows: Vec<SessionRow> = ordered_sessions(host, view)
        .into_iter()
        .map(|session| {
            let directory = DirectoryContext {
                project: project_directory,
                home,
            };
            session_row(session, host.run(&session.id), directory, now_ms, selection)
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

#[derive(Clone, Copy)]
struct DirectoryContext<'a> {
    project: Option<&'a Path>,
    home: Option<&'a Path>,
}

fn session_row(
    session: &Session,
    run: Option<&AgentRun>,
    directory: DirectoryContext<'_>,
    now_ms: u64,
    selection: &Selection,
) -> SessionRow {
    let kind = if run.is_some_and(is_agent) {
        SessionKind::Agent
    } else {
        SessionKind::Shell
    };
    let title = run.and_then(|run| run.status.title.as_ref().map(|title| title.value.clone()));
    let name = match kind {
        SessionKind::Agent => title.unwrap_or_else(|| session_name(session)),
        SessionKind::Shell => session_name(session),
    };
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
            SessionKind::Shell => short_directory(session, directory),
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
            .map(|started_at| format_age(started_at.saturating_mul(1000), now_ms)),
        selected: selection.session() == Some(session.id.as_str()),
    }
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

/// The name of the session, else the file name of its program.
pub(crate) fn session_name(session: &Session) -> String {
    session
        .name
        .clone()
        .unwrap_or_else(|| program_name(session))
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

/// The working directory below the Project, or below the home directory as
/// `~/…`. `None` when it is the directory of the Project.
fn short_directory(session: &Session, directory: DirectoryContext<'_>) -> Option<String> {
    let working = session
        .resolved_working_directory
        .as_deref()
        .or(session.working_directory.as_deref())?;
    if let Some(project) = directory.project
        && let Ok(below) = working.strip_prefix(project)
    {
        return (!below.as_os_str().is_empty()).then(|| below.display().to_string());
    }
    if let Some(home) = directory.home
        && let Ok(below) = working.strip_prefix(home)
    {
        return Some(if below.as_os_str().is_empty() {
            "~".to_owned()
        } else {
            format!("~/{}", below.display())
        });
    }
    Some(working.display().to_string())
}
