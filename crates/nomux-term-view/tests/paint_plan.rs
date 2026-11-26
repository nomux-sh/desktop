//! The paint plan of fixed screens: the counts of runs and glyph cells, the
//! cursor and the selection.

use gpui::{Hsla, Rgba};
use nomux_term::{Size, Terminal};
use nomux_term_view::{
    CursorState, GridPoint, GridSelection, PaintPlan, TerminalPalette, TerminalSnapshot,
};

fn color(red: u8) -> Hsla {
    Rgba {
        r: f32::from(red) / 255.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    }
    .into()
}

fn palette() -> TerminalPalette {
    TerminalPalette {
        ansi: std::array::from_fn(|index| color(u8::try_from(index).expect("16 colours") + 1)),
        foreground: color(100),
        background: color(200),
        cursor: color(150),
        selection: color(250),
    }
}

fn snapshot(rows: u16, columns: u16, output: &str) -> TerminalSnapshot {
    let mut terminal = Terminal::new(Size::new(rows, columns).expect("a valid size"));
    terminal.feed(output.as_bytes(), |_| {});
    TerminalSnapshot::capture(&terminal.screen(), 0)
}

#[test]
fn cells_of_one_background_make_one_run() {
    // Red for 4 cells, 2 default cells, green for 2 cells; then a second row
    // that starts red again.
    let screen = snapshot(
        3,
        10,
        "\x1b[41m    \x1b[0m  \x1b[42m  \x1b[0m\r\n\x1b[41m \x1b[0m",
    );
    let plan = PaintPlan::build(&screen, &palette(), None, cursor(true));
    let runs: Vec<(u16, u16, u16)> = plan
        .backgrounds
        .iter()
        .map(|run| (run.row, run.column, run.len))
        .collect();
    assert_eq!(runs, [(0, 0, 4), (0, 6, 2), (1, 0, 1)]);
    assert_eq!(plan.backgrounds[0].color, palette().ansi[1]);
    // Spaces paint no glyph.
    assert!(plan.glyphs.is_empty());
}

#[test]
fn inverse_paints_the_text_colour_as_background_and_hidden_text_paints_nothing() {
    let screen = snapshot(2, 10, "\x1b[7mab\x1b[0m\x1b[8mcd\x1b[0m");
    let plan = PaintPlan::build(&screen, &palette(), None, cursor(false));
    assert_eq!(plan.backgrounds.len(), 1);
    assert_eq!(plan.backgrounds[0].len, 2);
    assert_eq!(plan.backgrounds[0].color, palette().foreground);
    let columns: Vec<u16> = plan.glyphs.iter().map(|glyph| glyph.column).collect();
    assert_eq!(columns, [0, 1]);
    assert_eq!(plan.glyphs[0].color, palette().background);
}

#[test]
fn a_wide_character_paints_one_glyph_and_the_cursor_covers_two_columns() {
    let screen = snapshot(2, 10, "a漢字b\x1b[1;2H");
    let plan = PaintPlan::build(&screen, &palette(), None, cursor(true));
    let columns: Vec<u16> = plan.glyphs.iter().map(|glyph| glyph.column).collect();
    assert_eq!(columns, [0, 1, 3, 5]);
    let cursor = plan.cursor.expect("a visible cursor");
    assert_eq!((cursor.row, cursor.column, cursor.width), (0, 1, 2));
    // The glyph below a focused block cursor takes the background colour.
    assert_eq!(plan.glyphs[1].color, palette().background);
}

#[test]
fn a_hidden_cursor_is_not_painted_and_lines_merge_into_runs() {
    let screen = snapshot(2, 10, "\x1b[?25l\x1b[4mabc\x1b[0m d\x1b[9mef\x1b[0m");
    let plan = PaintPlan::build(&screen, &palette(), None, cursor(true));
    assert!(plan.cursor.is_none());
    let lines: Vec<(u16, u16)> = plan
        .lines
        .iter()
        .map(|line| (line.cells.column, line.cells.len))
        .collect();
    assert_eq!(lines, [(0, 3), (5, 2)]);
}

#[test]
fn a_selection_paints_one_run_for_each_row_and_copies_trimmed_lines() {
    let screen = snapshot(3, 8, "first   \r\nsecond\r\nthird");
    let selection = GridSelection {
        anchor: GridPoint { row: 2, column: 2 },
        head: GridPoint { row: 0, column: 3 },
    };
    let plan = PaintPlan::build(&screen, &palette(), Some(&selection), cursor(true));
    let runs: Vec<(u16, u16, u16)> = plan
        .selection
        .iter()
        .map(|run| (run.row, run.column, run.len))
        .collect();
    assert_eq!(runs, [(0, 3, 5), (1, 0, 8), (2, 0, 3)]);
    assert_eq!(selection.text(&screen), "st\nsecond\nthi");
}

#[test]
fn the_view_shows_history_rows_above_the_screen() {
    let mut terminal = Terminal::with_history(Size::new(2, 8).expect("a valid size"), 100);
    terminal.feed(b"one\r\ntwo\r\nthree\r\nfour", |_| {});
    let view = TerminalSnapshot::capture(&terminal.screen(), 1);
    let text = |row| -> String { view.row(row).iter().map(|cell| cell.character).collect() };
    assert_eq!(view.history_rows(), 2);
    assert_eq!(text(0).trim_end(), "two");
    assert_eq!(text(1).trim_end(), "three");
    // The cursor is on "four", below the view.
    assert!(view.visible_cursor().is_none());
    // A scroll past the history stops at its oldest row.
    let clamped = TerminalSnapshot::capture(&terminal.screen(), 10);
    assert_eq!(clamped.scrolled(), 2);
}

fn cursor(focused: bool) -> CursorState {
    CursorState {
        focused,
        shown: true,
    }
}
