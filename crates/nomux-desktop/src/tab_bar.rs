//! The tab bar of the main area: one Tab for each open session, with the
//! state glyph and the title. The track is `surface.sunken`; the active Tab
//! is one tone step up (`surface.raised`) with the accent indicator. No
//! colour except the state glyph.

use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::{
    App, Context, ElementId, InteractiveElement, IntoElement, MouseButton, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div,
};
use nomux_desktop_model::TabItem;
use nomux_desktop_theme::{ColorToken, DesktopTheme, LengthToken, TextStyle};
use nomux_sidebar::{Mark, mark};

/// What a click on a Tab does.
pub(crate) type TabHandler = Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// The view of the tab bar. It renders again only when its Tabs or the
/// theme change, or when the clock of the working glyph asks.
pub(crate) struct TabBarView {
    theme: DesktopTheme,
    items: Vec<TabItem>,
    on_activate: TabHandler,
    on_close: TabHandler,
}

impl TabBarView {
    pub(crate) fn new(theme: DesktopTheme, on_activate: TabHandler, on_close: TabHandler) -> Self {
        Self {
            theme,
            items: Vec::new(),
            on_activate,
            on_close,
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
        let space_1 = theme.length(LengthToken::Space1);
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
        div()
            .id(ElementId::Name(format!("tab-{}", item.session_id).into()))
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap(space_1)
            .h(metrics.control_height)
            .px(metrics.padding)
            .rounded_t(theme.length(LengthToken::RadiusButton))
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
                        .top_0()
                        .h(theme.length(LengthToken::FocusRingWidth))
                        .bg(theme.color(ColorToken::Accent)),
                )
            })
            .when(!item.active, |tab| tab.hover(move |style| style.bg(hover)))
            .on_click(move |_, window, cx| on_activate(&id, window, cx))
            .on_mouse_down(MouseButton::Middle, move |_, window, cx| {
                on_close(&close_id, window, cx);
            })
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(theme.length(LengthToken::SizeIcon))
                    .child(mark(
                        Mark::State(item.state),
                        theme.length(LengthToken::SizeMark),
                        theme,
                        turning,
                    )),
            )
            .child(item.title.clone())
            .when(item.active, |tab| tab.child(self.close_button(item)))
    }

    /// The close mark of the active Tab, in `text.muted`.
    fn close_button(&self, item: &TabItem) -> impl IntoElement {
        let theme = &self.theme;
        let id = item.session_id.clone();
        let on_close = self.on_close.clone();
        let hover = theme.color(ColorToken::FillHover);
        div()
            .id(ElementId::Name(format!("close-{}", item.session_id).into()))
            .flex()
            .items_center()
            .justify_center()
            .size(theme.length(LengthToken::SizeIcon))
            .rounded(theme.length(LengthToken::RadiusButton))
            .text_color(theme.color(ColorToken::TextMuted))
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
            .flex()
            .items_end()
            .gap(space_1)
            .h(theme.metrics().control_height + space_1)
            .px(space_1)
            .overflow_x_scroll()
            .bg(theme.color(ColorToken::SurfaceSunken))
            .children(self.items.iter().map(|item| self.tab(item, turning)))
    }
}
