//! The project rail: the section "Project rail" of the specification. The
//! rail is closed; the drag to the expanded rail is not built yet.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Div, ElementId, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, div,
};
use nomux_desktop_model::rail::{RailTile, TileKind, TileTarget};
use nomux_desktop_theme::{ColorToken, DesktopTheme, LengthToken, TextStyle};

use crate::ClickHandler;
use crate::glyph::{Mark, mark};

/// The width of the closed rail: the tile and `space.3` on each side.
#[must_use]
pub fn rail_width(theme: &DesktopTheme) -> Pixels {
    theme.metrics().control_height + theme.length(LengthToken::Space3) * 2.0
}

/// The rail with `tiles`. `on_click` gets the target of a clicked tile.
pub fn rail(
    tiles: &[RailTile],
    theme: &DesktopTheme,
    on_click: &ClickHandler<TileTarget>,
) -> impl IntoElement {
    let painter = RailPainter { theme, on_click };
    div()
        .id("project-rail")
        .flex_none()
        .w(rail_width(theme))
        .h_full()
        .overflow_y_scroll()
        .bg(theme.color(ColorToken::SurfaceSunken))
        .child(
            painter
                .column()
                .py(theme.length(LengthToken::Space2))
                .children(tiles.iter().map(|tile| painter.tile(tile, 0))),
        )
}

struct RailPainter<'a> {
    theme: &'a DesktopTheme,
    on_click: &'a ClickHandler<TileTarget>,
}

