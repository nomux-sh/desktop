//! The sidebar of the selected Project: the sections "Geometry", "Rows",
//! "State glyphs" and "Empty project" of the specification.

use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Div, FontFeatures, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, div,
};
use nomux_desktop_model::GlyphState;
use nomux_desktop_model::sidebar::{SessionKind, SessionRow, SidebarContent};
use nomux_desktop_theme::{ColorToken, DesktopTheme, LengthToken, TextStyle};

use crate::ClickHandler;
use crate::glyph::{Mark, mark};

/// The tree level of a Session row. Level 0 is the worktree, which the
/// protocol does not have yet.
const SESSION_LEVEL: f32 = 1.0;

/// The width of the sidebar: 17 × `space.4` (272 px), the width of the
/// measured values of the specification.
#[must_use]
pub fn sidebar_width(theme: &DesktopTheme) -> Pixels {
    theme.length(LengthToken::Space4) * 17.0
}

/// The sidebar. `on_row` gets the session identifier of a clicked row, and
/// `on_new_session` the click on "New session" of an empty Project.
pub fn sidebar(
    content: Option<&SidebarContent>,
    theme: &DesktopTheme,
    on_row: &ClickHandler<String>,
    on_new_session: &ClickHandler<()>,
) -> impl IntoElement {
    let painter = SidebarPainter { theme };
    div()
        .id("sidebar")
        .flex_none()
        .w(sidebar_width(theme))
        .h_full()
        .overflow_y_scroll()
        .bg(theme.color(ColorToken::SurfaceBase))
        .px(theme.length(LengthToken::Space1))
        .flex()
        .flex_col()
        .when_some(content, |sidebar, content| {
            sidebar
                .child(painter.header(content))
                .child(div().h(theme.length(LengthToken::Space2)).flex_none())
                .when(content.rows.is_empty(), |sidebar| {
                    sidebar.child(painter.empty(on_new_session))
                })
                .children(content.rows.iter().map(|row| painter.row(row, on_row)))
        })
}

struct SidebarPainter<'a> {
    theme: &'a DesktopTheme,
}

