//! The project rail: the section "Project rail" of the specification. The
//! rail is closed; the drag to the expanded rail is not built yet.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Div, ElementId, Hsla, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Render, StatefulInteractiveElement, Styled, Window, div,
};
use nomux_desktop_model::ProjectView;
use nomux_desktop_model::menu::MenuTarget;
use nomux_desktop_model::rail::{RailTile, TileKind, TileTarget};
use nomux_desktop_theme::{ColorToken, DesktopTheme, LengthToken, TextStyle};

use crate::glyph::{Mark, mark};
use crate::{ClickHandler, ContextHandler};

/// The width of the closed rail: the tile and `space.3` on each side.
#[must_use]
pub fn rail_width(theme: &DesktopTheme) -> Pixels {
    theme.metrics().control_height + theme.length(LengthToken::Space3) * 2.0
}

/// The view of the rail. It renders again only when its tiles or the theme
/// change, or when the clock of the working glyph asks.
pub struct RailView {
    theme: DesktopTheme,
    tiles: Vec<RailTile>,
    handlers: RailHandlers,
}

/// What a click on a tile, on the add tile, and a right click on a tile do.
#[derive(Clone)]
pub struct RailHandlers {
    /// A click on a tile gets its target.
    pub on_click: ClickHandler<TileTarget>,
    /// A click on the add tile at the bottom of the rail.
    pub on_add: ClickHandler<()>,
    /// A right click on a tile of a Project.
    pub on_context: ContextHandler,
}

impl RailView {
    /// An empty rail.
    #[must_use]
    pub fn new(theme: DesktopTheme, handlers: RailHandlers) -> Self {
        Self {
            theme,
            tiles: Vec::new(),
            handlers,
        }
    }

    /// Shows `tiles`. Renders again only if they differ.
    pub fn set_tiles(&mut self, tiles: Vec<RailTile>, cx: &mut Context<Self>) {
        if self.tiles != tiles {
            self.tiles = tiles;
            cx.notify();
        }
    }

    /// Takes a new theme.
    pub fn set_theme(&mut self, theme: DesktopTheme, cx: &mut Context<Self>) {
        self.theme = theme;
        cx.notify();
    }

    /// `true` if a tile shows the working glyph.
    #[must_use]
    pub fn animates(&self) -> bool {
        self.tiles.iter().any(RailTile::animates)
    }

    /// Paints the working glyphs at the next frame.
    pub fn tick(&mut self, cx: &mut Context<Self>) {
        if self.animates() {
            cx.notify();
        }
    }
}

impl Render for RailView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        rail(
            &self.tiles,
            &self.theme,
            &self.handlers,
            !cx.reduce_motion(),
        )
    }
}

/// The rail with `tiles`, and the add tile below them.
fn rail(
    tiles: &[RailTile],
    theme: &DesktopTheme,
    handlers: &RailHandlers,
    turning: bool,
) -> impl IntoElement {
    let painter = RailPainter {
        theme,
        handlers,
        turning,
    };
    div()
        .flex_none()
        .flex()
        .flex_col()
        .w(rail_width(theme))
        .h_full()
        .bg(theme.color(ColorToken::SurfaceSunken))
        .child(
            div()
                .id("project-rail")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(
                    painter
                        .column()
                        .py(theme.length(LengthToken::Space2))
                        .children(tiles.iter().map(|tile| painter.tile(tile, 0))),
                ),
        )
        .child(painter.add_tile())
}

struct RailPainter<'a> {
    theme: &'a DesktopTheme,
    handlers: &'a RailHandlers,
    turning: bool,
}

impl RailPainter<'_> {
    fn column(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(self.theme.length(LengthToken::Space2))
    }

    /// The tile at the bottom of the rail: "+" adds a Project.
    fn add_tile(&self) -> impl IntoElement {
        let theme = self.theme;
        let side = theme.metrics().control_height;
        let hover = theme.color(ColorToken::FillHover);
        let on_add = self.handlers.on_add.clone();
        div()
            .id("add-project")
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .my(theme.length(LengthToken::Space2))
            .ml(theme.length(LengthToken::Space3))
            .size(side)
            .rounded(theme.length(LengthToken::RadiusButton))
            .text_color(theme.color(ColorToken::TextSecondary))
            .hover(move |style| style.bg(hover))
            .on_click(move |_, window, cx| on_add(&(), window, cx))
            .child(self.label("+", theme.color(ColorToken::TextSecondary)))
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
        let on_click = self.handlers.on_click.clone();
        let context_target = match &tile.target {
            TileTarget::Project(ProjectView::Project(id)) => {
                Some(MenuTarget::Project { id: id.clone() })
            }
            _ => None,
        };
        let on_context = self.handlers.on_context.clone();
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
            .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                if let Some(target) = &context_target {
                    on_context(&(target.clone(), event.position), window, cx);
                }
            })
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
            .child(mark(mark_kind, mark_side, theme, self.turning));
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
