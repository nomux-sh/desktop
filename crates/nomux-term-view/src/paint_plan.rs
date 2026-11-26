//! What one frame of a terminal paints, as plain data.

use gpui::Hsla;
use nomux_term::{Attributes, CellWidth, CursorShape, Underline};

use crate::palette::TerminalPalette;
use crate::selection::GridSelection;
use crate::snapshot::TerminalSnapshot;

/// The opacity of the text colour of a cell with the attribute `DIM`.
const DIM_OPACITY: f32 = 0.6;

/// Cells of one row with one colour: a background, a selection or a line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellRun {
    /// The row on the screen.
    pub row: u16,
    /// The first column.
    pub column: u16,
    /// The number of columns.
    pub len: u16,
    /// The colour.
    pub color: Hsla,
}

/// A cell that shows a symbol.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphCell {
    /// The row on the screen.
    pub row: u16,
    /// The column.
    pub column: u16,
    /// The text colour.
    pub color: Hsla,
    /// Bold text.
    pub bold: bool,
    /// Italic text.
    pub italic: bool,
}

/// The kind of a line through or below text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// A line below the text. Each underline style is drawn as one straight
    /// line in this version.
    Underline,
    /// A line through the text.
    Strikethrough,
}

/// A line through or below cells of one row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineRun {
    /// The kind of line.
    pub kind: LineKind,
    /// The cells and the colour.
    pub cells: CellRun,
}

/// The cursor that a frame paints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorMark {
    /// The row on the screen.
    pub row: u16,
    /// The column.
    pub column: u16,
    /// The number of columns: 2 on a wide character.
    pub width: u16,
    /// The shape that the program selected.
    pub shape: CursorShape,
    /// `false` when the terminal has no focus: a block is then an outline.
    pub focused: bool,
}

/// What one frame paints, in the sequence of painting.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PaintPlan {
    /// The backgrounds that are not the default background, merged into
    /// runs of one colour.
    pub backgrounds: Vec<CellRun>,
    /// The selected cells, merged into one run for each row.
    pub selection: Vec<CellRun>,
    /// The cells that show a symbol.
    pub glyphs: Vec<GlyphCell>,
    /// The underlines and strikethroughs, merged into runs.
    pub lines: Vec<LineRun>,
    /// The cursor, if the program shows it.
    pub cursor: Option<CursorMark>,
}

impl PaintPlan {
    /// The plan of a snapshot.
    #[must_use]
    pub fn build(
        snapshot: &TerminalSnapshot,
        palette: &TerminalPalette,
        selection: Option<&GridSelection>,
        focused: bool,
    ) -> Self {
        let mut plan = Self::default();
        let cursor = snapshot.visible_cursor();
        let block_cursor_cell = cursor
            .filter(|cursor| focused && cursor.shape == CursorShape::Block)
            .map(|cursor| (cursor.position.row, cursor.position.column));
        for row in 0..snapshot.screen_rows() {
            for (column, cell) in (0_u16..).zip(snapshot.row(row)) {
                let attributes = cell.style.attributes;
                let mut foreground = palette.resolve(cell.style.foreground, palette.foreground);
                let mut background = palette.resolve(cell.style.background, palette.background);
                if attributes.contains(Attributes::INVERSE) {
                    std::mem::swap(&mut foreground, &mut background);
                }
                if attributes.contains(Attributes::DIM) {
                    foreground.a *= DIM_OPACITY;
                }
                if background != palette.background {
                    push_run(&mut plan.backgrounds, row, column, background);
                }
                // The cell after a wide character has no symbol of its own.
                let has_symbol = cell.character != ' ' || !cell.combining.is_empty();
                if has_symbol && !attributes.contains(Attributes::HIDDEN) {
                    let color = if block_cursor_cell == Some((row, column)) {
                        background
                    } else {
                        foreground
                    };
                    plan.glyphs.push(GlyphCell {
                        row,
                        column,
                        color,
                        bold: attributes.contains(Attributes::BOLD),
                        italic: attributes.contains(Attributes::ITALIC),
                    });
                }
                if cell.style.underline != Underline::None {
                    let color = palette.resolve(cell.style.underline_color, foreground);
                    push_line(&mut plan.lines, LineKind::Underline, row, column, color);
                }
                if attributes.contains(Attributes::STRIKETHROUGH) {
                    push_line(
                        &mut plan.lines,
                        LineKind::Strikethrough,
                        row,
                        column,
                        foreground,
                    );
                }
            }
            if let Some(columns) =
                selection.and_then(|selection| selection.columns_of(row, snapshot.columns()))
            {
                plan.selection.push(CellRun {
                    row,
                    column: columns.start,
                    len: columns.end - columns.start,
                    color: palette.selection,
                });
            }
        }
        if let Some(cursor) = cursor {
            let wide = snapshot
                .cell(cursor.position.row, cursor.position.column)
                .is_some_and(|cell| cell.width == CellWidth::Double);
            plan.cursor = Some(CursorMark {
                row: cursor.position.row,
                column: cursor.position.column,
                width: if wide { 2 } else { 1 },
                shape: cursor.shape,
                focused,
            });
        }
        plan
    }
}

/// Adds one cell to the last run if it continues it, else starts a run.
fn push_run(runs: &mut Vec<CellRun>, row: u16, column: u16, color: Hsla) {
    if let Some(last) = runs.last_mut()
        && last.row == row
        && last.column + last.len == column
        && last.color == color
    {
        last.len += 1;
        return;
    }
    runs.push(CellRun {
        row,
        column,
        len: 1,
        color,
    });
}

fn push_line(lines: &mut Vec<LineRun>, kind: LineKind, row: u16, column: u16, color: Hsla) {
    if let Some(last) = lines.last_mut()
        && last.kind == kind
        && last.cells.row == row
        && last.cells.column + last.cells.len == column
        && last.cells.color == color
    {
        last.cells.len += 1;
        return;
    }
    lines.push(LineRun {
        kind,
        cells: CellRun {
            row,
            column,
            len: 1,
            color,
        },
    });
}
