//! The rules of motion: when an animation may run, the step of the working
//! glyph, and the phase of a blinking cursor.
//!
//! A clock for an animation wakes the window. Thus it runs only while the
//! window is visible, something that animates is in it, and the user did
//! not ask for reduced motion.

use std::time::Duration;

/// What the window and the system say about motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionContext {
    /// The platform shows the frames of the window: it is not hidden,
    /// minimised or behind other windows.
    pub window_visible: bool,
    /// The user asked for reduced motion.
    pub reduce_motion: bool,
}

impl MotionContext {
    /// `true` if an animation in the window may run.
    #[must_use]
    pub fn allows_motion(self) -> bool {
        self.window_visible && !self.reduce_motion
    }
}

/// The steps of one turn of the working glyph. The clock of the glyph wakes
/// the window one time for each step.
pub const WORKING_STEPS_PER_TURN: u32 = 15;

/// The interval of the clock of the working glyph, when one turn takes
/// `turn`.
#[must_use]
pub fn working_step(turn: Duration) -> Duration {
    turn / WORKING_STEPS_PER_TURN
}

/// The cycles of a blinking cursor after the last change of the screen or
/// the focus. Then the cursor stays shown: no animation repeats without an
/// end.
pub const BLINK_CYCLES: u32 = 10;

/// The phase of a blinking cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlinkPhase {
    /// `true` while the cursor is shown.
    pub shown: bool,
    /// The time to the next change of `shown`. `None` when the blink ended.
    pub next_change: Option<Duration>,
}

/// The phase of a blinking cursor `elapsed` after the last change. One
/// cycle is `cycle`: the first half shows the cursor, the second half hides
/// it.
#[must_use]
pub fn blink_phase(elapsed: Duration, cycle: Duration) -> BlinkPhase {
    let half = cycle.as_nanos() / 2;
    if half == 0 || elapsed.as_nanos() >= cycle.as_nanos() * u128::from(BLINK_CYCLES) {
        return BlinkPhase {
            shown: true,
            next_change: None,
        };
    }
    let halves = elapsed.as_nanos() / half;
    let next = (halves + 1) * half - elapsed.as_nanos();
    BlinkPhase {
        shown: halves.is_multiple_of(2),
        next_change: Some(Duration::from_nanos(
            u64::try_from(next).unwrap_or(u64::MAX),
        )),
    }
}