impl SidebarPainter<'_> {
    fn text(&self, style: TextStyle, color: ColorToken) -> Div {
        let font = self.theme.font(style);
        div()
            .font_family(font.family)
            .font_weight(font.weight)
            .text_size(self.theme.font_size(style))
            .text_color(self.theme.color(color))
            .whitespace_nowrap()
    }

    /// The tail text: metadata with tabular figures, thus the ages make a
    /// column.
    fn tail_text(&self, color: ColorToken) -> Div {
        self.text(TextStyle::Meta, color)
            .font_features(FontFeatures(Arc::new(vec![("tnum".into(), 1)])))
    }

    fn header(&self, content: &SidebarContent) -> impl IntoElement {
        let theme = self.theme;
        let metrics = theme.metrics();
        let space_1 = theme.length(LengthToken::Space1);
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(metrics.row_height)
            .pl(metrics.padding - space_1)
            .pr(space_1)
            .gap(theme.length(LengthToken::Space2))
            .child(
                self.text(TextStyle::Title, ColorToken::TextPrimary)
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(content.title.clone()),
            )
            .when(content.needs_you > 0, |header| {
                header.child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(space_1)
                        .child(self.slot(Mark::State(GlyphState::NeedsYou)))
                        .child(
                            self.text(TextStyle::Meta, ColorToken::TextSecondary)
                                .child(format!("{} needs you", content.needs_you)),
                        ),
                )
            })
    }

    /// The 16 px slot with the glyph of `size.mark` in its centre.
    fn slot(&self, mark_kind: Mark) -> impl IntoElement {
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(self.theme.length(LengthToken::SizeIcon))
            .child(mark(
                mark_kind,
                self.theme.length(LengthToken::SizeMark),
                self.theme,
            ))
    }

    fn row(&self, row: &SessionRow, on_row: &ClickHandler<String>) -> AnyElement {
        let theme = self.theme;
        let metrics = theme.metrics();
        let space_1 = theme.length(LengthToken::Space1);
        let space_2 = theme.length(LengthToken::Space2);
        let indent = metrics.padding - space_1 + theme.length(LengthToken::Space4) * SESSION_LEVEL;
        let session_id = row.session_id.clone();
        let on_row = on_row.clone();
        let hover = theme.color(ColorToken::FillHover);
        let name = match row.kind {
            SessionKind::Agent => self
                .text(TextStyle::Row, ColorToken::TextPrimary)
                .font_weight(theme.font(TextStyle::Title).weight),
            SessionKind::Shell => self
                .text(TextStyle::Code, ColorToken::TextPrimary)
                .text_size(theme.font_size(TextStyle::Row)),
        };
        let secondary_color = if row.state == GlyphState::NeedsYou {
            ColorToken::TextPrimary
        } else {
            ColorToken::TextMuted
        };
        div()
            .id(gpui::ElementId::Name(
                format!("session-{}", row.session_id).into(),
            ))
            .flex()
            .flex_none()
            .items_center()
            .h(metrics.row_height)
            .pl(indent)
            .pr(space_1)
            .rounded(theme.length(LengthToken::RadiusButton))
            .when(row.selected, |row_box| {
                row_box.bg(theme.color(ColorToken::FillSelected))
            })
            .when(!row.selected, |row_box| {
                row_box.hover(move |style| style.bg(hover))
            })
            .on_click(move |_, window, cx| on_row(&session_id, window, cx))
            .child(self.slot(Mark::State(row.state)))
            .child(div().w(space_1).flex_none())
            .child(name.flex_none().child(row.name.clone()))
            .when_some(row.secondary.clone(), |row_box, secondary| {
                row_box.child(
                    self.text(TextStyle::Meta, secondary_color)
                        .ml(space_2)
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(secondary),
                )
            })
            .when(row.secondary.is_none(), |row_box| {
                row_box.child(div().flex_1())
            })
            .child(self.tail(row, space_2))
            .into_any_element()
    }

    /// The closed children (the glyph of the most urgent one and "+N"), then
    /// the age.
    fn tail(&self, row: &SessionRow, gap: Pixels) -> impl IntoElement {
        div()
            .flex()
            .flex_none()
            .items_center()
            .ml(gap)
            .gap(gap)
            .when_some(row.children, |tail, (state, count)| {
                tail.child(
                    div()
                        .flex()
                        .items_center()
                        .child(self.slot(Mark::State(state)))
                        .child(
                            self.tail_text(ColorToken::TextMuted)
                                .child(format!("+{count}")),
                        ),
                )
            })
            .when_some(row.age.clone(), |tail, age| {
                tail.child(self.tail_text(ColorToken::TextMuted).child(age))
            })
    }

    /// A Project without sessions: two lines and the button "New session".
    fn empty(&self, on_new_session: &ClickHandler<()>) -> impl IntoElement {
        let theme = self.theme;
        let metrics = theme.metrics();
        let space_1 = theme.length(LengthToken::Space1);
        let on_new_session = on_new_session.clone();
        div()
            .flex()
            .flex_col()
            .items_start()
            .pl(metrics.padding - space_1)
            .gap(space_1)
            .child(
                self.text(TextStyle::Row, ColorToken::TextSecondary)
                    .child("No sessions"),
            )
            .child(
                self.text(TextStyle::Meta, ColorToken::TextMuted)
                    .child("A session runs a shell or an agent in this project."),
            )
            .child(
                button(theme, "New session")
                    .id("sidebar-new-session")
                    .mt(theme.length(LengthToken::Space2) - space_1)
                    .on_click(move |_, window, cx| on_new_session(&(), window, cx)),
            )
    }
}

/// A button of elevation level 1: `surface.button`, `control-height`,
/// `radius.button`, the label in `font.row` and `text.primary`.
#[must_use]
pub fn button(theme: &DesktopTheme, label: &str) -> Div {
    let metrics = theme.metrics();
    let font = theme.font(TextStyle::Row);
    let hover: Hsla = theme
        .color(ColorToken::SurfaceButton)
        .blend(theme.color(ColorToken::FillHover));
    div()
        .flex()
        .items_center()
        .h(metrics.control_height)
        .px(metrics.padding)
        .rounded(theme.length(LengthToken::RadiusButton))
        .bg(theme.color(ColorToken::SurfaceButton))
        .hover(move |style| style.bg(hover))
        .font_family(font.family)
        .font_weight(font.weight)
        .text_size(theme.font_size(TextStyle::Row))
        .text_color(theme.color(ColorToken::TextPrimary))
        .child(label.to_owned())
}
