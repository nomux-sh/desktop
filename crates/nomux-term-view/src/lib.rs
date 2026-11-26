//! The terminal element of the nomux desktop client.
//!
//! - [`TerminalSnapshot`]: an owned copy of the screen of a
//!   [`nomux_term::Terminal`], taken once for each frame.
//! - [`PaintPlan`]: what one frame paints, from a snapshot: merged background
//!   runs, glyph cells, lines and the cursor. Plain data, without GPUI calls.
//! - [`GlyphCache`]: the shaped glyphs of each symbol and style; the colour
//!   comes at paint time.
//! - [`TerminalElement`]: the GPUI element that paints a plan.
//! - [`input`]: the bytes of keys, paste, mouse and focus for the program.
//! - [`GridSelection`]: a selection of cells and its text.
//!
//! The crate does no I/O. The caller feeds the terminal and sends the
//! bytes.

mod element;
mod glyph_cache;
pub mod input;
mod paint_plan;
mod palette;
mod selection;
mod snapshot;

pub use element::{
    GridLayout, PaintStats, TerminalCaches, TerminalElement, TerminalFrame, TerminalStyle,
};
pub use glyph_cache::{GlyphCache, GlyphKey, PositionedGlyph};
pub use paint_plan::{CellRun, CursorMark, GlyphCell, LineKind, LineRun, PaintPlan};
pub use palette::TerminalPalette;
pub use selection::{GridPoint, GridSelection};
pub use snapshot::{SnapshotCell, TerminalSnapshot};
