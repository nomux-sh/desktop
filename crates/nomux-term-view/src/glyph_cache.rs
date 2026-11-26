//! The shaped glyphs of each symbol and style. A cell paints the glyphs of
//! its symbol with the colour of the cell, thus the colour is not a part of
//! the key and a symbol is shaped one time.

use std::collections::HashMap;
use std::rc::Rc;

use gpui::{FontId, GlyphId, Pixels};

/// The most entries. When the cache is full, it starts again empty: a
/// screen has far fewer different symbols.
const CAPACITY: usize = 4096;

/// What a glyph cache entry is for: the text of one cell and its style.
/// Without combining characters the key allocates nothing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    /// The character of the cell.
    pub character: char,
    /// The combining characters of the cell.
    pub combining: Box<[char]>,
    /// Bold text.
    pub bold: bool,
    /// Italic text.
    pub italic: bool,
}

/// One shaped glyph of a symbol.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionedGlyph {
    /// The font that has the glyph.
    pub font_id: FontId,
    /// The glyph.
    pub glyph_id: GlyphId,
    /// The x position from the left edge of the cell.
    pub x: Pixels,
    /// A colour emoji: it keeps its own colours.
    pub is_emoji: bool,
}

/// The cache, with the number of hits and misses since the last
/// [`GlyphCache::take_counts`].
#[derive(Debug, Default)]
pub struct GlyphCache {
    entries: HashMap<GlyphKey, Rc<[PositionedGlyph]>>,
    hits: usize,
    misses: usize,
}

impl GlyphCache {
    /// The glyphs of `key`. `shape` shapes them on a miss.
    pub fn get_or_shape(
        &mut self,
        key: &GlyphKey,
        shape: impl FnOnce() -> Vec<PositionedGlyph>,
    ) -> Rc<[PositionedGlyph]> {
        if let Some(glyphs) = self.entries.get(key) {
            self.hits += 1;
            return glyphs.clone();
        }
        self.misses += 1;
        if self.entries.len() >= CAPACITY {
            self.entries.clear();
        }
        let glyphs: Rc<[PositionedGlyph]> = shape().into();
        self.entries.insert(key.clone(), glyphs.clone());
        glyphs
    }

    /// Removes each entry, for example after a change of the font.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// The number of hits and misses, and starts to count again from 0.
    pub fn take_counts(&mut self) -> (usize, usize) {
        let counts = (self.hits, self.misses);
        self.hits = 0;
        self.misses = 0;
        counts
    }
}
