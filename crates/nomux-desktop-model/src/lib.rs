//! The state of the nomux desktop client, without GPUI.
//!
//! - [`HostState`]: the copy of the client of what the daemon of a host
//!   has: the workspace tree, the sessions and the Agent runs. It applies the
//!   events of the host and says what to load again ([`Reload`]).
//! - [`Selection`]: what the user selected: a Project (or the sessions
//!   without a Project), a session, and the open Groups of the rail.
//! - [`rail`] and [`sidebar`]: the content of the project rail and of the
//!   sidebar, as plain data for a view.
//! - [`Tabs`]: the open terminal tabs, the active one, and their saved
//!   state.
//! - [`menu`]: the choices of the context menus.
//! - [`daemon_support`]: whether the running daemon has what the client
//!   needs.
//! - [`attention`]: the sessions that need the user, across all Projects.
//! - [`notification`]: when a session earns a system notification.
//! - [`GlyphState`]: the state that a glyph shows, and its priority.
//! - [`motion`]: when an animation may run, and the phase of a blinking
//!   cursor.

mod age;
pub mod attention;
mod daemon_support;
mod glyph_state;
mod host_state;
mod line_editor;
pub mod menu;
pub mod motion;
pub mod notification;
pub mod rail;
mod rail_labels;
mod selection;
pub mod sidebar;
mod tabs;

pub use age::format_age;
pub use daemon_support::{DaemonSupport, MINIMUM_DAEMON_VERSION, daemon_support};
pub use glyph_state::GlyphState;
pub use host_state::{HostState, Reload, project_name};
pub use line_editor::LineEditor;
pub use selection::{ProjectView, Selection};
pub use tabs::{TabItem, Tabs};
