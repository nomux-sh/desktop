//! The colours of a terminal.

use gpui::{Hsla, Rgba};
use nomux_term::Color;

/// The colours of a terminal: from the `terminal.*` tokens of the theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerminalPalette {
    /// The 16 ANSI colours.
    pub ansi: [Hsla; 16],
    /// The default text colour.
    pub foreground: Hsla,
    /// The default background.
    pub background: Hsla,
    /// The colour of the cursor.
    pub cursor: Hsla,
    /// The tint of selected cells.
    pub selection: Hsla,
}

/// The six steps of each channel of the colour cube of the 256-colour
/// palette (colours 16 to 231), as xterm defines them.
const CUBE_STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];

impl TerminalPalette {
    /// The colour of a cell colour. `Color::Default` is `default`.
    #[must_use]
    pub fn resolve(&self, color: Color, default: Hsla) -> Hsla {
        match color {
            Color::Default => default,
            Color::Indexed(index @ 0..16) => self.ansi[usize::from(index)],
            Color::Indexed(index @ 16..232) => {
                let cube = index - 16;
                rgb(
                    CUBE_STEPS[usize::from(cube / 36)],
                    CUBE_STEPS[usize::from(cube / 6 % 6)],
                    CUBE_STEPS[usize::from(cube % 6)],
                )
            }
            Color::Indexed(index) => {
                let grey = 8 + (index - 232) * 10;
                rgb(grey, grey, grey)
            }
            Color::Rgb(red, green, blue) => rgb(red, green, blue),
        }
    }
}

fn rgb(red: u8, green: u8, blue: u8) -> Hsla {
    Rgba {
        r: f32::from(red) / 255.0,
        g: f32::from(green) / 255.0,
        b: f32::from(blue) / 255.0,
        a: 1.0,
    }
    .into()
}
