//! A selection of cells, from the cell where the drag started to the cell
//! below the pointer, in the sequence of reading.

use nomux_term::CellWidth;

use crate::snapshot::TerminalSnapshot;

/// A cell position on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GridPoint {
    /// The row: 0 is the top row of the screen.
    pub row: u16,
    /// The column.
    pub column: u16,
}

/// A selection of cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridSelection {
    /// The cell where the selection started.
    pub anchor: GridPoint,
    /// The cell where the selection ends now.
    pub head: GridPoint,
}

impl GridSelection {
    /// The first and the last selected cell.
    #[must_use]
    pub fn ordered(&self) -> (GridPoint, GridPoint) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    /// The selected columns of `row` as `start..end`, or `None`.
    #[must_use]
    pub fn columns_of(&self, row: u16, columns: u16) -> Option<std::ops::Range<u16>> {
        let (start, end) = self.ordered();
        if row < start.row || row > end.row {
            return None;
        }
        let first = if row == start.row { start.column } else { 0 };
        let last = if row == end.row {
            end.column.min(columns.saturating_sub(1))
        } else {
            columns.saturating_sub(1)
        };
        (first <= last).then(|| first..last + 1)
    }

    /// The text of the selected cells: one line for each row, without the
    /// spaces at the end of a row.
    #[must_use]
    pub fn text(&self, snapshot: &TerminalSnapshot) -> String {
        let (start, end) = self.ordered();
        let mut lines = Vec::new();
        for row in start.row..=end.row.min(snapshot.screen_rows().saturating_sub(1)) {
            let Some(range) = self.columns_of(row, snapshot.columns()) else {
                continue;
            };
            let mut line = String::new();
            for cell in &snapshot.row(row)[usize::from(range.start)..usize::from(range.end)] {
                if cell.width == CellWidth::Continuation {
                    continue;
                }
                line.push(cell.character);
                line.extend(cell.combining.iter());
            }
            lines.push(line.trim_end().to_owned());
        }
        lines.join("\n")
    }
}
