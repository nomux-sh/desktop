//! The sessions that need the user, across all Projects: the order of the
//! attention list and the jump to the next session that needs the user.

use crate::glyph_state::GlyphState;
use crate::host_state::HostState;

/// One session of the attention list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attention {
    /// The session identifier.
    pub session_id: String,
    /// The state of the session: needs you, done and not seen, or working.
    pub state: GlyphState,
    /// When the daemon saw the state for the first time: milliseconds after
    /// the Unix epoch.
    pub since_ms: u64,
}

/// The place of a state in the list, `None` for a state that is not in it.
fn rank(state: GlyphState) -> Option<u8> {
    match state {
        GlyphState::NeedsYou => Some(0),
        GlyphState::DoneUnseen => Some(1),
        GlyphState::Working => Some(2),
        GlyphState::Error | GlyphState::Idle | GlyphState::NoProcess => None,
    }
}

/// The attention list: the sessions that need the user, then the sessions
/// that are done and not seen, then the sessions that work. In each group
/// the state that is oldest comes first: the one that waits longest. The
/// identifier breaks a tie, thus the order does not depend on the input.
/// A session in another state is not in the list.
#[must_use]
pub fn attention_order(mut items: Vec<Attention>) -> Vec<Attention> {
    items.retain(|item| rank(item.state).is_some());
    items.sort_by(|a, b| {
        (rank(a.state), a.since_ms, &a.session_id).cmp(&(rank(b.state), b.since_ms, &b.session_id))
    });
    items
}

/// The attention list of all shown sessions of the host.
#[must_use]
pub fn attention(host: &HostState) -> Vec<Attention> {
    let items = host
        .shown_sessions()
        .filter_map(|session| {
            let run = host.run(&session.id)?;
            Some(Attention {
                session_id: session.id.clone(),
                state: GlyphState::of_session(session, Some(run)),
                since_ms: run.status.since_ms,
            })
        })
        .collect();
    attention_order(items)
}

/// The session that the jump key goes to: the session that needs the user
/// after `current` in the list, and the first one when `current` is not one
/// of them. The sequence wraps around. `None` if no session needs the user.
#[must_use]
pub fn next_needing_you<'a>(list: &'a [Attention], current: Option<&str>) -> Option<&'a str> {
    let waiting: Vec<&Attention> = list
        .iter()
        .filter(|item| item.state == GlyphState::NeedsYou)
        .collect();
    let after = current
        .and_then(|id| waiting.iter().position(|item| item.session_id == id))
        .map_or(0, |index| (index + 1) % waiting.len());
    waiting.get(after).map(|item| item.session_id.as_str())
}
