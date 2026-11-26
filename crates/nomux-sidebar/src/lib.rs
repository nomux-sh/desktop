//! The project rail and the sidebar of the nomux desktop client, from the
//! model and the tokens of the theme. The geometry is the geometry of
//! `docs/sidebar.md` in the `themes` repository.
//!
//! The views hold only what they show: the caller gives the content
//! ([`nomux_desktop_model::rail`], [`nomux_desktop_model::sidebar`]) and
//! the handlers of the clicks. A view renders again only when its content
//! changes; the window paints it from the cache in the other frames.

mod context_menu;
mod glyph;
mod rail;
mod sidebar;

use std::rc::Rc;

use gpui::{App, Pixels, Point, Window};
use nomux_desktop_model::menu::MenuTarget;

pub use context_menu::context_menu;
pub use glyph::{Mark, mark};
pub use rail::{RailHandlers, RailView, rail_width};
pub use sidebar::{SessionRowView, SidebarView, button, sidebar_width};

/// A click handler.
pub type ClickHandler<T> = Rc<dyn Fn(&T, &mut Window, &mut App)>;

/// A right click: what was clicked and where the pointer is, in the window.
pub type ContextHandler = ClickHandler<(MenuTarget, Point<Pixels>)>;
