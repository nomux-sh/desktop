//! The terminal tabs of the main area: which sessions are open, in which
//! sequence, and which one is active. Each tab shows one session; a session
//! is in one tab at most. Closing a tab detaches: the session continues.
//!
//! The state of the tabs is saved as text, one session for each line in the
//! sequence of the tabs, the active one with the prefix `* `.

use crate::glyph_state::GlyphState;
use crate::host_state::HostState;

/// The open tabs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tabs {
    sessions: Vec<String>,
    active: Option<usize>,
}

/// One tab of the tab bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabItem {
    /// The session of the tab.
    pub session_id: String,
    /// The title: the name of the session, else the name of its program.
    pub title: String,
    /// The glyph of the state of the session.
    pub state: GlyphState,
    /// `true` for the active tab.
    pub active: bool,
}

impl TabItem {
    /// `true` if the tab shows the working glyph: the glyph turns.
    #[must_use]
    pub fn animates(&self) -> bool {
        self.state == GlyphState::Working
    }
}

impl Tabs {
    /// The sessions of the tabs, in their sequence.
    #[must_use]
    pub fn sessions(&self) -> &[String] {
        &self.sessions
    }

    /// The session of the active tab.
    #[must_use]
    pub fn active(&self) -> Option<&str> {
        self.active
            .and_then(|index| self.sessions.get(index))
            .map(String::as_str)
    }

    /// `true` if no tab is open.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    fn position(&self, session_id: &str) -> Option<usize> {
        self.sessions.iter().position(|id| id == session_id)
    }

    /// Shows a session, as a click on its row does: activates its tab if it
    /// has one, else the active tab shows it. Returns the session that the
    /// active tab showed before and does not show any more (a detach).
    pub fn show(&mut self, session_id: &str) -> Option<String> {
        if let Some(index) = self.position(session_id) {
            self.active = Some(index);
            return None;
        }
        match self.active {
            Some(index) => Some(std::mem::replace(
                &mut self.sessions[index],
                session_id.to_owned(),
            )),
            None => {
                self.open(session_id);
                None
            }
        }
    }

    /// Opens a session in a new tab after the active one and activates it.
    /// A session that has a tab is activated.
    pub fn open(&mut self, session_id: &str) {
        if let Some(index) = self.position(session_id) {
            self.active = Some(index);
            return;
        }
        let index = self.active.map_or(self.sessions.len(), |active| active + 1);
        self.sessions.insert(index, session_id.to_owned());
        self.active = Some(index);
    }

    /// Closes the active tab. The tab at its right becomes active, else the
    /// one at its left. Returns the session of the closed tab.
    pub fn close_active(&mut self) -> Option<String> {
        let index = self.active?;
        let closed = self.sessions.remove(index);
        self.active = if self.sessions.is_empty() {
            None
        } else {
            Some(index.min(self.sessions.len() - 1))
        };
        Some(closed)
    }

    /// Closes the tab of `session_id`. If it was the active tab, its right
    /// neighbour becomes active, else its left one; another active tab stays
    /// active. Returns `false` if the session has no tab.
    pub fn close(&mut self, session_id: &str) -> bool {
        let Some(index) = self.position(session_id) else {
            return false;
        };
        let active = self.active().map(str::to_owned);
        if active.as_deref() == Some(session_id) {
            self.close_active();
            return true;
        }
        self.sessions.remove(index);
        self.active = active.and_then(|id| self.position(&id));
        true
    }

    /// Activates the tab at `index` (0 is the first). An index after the
    /// last tab does nothing.
    pub fn activate(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.active = Some(index);
        }
    }

    /// Activates the last tab.
    pub fn activate_last(&mut self) {
        self.active = self.sessions.len().checked_sub(1);
    }

    /// Activates the next tab (`step` 1) or the previous tab (`step` -1).
    /// The sequence wraps around.
    pub fn activate_next(&mut self, step: isize) {
        let (Some(active), Ok(count)) = (self.active, isize::try_from(self.sessions.len())) else {
            return;
        };
        let active = isize::try_from(active).unwrap_or(0);
        self.active = usize::try_from((active + step).rem_euclid(count)).ok();
    }

    /// Moves the active tab one place to the right (`step` 1) or to the
    /// left (`step` -1). The first and the last tab stay at the edge.
    pub fn move_active(&mut self, step: isize) {
        let Some(active) = self.active else {
            return;
        };
        let Some(target) = active
            .checked_add_signed(step)
            .filter(|target| *target < self.sessions.len())
        else {
            return;
        };
        self.sessions.swap(active, target);
        self.active = Some(target);
    }

    /// Closes the tabs of the sessions that the host does not have any more.
    /// The active tab stays active if it stays open, else its next neighbour
    /// becomes active.
    pub fn retain_existing(&mut self, host: &HostState) {
        let active = self.active().map(str::to_owned);
        let before = self.active.unwrap_or(0);
        self.sessions.retain(|id| host.session(id).is_some());
        self.active = match active.and_then(|id| self.position(&id)) {
            Some(index) => Some(index),
            None if self.sessions.is_empty() => None,
            None => Some(before.min(self.sessions.len() - 1)),
        };
    }

    /// The items of the tab bar.
    #[must_use]
    pub fn items(&self, host: &HostState) -> Vec<TabItem> {
        self.sessions
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let session = host.session(id);
                TabItem {
                    session_id: id.clone(),
                    title: session.map_or_else(|| id.clone(), crate::sidebar::session_name),
                    state: session.map_or(GlyphState::NoProcess, |session| {
                        GlyphState::of_session(session, host.run(id))
                    }),
                    active: self.active == Some(index),
                }
            })
            .collect()
    }

    /// The saved state: one session for each line, the active one with the
    /// prefix `* `.
    #[must_use]
    pub fn to_text(&self) -> String {
        self.sessions
            .iter()
            .enumerate()
            .map(|(index, id)| {
                if self.active == Some(index) {
                    format!("* {id}\n")
                } else {
                    format!("{id}\n")
                }
            })
            .collect()
    }

    /// The tabs of a saved state. A line that is not a session identifier
    /// is ignored.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let mut tabs = Self::default();
        for line in text.lines() {
            let (active, id) = match line.strip_prefix("* ") {
                Some(id) => (true, id.trim()),
                None => (false, line.trim()),
            };
            let valid = !id.is_empty()
                && id
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric());
            if !valid || tabs.position(id).is_some() {
                continue;
            }
            tabs.sessions.push(id.to_owned());
            if active {
                tabs.active = Some(tabs.sessions.len() - 1);
            }
        }
        if tabs.active.is_none() && !tabs.sessions.is_empty() {
            tabs.active = Some(0);
        }
        tabs
    }
}
