//! The state of the nomux desktop client, without GPUI.
//!
//! - [`HostState`]: the copy of the client of what the daemon of a host
//!   has: the workspace tree, the sessions and the Agent runs. It applies the
//!   events of the host and says what to load again ([`Reload`]).
//! - [`Selection`]: what the user selected: a Project (or the sessions
//!   without a Project), a session, and the open Groups of the rail.
//! - [`rail`] and [`sidebar`]: the content of the project rail and of the
//!   sidebar, as plain data for a view.
//! - [`GlyphState`]: the state that a glyph shows, and its priority.

mod age;
mod glyph_state;
mod host_state;
pub mod rail;
mod rail_labels;
mod selection;
pub mod sidebar;

pub use age::format_age;
pub use glyph_state::GlyphState;
pub use host_state::{HostState, Reload};
pub use selection::{ProjectView, Selection};
