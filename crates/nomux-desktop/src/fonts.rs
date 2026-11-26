//! The fonts of the `font.ui` and `font.mono` tokens. The client ships
//! them, thus the text looks the same on each computer.

use std::borrow::Cow;

use gpui::App;

const FONTS: [&[u8]; 6] = [
    include_bytes!("../../../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../../../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-Italic.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-BoldItalic.ttf"),
];

/// Adds the shipped fonts to the text system.
///
/// # Errors
///
/// Returns an error if the text system cannot read a font file.
pub fn load(cx: &App) -> gpui::Result<()> {
    cx.text_system()
        .add_fonts(FONTS.iter().map(|font| Cow::Borrowed(*font)).collect())
}
