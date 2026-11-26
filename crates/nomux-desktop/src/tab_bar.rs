//! The tab bar of the main area: one Tab for each open session, with the
//! state glyph and the title, as the tab bar of the preview page of the
//! themes: the track is `surface.sunken` with `space.1` above and below and
//! `space.2` at the sides; a Tab is `control-height` high with
//! `radius.button`; the active Tab is one tone step up (`surface.raised`) in
//! `text.primary` with the accent line of `focus.ring-width` at its bottom.
//! No colour except the state glyph. Below the pointer the close mark takes
//! the place of the state glyph, as in Safari, thus a Tab at rest has no
//! empty place. The track after the last Tab is a part of the title bar.

use std::rc::Rc;

use gpui::Pixels;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, Context, ElementId, InteractiveElement, IntoElement, MouseButton, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div,
};
use nomux_desktop_model::TabItem;
use nomux_desktop_model::menu::MenuTarget;
use nomux_desktop_theme::{ColorToken, DesktopTheme, LengthToken, TextStyle};
use nomux_sidebar::{ContextHandler, Mark, mark};

use crate::title_bar;

/// The height of the tab bar: a Tab and `space.1` above and below it.
pub(crate) fn tab_bar_height(theme: &DesktopTheme) -> Pixels {
    theme.metrics().control_height + theme.length(LengthToken::Space1) * 2.0
}

/// What a click on a Tab does.
pub(crate) type TabHandler = Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// The view of the tab bar. It renders again only when its Tabs or the
/// theme change, or when the clock of the working glyph asks.
pub(crate) struct TabBarView {
    theme: DesktopTheme,
    items: Vec<TabItem>,
    on_activate: TabHandler,
    on_close: TabHandler,
    on_context: ContextHandler,
}

impl TabBarView {
    pub(crate) fn new(
        theme: DesktopTheme,
        handlers: (TabHandler, TabHandler, ContextHandler),
    ) -> Self {
        let (on_activate, on_close, on_context) = handlers;
        Self {
            theme,
            items: Vec::new(),
            on_activate,
            on_close,
            on_context,
        }
    }

    pub(crate) fn set_items(&mut self, items: Vec<TabItem>, cx: &mut Context<Self>) {
        if self.items != items {
            self.items = items;
            cx.notify();
        }
    }

    pub(crate) fn set_theme(&mut self, theme: DesktopTheme, cx: &mut Context<Self>) {
        self.theme = theme;
        cx.notify();
    }

    pub(crate) fn animates(&self) -> bool {
        self.items.iter().any(TabItem::animates)
    }

    pub(crate) fn tick(&mut self, cx: &mut Context<Self>) {
        if self.animates() {
            cx.notify();
        }
    }

    fn tab(&self, item: &TabItem, turning: bool) -> impl IntoElement {
        let theme = &self.theme;
        let metrics = theme.metrics();
        let font = theme.font(TextStyle::Title);
        let hover = theme.color(ColorToken::FillHover);
        let text = if item.active {
            ColorToken::TextPrimary
        } else {
            ColorToken::TextSecondary
        };
        let id = item.session_id.clone();
        let on_activate = self.on_activate.clone();
        let close_id = item.session_id.clone();
        let on_close = self.on_close.clone();
        let context_target = MenuTarget::Tab {
            id: item.session_id.clone(),
            tab_count: self.items.len(),
        };
        let on_context = self.on_context.clone();
        let group = format!("tab-{}", item.session_id);
        div()
            .id(ElementId::Name(group.clone().into()))
            .group(group.clone())
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap(theme.length(LengthToken::Space2))
            .h(metrics.control_height)
            .px(metrics.padding)
            .rounded(theme.length(LengthToken::RadiusButton))
            .font_family(font.family)
            .font_weight(font.weight)
            .text_size(theme.font_size(TextStyle::Title))
            .text_color(theme.color(text))
            .when(item.active, |tab| {
                tab.bg(theme.color(ColorToken::SurfaceRaised)).child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(theme.length(LengthToken::FocusRingWidth))
                        .rounded_b(theme.length(LengthToken::RadiusButton))
                        .bg(theme.color(ColorToken::Accent)),
                )
            })
            .when(!item.active, |tab| tab.hover(move |style| style.bg(hover)))
            .on_click(move |_, window, cx| on_activate(&id, window, cx))
            .on_mouse_down(MouseButton::Middle, move |_, window, cx| {
                on_close(&close_id, window, cx);
            })
            .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                on_context(&(context_target.clone(), event.position), window, cx);
            })
            .child(
                div()
                    .relative()
                    .flex_none()
                    .size(theme.length(LengthToken::SizeIcon))
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .group_hover(group.clone(), |style| style.invisible())
                            .child(mark(
                                Mark::State(item.state),
                                theme.length(LengthToken::SizeMark),
                                theme,
                                turning,
                            )),
                    )
                    .child(self.close_button(item, group)),
            )
            .child(item.title.clone())
    }

    /// The close mark in the slot of the state glyph: below the pointer
    /// only, in `text.secondary`, as the actions of a row. A hidden mark
    /// takes no click.
    fn close_button(&self, item: &TabItem, group: String) -> impl IntoElement {
        let theme = &self.theme;
        let id = item.session_id.clone();
        let on_close = self.on_close.clone();
        let hover = theme.color(ColorToken::FillHover);
        div()
            .id(ElementId::Name(format!("close-{}", item.session_id).into()))
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(theme.length(LengthToken::RadiusButton))
            .text_color(theme.color(ColorToken::TextSecondary))
            .invisible()
            .group_hover(group, |style| style.visible())
            .hover(move |style| style.bg(hover))
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                on_close(&id, window, cx);
            })
            .child("×")
    }
}

impl Render for TabBarView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = &self.theme;
        let space_1 = theme.length(LengthToken::Space1);
        let turning = !cx.reduce_motion();
        div()
            .id("tab-bar")
            .flex_none()
            .w_full()
            .flex()
            .items_center()
            .gap(space_1)
            .h(tab_bar_height(theme))
            .px(theme.length(LengthToken::Space2))
            .overflow_x_scroll()
            .bg(theme.color(ColorToken::SurfaceSunken))
            .children(self.items.iter().map(|item| self.tab(item, turning)))
            .child(title_bar::drag_region(
                div().id("tab-bar-title").flex_1().h_full(),
            ))
    }
}
