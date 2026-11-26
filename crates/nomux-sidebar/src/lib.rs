//! The project rail and the sidebar of the nomux desktop client, from the
//! model and the tokens of the theme. The geometry is the geometry of
//! `docs/sidebar.md` in the `themes` repository.
//!
//! The views hold only what they show: the caller gives the content
//! ([`nomux_desktop_model::rail`], [`nomux_desktop_model::sidebar`]) and
//! the handlers of the clicks. A view renders again only when its content
//! changes; the window paints it from the cache in the other frames.

mod glyph;
mod rail;
mod sidebar;

use std::rc::Rc;

use gpui::{App, Window};

pub use glyph::{Mark, mark};
pub use rail::{RailView, rail_width};
pub use sidebar::{SessionRowView, SidebarView, button, sidebar_width};

/// A click handler.
pub type ClickHandler<T> = Rc<dyn Fn(&T, &mut Window, &mut App)>;
