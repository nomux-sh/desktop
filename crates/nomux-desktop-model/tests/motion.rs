//! The rules of motion: the phase of a blinking cursor and when an
//! animation may run.

use std::time::Duration;

use nomux_desktop_model::motion::{BLINK_CYCLES, MotionContext, blink_phase, working_step};

const CYCLE: Duration = Duration::from_millis(1000);

#[test]
fn a_blink_shows_the_cursor_in_the_first_half_of_each_cycle() {
    let at = |milliseconds| blink_phase(Duration::from_millis(milliseconds), CYCLE);
    assert!(at(0).shown);
    assert_eq!(at(0).next_change, Some(Duration::from_millis(500)));
    assert!(!at(500).shown);
    assert_eq!(at(700).next_change, Some(Duration::from_millis(300)));
    assert!(at(1000).shown);
}

#[test]
fn a_blink_ends_shown_after_its_cycles() {
    let end = CYCLE * BLINK_CYCLES;
    let last = blink_phase(end - Duration::from_millis(1), CYCLE);
    assert!(!last.shown);
    assert!(last.next_change.is_some());
    let ended = blink_phase(end, CYCLE);
    assert!(ended.shown);
    assert_eq!(ended.next_change, None);
}

#[test]
fn motion_runs_only_in_a_visible_window_without_reduced_motion() {
    let context = |window_visible, reduce_motion| MotionContext {
        window_visible,
        reduce_motion,
    };
    assert!(context(true, false).allows_motion());
    assert!(!context(false, false).allows_motion());
    assert!(!context(true, true).allows_motion());
}

#[test]
fn the_working_glyph_wakes_the_window_fifteen_times_for_each_turn() {
    assert_eq!(working_step(CYCLE), Duration::from_nanos(66_666_666));
}
