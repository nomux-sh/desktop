//! The GPUI element that paints a terminal snapshot.
//!
//! Each cell paints its glyphs one by one at its own column: a symbol is
//! shaped one time and kept in the [`GlyphCache`], and the colour comes at
//! paint time. Backgrounds of one colour are merged into one quad for each
//! run. Thus a frame has no shaping of lines and few quads.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    App, BorderStyle, Bounds, Element, ElementId, ElementInputHandler, Entity, EntityInputHandler,
    FocusHandle, Font, FontStyle, FontWeight, GlobalElementId, Hsla, InspectorElementId, LayoutId,
    Pixels, Point, SharedString, Size, Style, TextAlign, TextRun, UnderlineStyle, Window, fill,
    outline, point, relative, size,
};
use nomux_term::CursorShape;

use crate::glyph_cache::{GlyphCache, GlyphKey, PositionedGlyph};
use crate::paint_plan::{CellRun, CursorMark, CursorState, LineKind, PaintPlan};
use crate::palette::TerminalPalette;
use crate::selection::{GridPoint, GridSelection};
use crate::snapshot::TerminalSnapshot;

/// How a terminal looks: from the tokens of the theme.
#[derive(Debug, Clone)]
pub struct TerminalStyle {
    /// The colours.
    pub palette: TerminalPalette,
    /// The monospace font, regular weight.
    pub font: Font,
    /// The font size.
    pub font_size: Pixels,
    /// The space between the edge of the element and the cells.
    pub padding: Pixels,
    /// The width of a bar cursor, of an underline cursor and of the outline
    /// of a cursor without focus.
    pub cursor_width: Pixels,
    /// The width of an underline and of a strikethrough.
    pub line_width: Pixels,
}

/// Where the cells are: written by the element at each layout, read by the
/// view for the mouse and for the size of the session.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GridLayout {
    /// The top left corner of the cell in row 0 and column 0.
    pub origin: Point<Pixels>,
    /// The size of one cell.
    pub cell: Size<Pixels>,
    /// The number of columns that fit.
    pub columns: u16,
    /// The number of rows that fit.
    pub rows: u16,
}

impl GridLayout {
    /// The cell below a position, clamped to the grid.
    #[must_use]
    pub fn cell_at(&self, position: Point<Pixels>) -> GridPoint {
        let column = ((position.x - self.origin.x) / self.cell.width).floor();
        let row = ((position.y - self.origin.y) / self.cell.height).floor();
        GridPoint {
            row: clamp_index(row, self.rows),
            column: clamp_index(column, self.columns),
        }
    }

    fn cell_bounds(&self, row: u16, column: u16, columns: u16) -> Bounds<Pixels> {
        let left = (self.origin.x + self.cell.width * f32::from(column)).round();
        let right = (self.origin.x + self.cell.width * f32::from(column + columns)).round();
        let top = self.origin.y + self.cell.height * f32::from(row);
        Bounds::new(point(left, top), size(right - left, self.cell.height))
    }
}

fn clamp_index(value: f32, count: u16) -> u16 {
    let largest = f32::from(count.saturating_sub(1));
    // The value is clamped into the range of u16 before the cast.
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let index = value.clamp(0.0, largest) as u16;
    index
}

/// The numbers of the last paint, for the measurement mode and the
/// tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaintStats {
    /// When the paint of the element ended.
    pub painted_at: Instant,
    /// The generation of the painted frame.
    pub generation: u64,
    /// The time of the paint of the element.
    pub duration: Duration,
    /// The number of quads.
    pub quads: usize,
    /// The number of cells that painted a symbol.
    pub glyph_cells: usize,
    /// The glyph cache hits.
    pub cache_hits: usize,
    /// The glyph cache misses: each one shaped a symbol.
    pub cache_misses: usize,
}

/// What one frame shows.
#[derive(Debug, Clone)]
pub struct TerminalFrame {
    /// The screen.
    pub snapshot: Arc<TerminalSnapshot>,
    /// A number that grows with each new snapshot.
    pub generation: u64,
    /// The selected cells.
    pub selection: Option<GridSelection>,
    /// The text of an IME composition, drawn at the cursor.
    pub marked_text: Option<SharedString>,
    /// `false` in the hidden half of a blink cycle.
    pub cursor_shown: bool,
}

/// The state that lives longer than one frame: shared by the view and its
/// elements.
#[derive(Debug, Clone, Default)]
pub struct TerminalCaches {
    /// The shaped glyphs.
    pub glyphs: Rc<RefCell<GlyphCache>>,
    /// Where the cells are, written at each layout.
    pub layout: Rc<Cell<GridLayout>>,
    /// The numbers of the last paint.
    pub stats: Rc<Cell<Option<PaintStats>>>,
}

