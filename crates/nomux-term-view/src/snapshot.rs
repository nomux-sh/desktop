//! An owned copy of the rows that a terminal view shows.

use nomux_term::{Cell, CellWidth, Cursor, Modes, Position, Screen, Style};

/// One cell of a snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotCell {
    /// The character. An empty cell has a space.
    pub character: char,
    /// The combining characters after [`SnapshotCell::character`]. Empty for
    /// almost each cell, thus it allocates nothing.
    pub combining: Box<[char]>,
    /// The number of columns of the character.
    pub width: CellWidth,
    /// The colours and the attributes.
    pub style: Style,
}

/// The rows that a view shows at one moment, the cursor and the modes.
///
/// A view that scrolled up into the history shows history rows above the
/// first rows of the screen; the snapshot copies only the rows that the
/// view shows, never the whole history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSnapshot {
    columns: u16,
    rows: u16,
    scrolled: usize,
    history_rows: usize,
    cells: Vec<SnapshotCell>,
    cursor: Cursor,
    modes: Modes,
}

impl TerminalSnapshot {
    /// Copies the rows of the view: the screen, moved down by `scrolled`
    /// rows of history. `scrolled` is clamped to the history.
    #[must_use]
    pub fn capture(screen: &Screen<'_>, scrolled: usize) -> Self {
        let size = screen.size();
        let history_rows = screen.history_rows();
        let scrolled = scrolled.min(history_rows);
        let mut cells = Vec::with_capacity(usize::from(size.rows()) * usize::from(size.columns()));
        for row in 0..size.rows() {
            let rows_above = scrolled
                .checked_sub(usize::from(row))
                .filter(|above| *above > 0);
            for column in 0..size.columns() {
                let cell = match rows_above {
                    Some(rows_above) => screen.history_cell(rows_above, column),
                    None => {
                        let screen_row = usize::from(row) - scrolled;
                        u16::try_from(screen_row)
                            .ok()
                            .and_then(|row| screen.cell(Position { row, column }))
                    }
                };
                cells.push(cell.map_or_else(blank_cell, owned_cell));
            }
        }
        Self {
            columns: size.columns(),
            rows: size.rows(),
            scrolled,
            history_rows,
            cells,
            cursor: screen.cursor(),
            modes: screen.modes(),
        }
    }

    /// The number of columns.
    #[must_use]
    pub fn columns(&self) -> u16 {
        self.columns
    }

    /// The number of rows of the view.
    #[must_use]
    pub fn screen_rows(&self) -> u16 {
        self.rows
    }

    /// The number of history rows above the first row of the view's
    /// screen: 0 when the view shows the screen.
    #[must_use]
    pub fn scrolled(&self) -> usize {
        self.scrolled
    }

    /// The number of rows of the history of the terminal.
    #[must_use]
    pub fn history_rows(&self) -> usize {
        self.history_rows
    }

    /// The cell at a row of the view and a column.
    #[must_use]
    pub fn cell(&self, row: u16, column: u16) -> Option<&SnapshotCell> {
        if row >= self.rows || column >= self.columns {
            return None;
        }
        self.cells
            .get(usize::from(row) * usize::from(self.columns) + usize::from(column))
    }

    /// The cells of one row of the view.
    #[must_use]
    pub fn row(&self, row: u16) -> &[SnapshotCell] {
        let start = usize::from(row) * usize::from(self.columns);
        self.cells
            .get(start..start + usize::from(self.columns))
            .unwrap_or(&[])
    }

    /// The cursor on the view: its row moves down with the scroll. `None`
    /// if the program hides it or the scroll moved it below the view.
    #[must_use]
    pub fn visible_cursor(&self) -> Option<Cursor> {
        let mut cursor = self.cursor;
        let row = usize::from(cursor.position.row) + self.scrolled;
        cursor.position.row = u16::try_from(row).ok().filter(|row| *row < self.rows)?;
        cursor.visible.then_some(cursor)
    }

    /// The cursor of the screen, without the scroll.
    #[must_use]
    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// The modes that the program set.
    #[must_use]
    pub fn modes(&self) -> Modes {
        self.modes
    }
}

fn owned_cell(cell: Cell<'_>) -> SnapshotCell {
    SnapshotCell {
        character: cell.character,
        combining: cell.combining.into(),
        width: cell.width,
        style: cell.style,
    }
}

fn blank_cell() -> SnapshotCell {
    SnapshotCell {
        character: ' ',
        combining: Box::default(),
        width: CellWidth::Single,
        style: Style::default(),
    }
}
