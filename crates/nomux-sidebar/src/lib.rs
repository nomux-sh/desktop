//! The project rail and the sidebar of the nomux desktop client, from the
//! model and the tokens of the theme. The geometry is the geometry of
//! `docs/sidebar.md` in the `themes` repository.
//!
//! The views own no state: the caller gives the content
//! ([`nomux_desktop_model::rail`], [`nomux_desktop_model::sidebar`]) and
//! the handlers of the clicks.

mod glyph;
mod rail;
mod sidebar;

use std::rc::Rc;

use gpui::{App, Window};

pub use glyph::{Mark, mark};
pub use rail::{rail, rail_width};
pub use sidebar::{button, sidebar, sidebar_width};

/// A click handler.
pub type ClickHandler<T> = Rc<dyn Fn(&T, &mut Window, &mut App)>;
