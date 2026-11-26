//! The state glyphs and the open mark, drawn in a view box of 16 × 16
//! units: the paths of the section "State glyphs" of the specification.

use std::f32::consts::{PI, TAU};
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{
    Bounds, Hsla, IntoElement, LineCap, LineJoin, PathBuilder, PathStyle, Pixels, Point,
    StrokeOptions, Styled, Window, canvas, fill, point, px, quad, size,
};
use nomux_desktop_model::GlyphState;
use nomux_desktop_model::motion::WORKING_STEPS_PER_TURN;
use nomux_desktop_theme::{ColorToken, DesktopTheme, DurationToken};

/// The side of the view box of a glyph.
const VIEW_BOX: f32 = 16.0;
/// The stroke of the open shapes, in units of the view box.
const STROKE: f32 = 3.0;

/// A mark: a state glyph or the open mark of a Group tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// The glyph of a state.
    State(GlyphState),
    /// The open caret of an open Group.
    Open,
}

/// A mark of `side` pixels in the colour of its state. With `turning`, the
/// working glyph has the angle of the current time; without it, the static
/// arc. The glyph asks for no frame: a clock of the caller paints it again.
pub fn mark(mark: Mark, side: Pixels, theme: &DesktopTheme, turning: bool) -> impl IntoElement {
    let color = match mark {
        Mark::State(state) => theme.color(state_color(state)),
        Mark::Open => theme.color(ColorToken::TextMuted),
    };
    let turn = theme.duration(DurationToken::MotionIndicator).as_secs_f32();
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let turn = turning.then_some(turn);
            paint_mark(mark, bounds, color, turn, window);
        },
    )
    .size(side)
    .flex_none()
}

/// The colour token of a state: the dash of a session without a process has
/// the colour of idle.
fn state_color(state: GlyphState) -> ColorToken {
    match state {
        GlyphState::NeedsYou => ColorToken::StateNeedsYou,
        GlyphState::Error => ColorToken::StateError,
        GlyphState::Working => ColorToken::StateWorking,
        GlyphState::DoneUnseen => ColorToken::StateDone,
        GlyphState::Idle | GlyphState::NoProcess => ColorToken::StateIdle,
    }
}

fn paint_mark(
    mark: Mark,
    bounds: Bounds<Pixels>,
    color: Hsla,
    turn: Option<f32>,
    window: &mut Window,
) {
    let unit = bounds.size.width / VIEW_BOX;
    let at = |x: f32, y: f32| bounds.origin + point(unit * x, unit * y);
    match mark {
        Mark::State(GlyphState::NeedsYou) => {
            window
                .paint_quad(fill(circle(at(8.0, 8.0), unit * 6.5), color).corner_radii(unit * 6.5));
        }
        Mark::State(GlyphState::Idle) => {
            let outer = unit * (5.5 + STROKE / 2.0);
            window.paint_quad(quad(
                circle(at(8.0, 8.0), outer),
                outer,
                gpui::transparent_black(),
                unit * STROKE,
                color,
                gpui::BorderStyle::Solid,
            ));
        }
        Mark::State(GlyphState::Error) => {
            stroke(&[at(3.5, 3.5), at(12.5, 12.5)], unit, color, window);
            stroke(&[at(12.5, 3.5), at(3.5, 12.5)], unit, color, window);
        }
        Mark::State(GlyphState::DoneUnseen) => {
            stroke(
                &[at(2.5, 8.5), at(6.0, 12.0), at(13.5, 4.0)],
                unit,
                color,
                window,
            );
        }
        Mark::State(GlyphState::NoProcess) => {
            stroke(&[at(3.0, 8.0), at(13.0, 8.0)], unit, color, window);
        }
        Mark::Open => {
            stroke(
                &[at(3.0, 6.0), at(8.0, 11.0), at(13.0, 6.0)],
                unit,
                color,
                window,
            );
        }
        Mark::State(GlyphState::Working) => {
            paint_working_arc(at(8.0, 8.0), unit, color, turn, window);
        }
    }
}

/// The arc of three quarters. With `turn` (the time of one turn,
/// `motion.indicator`), turned clockwise by the time; else the static arc.
fn paint_working_arc(
    center: Point<Pixels>,
    unit: Pixels,
    color: Hsla,
    turn: Option<f32>,
    window: &mut Window,
) {
    // The angle of the current step, thus each tick of the clock shows the
    // next quarter turn.
    let angle = turn.map_or(0.0, |turn| {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |elapsed| elapsed.as_secs_f32() % turn);
        let steps = WORKING_STEPS_PER_TURN as f32;
        (seconds / turn * steps).floor() / steps * TAU
    });
    let radius = unit * 6.0;
    // The static arc starts at the top and ends at the left: M8 2a6 6 0 1 1-6 6.
    let start = on_circle(center, radius, -PI / 2.0 + angle);
    let end = on_circle(center, radius, PI + angle);
    let mut path = stroke_builder(unit);
    path.move_to(start);
    path.arc_to(point(radius, radius), px(0.0), true, true, end);
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

fn on_circle(center: Point<Pixels>, radius: Pixels, angle: f32) -> Point<Pixels> {
    center + point(radius * angle.cos(), radius * angle.sin())
}

fn circle(center: Point<Pixels>, radius: Pixels) -> Bounds<Pixels> {
    Bounds::new(
        center - point(radius, radius),
        size(radius * 2.0, radius * 2.0),
    )
}

fn stroke_builder(unit: Pixels) -> PathBuilder {
    PathBuilder::stroke(unit * STROKE).with_style(PathStyle::Stroke(StrokeOptions {
        line_width: (unit * STROKE).into(),
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..StrokeOptions::default()
    }))
}

fn stroke(points: &[Point<Pixels>], unit: Pixels, color: Hsla, window: &mut Window) {
    let mut path = stroke_builder(unit);
    let Some((first, rest)) = points.split_first() else {
        return;
    };
    path.move_to(*first);
    for point in rest {
        path.line_to(*point);
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
