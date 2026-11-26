//! The state that a state glyph shows.

use nomux_client::{AgentRun, AgentState, AgentStatus, Session, SessionState};

/// The state of a state glyph. The sequence of the variants is the
/// priority of the sidebar specification: the first one is the most urgent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GlyphState {
    /// The work waits for the user: the filled disc.
    NeedsYou,
    /// The work stopped with an error: the cross.
    Error,
    /// The work runs: the turning arc.
    Working,
    /// The work is done and the user did not see the result: the check.
    DoneUnseen,
    /// Nothing runs: the hollow ring.
    Idle,
    /// The session has no process: the dash.
    NoProcess,
}

impl GlyphState {
    /// The state of an Agent run or of a Child work.
    #[must_use]
    pub fn of_status(status: &AgentStatus) -> Self {
        match status.state {
            AgentState::NeedsYou(_) => Self::NeedsYou,
            AgentState::Error => Self::Error,
            AgentState::Working => Self::Working,
            AgentState::Done if !status.seen => Self::DoneUnseen,
            AgentState::Done | AgentState::Idle | AgentState::Unknown => Self::Idle,
        }
    }

    /// The state of a session: its own Agent run, or the dash when it has no
    /// process.
    #[must_use]
    pub fn of_session(session: &Session, run: Option<&AgentRun>) -> Self {
        if session.state == SessionState::Dormant {
            return Self::NoProcess;
        }
        run.map_or(Self::Idle, |run| Self::of_status(&run.status))
    }

    /// The most urgent state of `states`, `None` if there is none.
    pub fn most_urgent(states: impl IntoIterator<Item = Self>) -> Option<Self> {
        states.into_iter().min()
    }
}
