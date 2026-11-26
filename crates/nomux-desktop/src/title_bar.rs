//! The title bar on macOS. The window has a transparent title bar: the
//! surfaces of the theme reach the top edge, and the window buttons sit at
//! the top of the project rail, on the line of the sidebar header and the
//! tab bar. Where that line has no control, it is the title bar: a drag
//! moves the window, and a double click does what the system setting says
//! (zoom, minimize or nothing). In full screen the window buttons are hidden
//! and the line gives back its space. Other platforms keep the title bar of
//! the system.

use gpui::{InteractiveElement, MouseButton, Pixels, Point, Window, point};
use nomux_desktop_theme::{DesktopTheme, LengthToken};

use crate::tab_bar::tab_bar_height;

/// The size of one window button of macOS.
const BUTTON_SIZE: f32 = 14.0;
/// The width of the three window buttons of macOS 26: 14 pt each, 9 pt
/// apart. Earlier versions place them closer.
const BUTTONS_WIDTH: f32 = 3.0 * BUTTON_SIZE + 2.0 * 9.0;

/// `true` where the window has a transparent title bar.
pub(crate) const TRANSPARENT: bool = cfg!(target_os = "macos");

/// The position of the window buttons: at the left edge of the rail tiles,
/// centred on the line of the tab bar.
pub(crate) fn window_buttons(theme: &DesktopTheme) -> Point<Pixels> {
    let rail = nomux_sidebar::rail_width(theme);
    let tile = theme.metrics().control_height;
    point(
        (rail - tile) / 2.0,
        (tab_bar_height(theme) - gpui::px(BUTTON_SIZE)) / 2.0,
    )
}

/// The height of the title bar line: the height of the tab bar.
pub(crate) fn height(theme: &DesktopTheme) -> Pixels {
    tab_bar_height(theme)
}

/// The x of the sidebar header text from the left edge of the sidebar, after
/// the window buttons.
pub(crate) fn header_start(theme: &DesktopTheme) -> Pixels {
    window_buttons(theme).x + gpui::px(BUTTONS_WIDTH) + theme.length(LengthToken::Space2)
        - nomux_sidebar::rail_width(theme)
}

/// `true` while the window shows the title bar line: a transparent title
/// bar and no full screen.
pub(crate) fn shown(window: &Window) -> bool {
    TRANSPARENT && !window.is_fullscreen()
}

/// Makes `element` a part of the title bar: a drag moves the window and a
/// double click does the action of the system setting.
pub(crate) fn drag_region<E: InteractiveElement>(element: E) -> E {
    if !TRANSPARENT {
        return element;
    }
    element.on_mouse_down(MouseButton::Left, |event, window, _| {
        if !shown(window) {
            return;
        }
        if event.click_count == 2 {
            window.titlebar_double_click();
        } else {
            window.start_window_move();
        }
    })
}