/// The terminal element. `V` is the view that takes the text input of the
/// platform (keys that are text, and the IME).
pub struct TerminalElement<V: 'static> {
    frame: TerminalFrame,
    style: TerminalStyle,
    caches: TerminalCaches,
    focus: FocusHandle,
    view: Entity<V>,
}

impl<V: EntityInputHandler> TerminalElement<V> {
    /// An element that paints one frame.
    #[must_use]
    pub fn new(
        frame: TerminalFrame,
        style: TerminalStyle,
        caches: TerminalCaches,
        focus: FocusHandle,
        view: Entity<V>,
    ) -> Self {
        Self {
            frame,
            style,
            caches,
            focus,
            view,
        }
    }

    fn grid_layout(&self, bounds: Bounds<Pixels>, window: &Window) -> GridLayout {
        let text_system = window.text_system();
        let font_id = text_system.resolve_font(&self.style.font);
        let font_size = self.style.font_size;
        let cell_width = text_system
            .advance(font_id, font_size, 'm')
            .map_or(font_size, |advance| advance.width);
        let ascent = text_system.ascent(font_id, font_size);
        let descent = text_system.descent(font_id, font_size);
        let cell_height = (ascent + descent.abs()).ceil();
        let padding = self.style.padding;
        let inner = size(
            bounds.size.width - padding * 2.0,
            bounds.size.height - padding * 2.0,
        );
        GridLayout {
            origin: bounds.origin + point(padding, padding),
            cell: size(cell_width, cell_height),
            columns: clamp_index((inner.width / cell_width).floor(), u16::MAX).max(2),
            rows: clamp_index((inner.height / cell_height).floor(), u16::MAX).max(1),
        }
    }

    fn baseline_offset(&self, window: &Window, cell_height: Pixels) -> Pixels {
        let text_system = window.text_system();
        let font_id = text_system.resolve_font(&self.style.font);
        let ascent = text_system.ascent(font_id, self.style.font_size);
        let descent = text_system.descent(font_id, self.style.font_size);
        ascent + (cell_height - ascent - descent.abs()) / 2.0
    }

    fn paint_runs(
        &self,
        runs: &[CellRun],
        layout: &GridLayout,
        window: &mut Window,
        quads: &mut usize,
    ) {
        for run in runs {
            window.paint_quad(fill(
                layout.cell_bounds(run.row, run.column, run.len),
                run.color,
            ));
            *quads += 1;
        }
    }

    fn paint_glyphs(
        &self,
        plan: &PaintPlan,
        layout: &GridLayout,
        window: &mut Window,
    ) -> (usize, usize) {
        let baseline = self.baseline_offset(window, layout.cell.height);
        let mut cache = self.caches.glyphs.borrow_mut();
        for glyph in &plan.glyphs {
            let Some(cell) = self.frame.snapshot.cell(glyph.row, glyph.column) else {
                continue;
            };
            let key = GlyphKey {
                character: cell.character,
                combining: cell.combining.clone(),
                bold: glyph.bold,
                italic: glyph.italic,
            };
            let font = self.font_of(&key);
            let font_size = self.style.font_size;
            let shaped = cache.get_or_shape(&key, || shape(&key, font, font_size, window));
            let cell_origin = layout.cell_bounds(glyph.row, glyph.column, 1).origin;
            let origin = point(cell_origin.x, cell_origin.y + baseline);
            for positioned in shaped.iter() {
                let at = point(origin.x + positioned.x, origin.y);
                // A glyph that the atlas cannot take is left out of the frame.
                let _ = if positioned.is_emoji {
                    window.paint_emoji(at, positioned.font_id, positioned.glyph_id, font_size)
                } else {
                    window.paint_glyph(
                        at,
                        positioned.font_id,
                        positioned.glyph_id,
                        font_size,
                        glyph.color,
                    )
                };
            }
        }
        cache.take_counts()
    }

    fn font_of(&self, key: &GlyphKey) -> Font {
        let mut font = self.style.font.clone();
        if key.bold {
            font.weight = FontWeight::BOLD;
        }
        if key.italic {
            font.style = FontStyle::Italic;
        }
        font
    }

    fn paint_lines(&self, plan: &PaintPlan, layout: &GridLayout, window: &mut Window) -> usize {
        let thickness = self.style.line_width;
        for line in &plan.lines {
            let cells = layout.cell_bounds(line.cells.row, line.cells.column, line.cells.len);
            let y = match line.kind {
                LineKind::Underline => cells.bottom() - thickness * 2.0,
                LineKind::Strikethrough => cells.center().y,
            };
            window.paint_quad(fill(
                Bounds::new(point(cells.left(), y), size(cells.size.width, thickness)),
                line.cells.color,
            ));
        }
        plan.lines.len()
    }

