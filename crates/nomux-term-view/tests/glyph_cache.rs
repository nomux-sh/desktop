//! A symbol is shaped one time for each style; the colour is not a part of
//! the key.

use std::cell::Cell;

use gpui::{FontId, GlyphId, px};
use nomux_term_view::{GlyphCache, GlyphKey, PositionedGlyph};

fn key(character: char, bold: bool) -> GlyphKey {
    GlyphKey {
        character,
        combining: Box::default(),
        bold,
        italic: false,
    }
}

#[test]
fn repeated_symbols_hit_and_a_new_style_misses() {
    let mut cache = GlyphCache::default();
    let shaped = Cell::new(0);
    let shape = || {
        shaped.set(shaped.get() + 1);
        vec![PositionedGlyph {
            font_id: FontId(0),
            glyph_id: GlyphId(7),
            x: px(0.0),
            is_emoji: false,
        }]
    };
    let screen = [
        key('a', false),
        key('b', false),
        key('a', false),
        key('a', true),
        key('b', false),
    ];
    for symbol in &screen {
        cache.get_or_shape(symbol, shape);
    }
    assert_eq!(cache.take_counts(), (2, 3));
    assert_eq!(shaped.get(), 3);
    // The counts start again; the entries stay.
    cache.get_or_shape(&key('a', false), shape);
    assert_eq!(cache.take_counts(), (1, 0));
}
