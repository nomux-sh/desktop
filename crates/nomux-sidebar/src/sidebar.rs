//! The sidebar of the selected Project: the sections "Geometry", "Rows",
//! "State glyphs" and "Empty project" of the specification.
//!
//! [`SidebarView`] holds one [`SessionRowView`] for each row. A row renders
//! again only when its displayed values change, or when the clock of the
//! working glyph asks; the other rows are painted from the cache of GPUI.

use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AnyView, AppContext, Context, Div, Entity, FontFeatures, Hsla, InteractiveElement,
    IntoElement, ParentElement, Pixels, Render, StatefulInteractiveElement, StyleRefinement,
    Styled, Window, div,
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

/// The header of the sidebar: what the sidebar view paints itself.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Header {
    title: String,
    needs_you: usize,
    empty: bool,
}

/// The view of the sidebar. `on_row` gets the session identifier of a
/// clicked row, and `on_new_session` the click on "New session" of an
/// empty Project.
pub struct SidebarView {
    theme: DesktopTheme,
    header: Option<Header>,
    /// The x of the header text from the left edge of the sidebar, if
    /// something of the window covers the start of the header.
    header_start: Option<Pixels>,
    rows: Vec<Entity<SessionRowView>>,
    on_row: ClickHandler<String>,
    on_new_session: ClickHandler<()>,
}

impl SidebarView {
    /// An empty sidebar.
    #[must_use]
    pub fn new(
        theme: DesktopTheme,
        on_row: ClickHandler<String>,
        on_new_session: ClickHandler<()>,
    ) -> Self {
        Self {
            theme,
            header: None,
            header_start: None,
            rows: Vec::new(),
            on_row,
            on_new_session,
        }
    }

    /// Shows `content`. The sidebar renders again only if the header or the
    /// sequence of the rows changed, and a row only if its values changed.
    pub fn set_content(&mut self, content: Option<&SidebarContent>, cx: &mut Context<Self>) {
        let header = content.map(|content| Header {
            title: content.title.clone(),
            needs_you: content.needs_you,
            empty: content.rows.is_empty(),
        });
        let mut changed = header != self.header;
        self.header = header;
        let new_rows = content.map_or(&[][..], |content| content.rows.as_slice());
        let mut old_rows = std::mem::take(&mut self.rows);
        for row in new_rows {
            let existing = old_rows
                .iter()
                .position(|view| view.read(cx).row.session_id == row.session_id);
            let view = match existing {
                Some(position) => {
                    changed |= position != 0;
                    let view = old_rows.remove(position);
                    view.update(cx, |view, cx| view.set_row(row, cx));
                    view
                }
                None => {
                    changed = true;
                    let theme = self.theme;
                    let on_row = self.on_row.clone();
                    let row = row.clone();
                    cx.new(|_| SessionRowView::new(row, theme, on_row))
                }
            };
            self.rows.push(view);
        }
        changed |= !old_rows.is_empty();
        if changed {
            cx.notify();
        }
    }

    /// Moves the header text to `start` from the left edge of the sidebar,
    /// for the window buttons of a transparent title bar. `None`, or a
    /// start before `density.*.padding`, keeps the text at the padding.
    pub fn set_header_start(&mut self, start: Option<Pixels>, cx: &mut Context<Self>) {
        if self.header_start != start {
            self.header_start = start;
            cx.notify();
        }
    }

    /// Takes a new theme.
    pub fn set_theme(&mut self, theme: DesktopTheme, cx: &mut Context<Self>) {
        self.theme = theme;
        for row in &self.rows {
            row.update(cx, |row, cx| row.set_theme(theme, cx));
        }
        cx.notify();
    }

    /// The row views, in their sequence.
    #[must_use]
    pub fn rows(&self) -> &[Entity<SessionRowView>] {
        &self.rows
    }

    /// `true` if a row shows the working glyph.
    #[must_use]
    pub fn animates(&self, cx: &gpui::App) -> bool {
        self.rows.iter().any(|row| row.read(cx).row.animates())
    }

    /// Paints the working glyphs at the next frame: only the rows that show
    /// one render again.
    pub fn tick(&mut self, cx: &mut Context<Self>) {
        for row in &self.rows {
            row.update(cx, |row, cx| {
                if row.row.animates() {
                    cx.notify();
                }
            });
        }
    }
}

impl Render for SidebarView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let theme = &self.theme;
        let painter = SidebarPainter {
            theme,
            turning: false,
        };
        let row_height = theme.metrics().row_height;
        div()
            .id("sidebar")
            .flex_none()
            .w(sidebar_width(theme))
            .h_full()
            .overflow_y_scroll()
            .bg(theme.color(ColorToken::SurfaceBase))
            .p(theme.length(LengthToken::Space1))
            .flex()
            .flex_col()
            .when_some(self.header.as_ref(), |sidebar, header| {
                sidebar
                    .child(painter.header(header, self.header_start))
                    .child(div().h(theme.length(LengthToken::Space2)).flex_none())
                    .when(header.empty, |sidebar| {
                        sidebar.child(painter.empty(&self.on_new_session))
                    })
            })
            .children(self.rows.iter().map(|row| {
                AnyView::from(row.clone()).cached(
                    StyleRefinement::default()
                        .w_full()
                        .h(row_height)
                        .flex_none(),
                )
            }))
    }
}

/// The view of one Session row.
pub struct SessionRowView {
    row: SessionRow,
    theme: DesktopTheme,
    on_row: ClickHandler<String>,
    paints: u64,
}

impl SessionRowView {
    fn new(row: SessionRow, theme: DesktopTheme, on_row: ClickHandler<String>) -> Self {
        Self {
            row,
            theme,
            on_row,
            paints: 0,
        }
    }

    fn set_row(&mut self, row: &SessionRow, cx: &mut Context<Self>) {
        if self.row != *row {
            self.row = row.clone();
            cx.notify();
        }
    }

    fn set_theme(&mut self, theme: DesktopTheme, cx: &mut Context<Self>) {
        self.theme = theme;
        cx.notify();
    }

    /// The row that the view shows.
    #[must_use]
    pub fn row(&self) -> &SessionRow {
        &self.row
    }

    /// The number of times that the row rendered.
    #[must_use]
    pub fn paints(&self) -> u64 {
        self.paints
    }
}

impl Render for SessionRowView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.paints += 1;
        let painter = SidebarPainter {
            theme: &self.theme,
            turning: !cx.reduce_motion(),
        };
        painter.row(&self.row, &self.on_row)
    }
}

struct SidebarPainter<'a> {
    theme: &'a DesktopTheme,
    turning: bool,
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

    fn header(&self, content: &Header, start: Option<Pixels>) -> impl IntoElement {
        let theme = self.theme;
        let metrics = theme.metrics();
        let space_1 = theme.length(LengthToken::Space1);
        let start = start.map_or(metrics.padding, |start| start.max(metrics.padding));
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(metrics.row_height)
            .pl(start - space_1)
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
                self.turning,
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
            .w_full()
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