    fn paint_cursor(
        &self,
        cursor: &CursorMark,
        layout: &GridLayout,
        window: &mut Window,
        block_only: bool,
    ) {
        let cell = layout.cell_bounds(cursor.row, cursor.column, cursor.width);
        let color = self.style.palette.cursor;
        let width = self.style.cursor_width;
        let filled_block = cursor.shape == CursorShape::Block && cursor.focused;
        if block_only != filled_block {
            return;
        }
        let quad = match cursor.shape {
            CursorShape::Block if cursor.focused => fill(cell, color),
            CursorShape::Block => outline(cell, color, BorderStyle::Solid),
            CursorShape::Bar => fill(
                Bounds::new(cell.origin, size(width, cell.size.height)),
                color,
            ),
            CursorShape::Underline => fill(
                Bounds::new(
                    point(cell.left(), cell.bottom() - width),
                    size(cell.size.width, width),
                ),
                color,
            ),
        };
        window.paint_quad(quad);
    }

    fn paint_marked_text(&self, layout: &GridLayout, window: &mut Window, cx: &mut App) {
        let Some(text) = &self.frame.marked_text else {
            return;
        };
        let Some(cursor) = self.frame.snapshot.visible_cursor() else {
            return;
        };
        let origin = layout
            .cell_bounds(cursor.position.row, cursor.position.column, 1)
            .origin;
        let palette = &self.style.palette;
        let run = TextRun {
            len: text.len(),
            font: self.style.font.clone(),
            color: palette.foreground,
            background_color: Some(palette.background),
            underline: Some(UnderlineStyle {
                thickness: self.style.line_width,
                color: Some(palette.foreground),
                wavy: false,
            }),
            strikethrough: None,
        };
        let line =
            window
                .text_system()
                .shape_line(text.clone(), self.style.font_size, &[run], None);
        // Marked text that cannot be painted is shown again with the next
        // change of the composition.
        let _ = line.paint(
            origin,
            layout.cell.height,
            TextAlign::Left,
            None,
            window,
            cx,
        );
    }
}

/// The glyphs of one symbol, with their x position in the cell.
fn shape(key: &GlyphKey, font: Font, font_size: Pixels, window: &Window) -> Vec<PositionedGlyph> {
    let mut text = String::new();
    text.push(key.character);
    text.extend(key.combining.iter());
    let run = TextRun {
        len: text.len(),
        font,
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(text.into(), font_size, &[run], None);
    line.runs
        .iter()
        .flat_map(|run| {
            run.glyphs.iter().map(|glyph| PositionedGlyph {
                font_id: run.font_id,
                glyph_id: glyph.id,
                x: glyph.position.x,
                is_emoji: glyph.is_emoji,
            })
        })
        .collect()
}

impl<V: EntityInputHandler> Element for TerminalElement<V> {
    type RequestLayoutState = ();
    type PrepaintState = GridLayout;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let layout = self.grid_layout(bounds, window);
        let previous = self.caches.layout.replace(layout);
        // The view follows the grid size (the size of the session): a new
        // size renders it again.
        if (previous.rows, previous.columns) != (layout.rows, layout.columns) {
            let view = self.view.downgrade();
            cx.defer(move |cx| {
                let _ = view.update(cx, |_, cx| cx.notify());
            });
        }
        layout
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        layout: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let started = Instant::now();
        let focused = self.focus.is_focused(window);
        window.paint_quad(fill(bounds, self.style.palette.background));
        let plan = PaintPlan::build(
            &self.frame.snapshot,
            &self.style.palette,
            self.frame.selection.as_ref(),
            CursorState {
                focused,
                shown: self.frame.cursor_shown,
            },
        );
        let mut quads = 1;
        self.paint_runs(&plan.backgrounds, layout, window, &mut quads);
        self.paint_runs(&plan.selection, layout, window, &mut quads);
        if let Some(cursor) = &plan.cursor {
            self.paint_cursor(cursor, layout, window, true);
        }
        let (cache_hits, cache_misses) = self.paint_glyphs(&plan, layout, window);
        quads += self.paint_lines(&plan, layout, window);
        if let Some(cursor) = &plan.cursor {
            self.paint_cursor(cursor, layout, window, false);
            quads += 1;
        }
        self.paint_marked_text(layout, window, cx);
        window.handle_input(
            &self.focus,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );
        self.caches.stats.set(Some(PaintStats {
            painted_at: Instant::now(),
            generation: self.frame.generation,
            duration: started.elapsed(),
            quads,
            glyph_cells: plan.glyphs.len(),
            cache_hits,
            cache_misses,
        }));
    }
}
