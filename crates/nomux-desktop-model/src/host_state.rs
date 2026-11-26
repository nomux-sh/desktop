//! The copy of the client of the state of a host.

use std::collections::HashMap;

use nomux_client::{
    AgentRun, AgentTree, Applied, EventBody, HostEvent, Session, SessionState, Workspace,
};

use crate::selection::ProjectView;

/// What the client must load again from the daemon.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Reload {
    /// The list of the sessions.
    pub sessions: bool,
    /// The workspace tree.
    pub workspace: bool,
    /// The Agent run of each session.
    pub agent_tree: bool,
}

impl Reload {
    /// Everything.
    pub const ALL: Self = Self {
        sessions: true,
        workspace: true,
        agent_tree: true,
    };

    /// `true` if nothing must be loaded.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self == Self::default()
    }
}

/// The workspace tree, the sessions and the Agent runs of one host.
#[derive(Debug, Clone, Default)]
pub struct HostState {
    workspace: Workspace,
    sessions: Vec<Session>,
    runs: HashMap<String, AgentRun>,
}

impl HostState {
    /// Takes the workspace tree that the daemon sent.
    pub fn set_workspace(&mut self, workspace: Workspace) {
        self.workspace = workspace;
    }

    /// Takes the list of the sessions that the daemon sent.
    pub fn set_sessions(&mut self, sessions: Vec<Session>) {
        self.sessions = sessions;
    }

    /// Takes the Agent runs that the daemon sent.
    pub fn set_agent_tree(&mut self, tree: AgentTree) {
        self.runs = tree
            .runs
            .into_iter()
            .map(|run| (run.session_id.clone(), run))
            .collect();
    }

    /// Applies one event of the host and returns what the client must load
    /// again.
    ///
    /// The event stream has no event for a new or an ended session
    /// (nomux-sh/proto#23). Thus the list of the sessions is loaded again
    /// when an Agent run of an unknown session comes and when an Agent run
    /// goes: a new session gets a run, and an ended session loses it.
    pub fn apply_event(&mut self, event: &HostEvent) -> Reload {
        match &event.body {
            EventBody::AgentRunChanged(run) => {
                let known = self.session(&run.session_id).is_some();
                self.runs.insert(run.session_id.clone(), (**run).clone());
                Reload {
                    sessions: !known,
                    ..Reload::default()
                }
            }
            EventBody::AgentRunRemoved { session_id } => {
                self.runs.remove(session_id);
                Reload {
                    sessions: true,
                    ..Reload::default()
                }
            }
            EventBody::WorkspaceChanged { revision, change } => {
                match self.workspace.apply(*revision, change) {
                    Applied::Changed | Applied::Included => Reload::default(),
                    Applied::Missed => Reload {
                        workspace: true,
                        ..Reload::default()
                    },
                }
            }
            EventBody::Snapshot => Reload::ALL,
            EventBody::ExtensionEvent { .. } | EventBody::Unknown => Reload::default(),
        }
    }

    /// The workspace tree.
    #[must_use]
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// The session `id` if the client shows it: a session whose command
    /// ended or that the daemon cannot read is not shown.
    #[must_use]
    pub fn session(&self, id: &str) -> Option<&Session> {
        self.sessions
            .iter()
            .find(|session| session.id == id && is_shown(session))
    }

    /// The Agent run of the session `id`.
    #[must_use]
    pub fn run(&self, session_id: &str) -> Option<&AgentRun> {
        self.runs.get(session_id)
    }

    /// The shown sessions of a Project, or of no Project, in the sequence of
    /// the daemon.
    pub fn sessions_of<'a>(&'a self, view: &'a ProjectView) -> impl Iterator<Item = &'a Session> {
        self.sessions.iter().filter(move |session| {
            is_shown(session) && self.project_of(session) == view.project_id()
        })
    }

    /// The identifier of the Project of a session.
    #[must_use]
    pub fn project_of(&self, session: &Session) -> Option<&str> {
        session
            .project(&self.workspace.tree)
            .map(|node| node.id.as_str())
    }

    /// `true` if a shown session has no Project.
    #[must_use]
    pub fn has_unassigned_sessions(&self) -> bool {
        self.sessions_of(&ProjectView::Unassigned).next().is_some()
    }
}

fn is_shown(session: &Session) -> bool {
    !matches!(session.state, SessionState::Ended | SessionState::Damaged)
}