impl RailPainter<'_> {
    fn column(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(self.theme.length(LengthToken::Space2))
    }

    /// One tile. `depth` is the number of open Groups that hold it.
    fn tile(&self, tile: &RailTile, depth: usize) -> AnyElement {
        match &tile.kind {
            TileKind::OpenGroup { children, .. } => self.strip(tile, children, depth + 1),
            _ => self.tile_box(tile, depth).into_any_element(),
        }
    }

    /// An open Group: the strip behind its tile and its children.
    fn strip(&self, tile: &RailTile, children: &[RailTile], depth: usize) -> AnyElement {
        let theme = self.theme;
        let space_1 = theme.length(LengthToken::Space1);
        // From depth 6, a strip has the inset and the fill of depth 5.
        let inset = space_1 / 2.0 * depth.min(5) as f32;
        self.column()
            .mx(inset)
            .my(-space_1)
            .py(space_1)
            .rounded(theme.length(LengthToken::RadiusRaised))
            .bg(theme.strip_fill(depth))
            .child(self.tile_box(tile, depth))
            .children(children.iter().map(|child| self.tile(child, depth)))
            .into_any_element()
    }

    fn tile_box(&self, tile: &RailTile, depth: usize) -> impl IntoElement {
        let theme = self.theme;
        let side = theme.metrics().control_height;
        let space_1 = theme.length(LengthToken::Space1);
        let space_3 = theme.length(LengthToken::Space3);
        let inset = space_1 / 2.0 * depth.min(5) as f32;
        let button = theme.color(ColorToken::SurfaceButton);
        let background = if tile.selected {
            button.blend(theme.color(ColorToken::FillSelected))
        } else {
            button
        };
        let hover = button.blend(theme.color(ColorToken::FillHover));
        let target = tile.target.clone();
        let on_click = self.on_click.clone();
        let mut tile_box = div()
            .id(element_id(&tile.target))
            .relative()
            .flex_none()
            .ml(space_3 - inset)
            .size(side)
            .flex()
            .items_center()
            .justify_center()
            .rounded(theme.length(LengthToken::RadiusButton))
            .bg(background)
            .when(!tile.selected, |tile_box| {
                tile_box.hover(move |style| style.bg(hover))
            })
            .on_click(move |_, window, cx| on_click(&target, window, cx))
            .child(self.face(tile));
        if tile.selected {
            tile_box = tile_box.child(self.selection_bar(space_3 - inset));
        }
        match &tile.kind {
            TileKind::Project { state, .. } | TileKind::ClosedGroup { state, .. } => {
                if let Some(state) = state {
                    tile_box = tile_box.child(self.corner_mark(Mark::State(*state), true));
                }
            }
            TileKind::OpenGroup { .. } => {
                tile_box = tile_box.child(self.corner_mark(Mark::Open, false));
            }
        }
        tile_box
    }

    fn face(&self, tile: &RailTile) -> AnyElement {
        let theme = self.theme;
        let label_color = if tile.selected {
            theme.color(ColorToken::TextPrimary)
        } else {
            theme.color(ColorToken::TextSecondary)
        };
        match &tile.kind {
            TileKind::Project { label, .. } => self.label(label, label_color),
            TileKind::OpenGroup { initial, .. } => self.label(initial, label_color),
            TileKind::ClosedGroup { grid, .. } => self.mini_grid(grid),
        }
    }

    fn label(&self, text: &str, color: Hsla) -> AnyElement {
        let font = self.theme.font(TextStyle::Title);
        div()
            .font_family(font.family)
            .font_weight(font.weight)
            .text_size(self.theme.font_size(TextStyle::Title))
            .text_color(color)
            .child(text.to_owned())
            .into_any_element()
    }

    /// 2 × 2 squares: filled for a Project, hollow for a Group.
    fn mini_grid(&self, grid: &[bool]) -> AnyElement {
        let theme = self.theme;
        let space_1 = theme.length(LengthToken::Space1);
        let square = (theme.length(LengthToken::SizeIcon) - space_1) / 2.0;
        let muted = theme.color(ColorToken::TextMuted);
        let radius = theme.length(LengthToken::RadiusButton) / 4.0;
        let cell = |filled: bool| {
            let square_box = div().size(square).rounded(radius);
            if filled {
                square_box.bg(muted)
            } else {
                square_box
                    .border(theme.length(LengthToken::LineWidth))
                    .border_color(muted)
            }
        };
        let row = |cells: &[bool]| {
            div()
                .flex()
                .gap(space_1)
                .h(square)
                .children(cells.iter().map(|filled| cell(*filled)))
        };
        let (top, bottom) = grid.split_at(grid.len().min(2));
        div()
            .flex()
            .flex_col()
            .gap(space_1)
            .size(theme.length(LengthToken::SizeIcon))
            .child(row(top))
            .child(row(bottom))
            .into_any_element()
    }

    /// The bar at the left edge of the rail, `focus.ring-width` wide and
    /// `size.icon` high, centred on the tile.
    fn selection_bar(&self, tile_x: Pixels) -> impl IntoElement {
        let theme = self.theme;
        let width = theme.length(LengthToken::FocusRingWidth);
        let height = theme.length(LengthToken::SizeIcon);
        let side = theme.metrics().control_height;
        div()
            .absolute()
            .left(-tile_x)
            .top((side - height) / 2.0)
            .w(width)
            .h(height)
            .rounded_r(width)
            .bg(theme.color(ColorToken::Accent))
    }

    /// A mark at the top right (state) or bottom right (open) corner. Its
    /// box extends `space.1` / 2 past the tile, and a ring of
    /// `focus.ring-width` in `surface.sunken` cuts it out of the tile.
    fn corner_mark(&self, mark_kind: Mark, top: bool) -> impl IntoElement {
        let theme = self.theme;
        let side = theme.metrics().control_height;
        let mark_side = theme.length(LengthToken::SizeCornerMark);
        let ring = theme.length(LengthToken::FocusRingWidth);
        let overhang = theme.length(LengthToken::Space1) / 2.0;
        let cut = mark_side + ring * 2.0;
        let left = side + overhang - mark_side - ring;
        let corner = div()
            .absolute()
            .left(left)
            .size(cut)
            .rounded(cut / 2.0)
            .bg(theme.color(ColorToken::SurfaceSunken))
            .flex()
            .items_center()
            .justify_center()
            .child(mark(mark_kind, mark_side, theme));
        if top {
            corner.top(-overhang - ring)
        } else {
            corner.top(side + overhang - mark_side - ring)
        }
    }
}

fn element_id(target: &TileTarget) -> ElementId {
    match target {
        TileTarget::Project(view) => {
            ElementId::Name(format!("project-{}", view.project_id().unwrap_or("~")).into())
        }
        TileTarget::Group(id) => ElementId::Name(format!("group-{id}").into()),
    }
}
