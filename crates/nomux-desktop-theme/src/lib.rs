//! The theme of the desktop client: a shipped nomux theme, with each token
//! as a GPUI value. The client has no colour and no length of its own: each
//! one comes from a token.

use std::time::Duration;

use gpui::{Font, FontFeatures, FontStyle, FontWeight, Hsla, Pixels, Rgba, px};
pub use nomux_theme::{ColorToken, DurationToken, LengthToken, TextStyle};
use nomux_theme::{Density, DensityMetrics, Theme, shipped};

/// The theme of a dark appearance.
pub const DARK_THEME_ID: &str = "nomux-dark";
/// The theme of a light appearance.
pub const LIGHT_THEME_ID: &str = "nomux-light";
/// The density of slice 1. The configuration of the user selects it later.
pub const DENSITY: Density = Density::Comfortable;

/// The resolved tokens of one theme.
#[derive(Debug, Clone, Copy)]
pub struct DesktopTheme {
    theme: &'static Theme,
}

/// The metrics of the density, in pixels.
#[derive(Debug, Clone, Copy)]
pub struct Metrics {
    /// The height of a row.
    pub row_height: Pixels,
    /// The height of a control and the size of a rail tile.
    pub control_height: Pixels,
    /// The horizontal padding of rows and controls.
    pub padding: Pixels,
    /// The height of a row with a second line.
    pub two_line_row_height: Pixels,
}

impl DesktopTheme {
    /// The nomux theme for a dark or a light appearance.
    ///
    /// # Panics
    ///
    /// Panics if the theme crate does not ship the nomux themes, which its
    /// own tests exclude.
    #[must_use]
    pub fn for_appearance(dark: bool) -> Self {
        let id = if dark { DARK_THEME_ID } else { LIGHT_THEME_ID };
        Self {
            theme: shipped::theme(id).expect("nomux-theme ships the nomux themes"),
        }
    }

    /// `true` for a dark theme.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.theme.id() == DARK_THEME_ID
    }

    /// A colour token.
    #[must_use]
    pub fn color(&self, token: ColorToken) -> Hsla {
        to_hsla(self.theme.color(token))
    }

    /// A length token.
    #[must_use]
    pub fn length(&self, token: LengthToken) -> Pixels {
        px(self.theme.length(token).pixels())
    }

    /// A duration token.
    #[must_use]
    pub fn duration(&self, token: DurationToken) -> Duration {
        self.theme.duration(token)
    }

    /// The font of one step of the type scale: the first family of the
    /// token. The client ships that family.
    #[must_use]
    pub fn font(&self, style: TextStyle) -> Font {
        let spec = self.theme.font(style);
        Font {
            family: spec
                .family
                .names()
                .first()
                .cloned()
                .unwrap_or_default()
                .into(),
            features: FontFeatures::default(),
            fallbacks: None,
            weight: FontWeight(f32::from(spec.weight.value())),
            style: FontStyle::Normal,
        }
    }

    /// The font size of one step of the type scale.
    #[must_use]
    pub fn font_size(&self, style: TextStyle) -> Pixels {
        px(self.theme.font(style).size.pixels())
    }

    /// The height of a terminal row as a multiple of `font.code.size`: the
    /// value of the terminal of the preview page (`line-height: 1.5`).
    /// `nomux-theme` 0.3.1 has no token for it.
    #[must_use]
    pub fn terminal_line_height(&self) -> f32 {
        1.5
    }

    /// The metrics of the density of the client.
    #[must_use]
    pub fn metrics(&self) -> Metrics {
        let DensityMetrics {
            row_height,
            control_height,
            padding,
            two_line_row_height,
        } = self.theme.density(DENSITY);
        Metrics {
            row_height: px(row_height.pixels()),
            control_height: px(control_height.pixels()),
            padding: px(padding.pixels()),
            two_line_row_height: px(two_line_row_height.pixels()),
        }
    }

    /// The colour of a strip of an open Group at `depth` (1 or more):
    /// `depth` layers of `fill.level` over `surface.sunken`, at most 5.
    #[must_use]
    pub fn strip_fill(&self, depth: usize) -> Hsla {
        let levels = nomux_theme::rules::levels(self.theme);
        to_hsla(levels[depth.min(levels.len() - 1)])
    }
}

fn to_hsla(color: nomux_theme::Rgba) -> Hsla {
    Rgba {
        r: f32::from(color.red) / 255.0,
        g: f32::from(color.green) / 255.0,
        b: f32::from(color.blue) / 255.0,
        a: f32::from(color.alpha) / 255.0,
    }
    .into()
}
