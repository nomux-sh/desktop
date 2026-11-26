//! The context menu (right click): a raised box with one row for each choice
//! of the model. The window shows it, at the pointer, while the user has not
//! chosen or clicked outside of it.

use gpui::{
    AnyElement, ElementId, InteractiveElement, IntoElement, ParentElement, Pixels, Point,
    StatefulInteractiveElement, Styled, anchored, deferred, div,
};
use nomux_desktop_model::menu::{MenuCommand, MenuEntry};
use nomux_desktop_theme::{ColorToken, DesktopTheme, LengthToken, TextStyle};

use crate::ClickHandler;

/// The menu with `entries`, with its top left corner at `position` (a place
/// of the window). `on_choose` gets the command of a clicked choice, and
/// `on_dismiss` a click outside of the menu.
pub fn context_menu(
    theme: &DesktopTheme,
    position: Point<Pixels>,
    entries: &[MenuEntry],
    on_choose: &ClickHandler<MenuCommand>,
    on_dismiss: &ClickHandler<()>,
) -> AnyElement {
    let space_1 = theme.length(LengthToken::Space1);
    let on_dismiss = on_dismiss.clone();
    let menu = div()
        .id("context-menu")
        .occlude()
        .flex()
        .flex_col()
        .min_w(theme.length(LengthToken::Space4) * 10.0)
        .p(space_1)
        .rounded(theme.length(LengthToken::RadiusRaised))
        .border(theme.length(LengthToken::LineWidth))
        .border_color(theme.color(ColorToken::LineDivider))
        .bg(theme.color(ColorToken::SurfaceRaised))
        .on_mouse_down_out(move |_, window, cx| on_dismiss(&(), window, cx))
        .children(
            entries
                .iter()
                .enumerate()
                .map(|(index, entry)| choice(theme, index, entry, on_choose)),
        );
    deferred(
        anchored()
            .position(position)
            .snap_to_window_with_margin(space_1)
            .child(menu),
    )
    .with_priority(1)
    .into_any_element()
}

fn choice(
    theme: &DesktopTheme,
    index: usize,
    entry: &MenuEntry,
    on_choose: &ClickHandler<MenuCommand>,
) -> impl IntoElement {
    let metrics = theme.metrics();
    let font = theme.font(TextStyle::Row);
    let hover = theme.color(ColorToken::FillHover);
    let color = if entry.destructive {
        ColorToken::StateError
    } else {
        ColorToken::TextPrimary
    };
    let command = entry.command.clone();
    let on_choose = on_choose.clone();
    div()
        .id(ElementId::Name(format!("menu-choice-{index}").into()))
        .flex()
        .items_center()
        .h(metrics.control_height)
        .px(metrics.padding)
        .rounded(theme.length(LengthToken::RadiusButton))
        .font_family(font.family)
        .font_weight(font.weight)
        .text_size(theme.font_size(TextStyle::Row))
        .text_color(theme.color(color))
        .whitespace_nowrap()
        .hover(move |style| style.bg(hover))
        .on_click(move |_, window, cx| on_choose(&command, window, cx))
        .child(entry.label.clone())
}
