//! The rules of the terminal tabs and of their saved state.

mod sample;

use nomux_client::SessionState;
use nomux_desktop_model::Tabs;

fn tabs(sessions: &[&str], active: &str) -> Tabs {
    let mut tabs = Tabs::default();
    for id in sessions {
        tabs.open(id);
    }
    tabs.show(active);
    tabs
}

#[test]
fn a_click_shows_the_session_in_the_active_tab_and_a_new_tab_goes_after_it() {
    let mut tabs = tabs(&["a", "b"], "a");
    // The active tab shows `c` now: `a` is detached.
    assert_eq!(tabs.show("c"), Some("a".to_owned()));
    assert_eq!(tabs.sessions(), ["c", "b"]);
    // A session that has a tab is activated, not shown twice.
    assert_eq!(tabs.show("b"), None);
    assert_eq!(tabs.active(), Some("b"));
    tabs.activate(0);
    tabs.open("d");
    assert_eq!(tabs.sessions(), ["c", "d", "b"]);
    assert_eq!(tabs.active(), Some("d"));
}

#[test]
fn closing_the_active_tab_activates_its_right_neighbour_else_its_left_one() {
    let mut tabs = tabs(&["a", "b", "c"], "b");
    assert_eq!(tabs.close_active(), Some("b".to_owned()));
    assert_eq!(tabs.active(), Some("c"));
    tabs.close_active();
    assert_eq!(tabs.active(), Some("a"));
    tabs.close_active();
    assert_eq!(tabs.active(), None);
    assert_eq!(tabs.close_active(), None);
}

#[test]
fn closing_another_tab_keeps_the_active_one() {
    let mut tabs = tabs(&["a", "b", "c"], "c");
    assert!(tabs.close("a"));
    assert_eq!(tabs.active(), Some("c"));
    assert!(!tabs.close("x"));
}

#[test]
fn the_next_tab_wraps_and_a_moved_tab_stops_at_the_edge() {
    let mut tabs = tabs(&["a", "b", "c"], "c");
    tabs.activate_next(1);
    assert_eq!(tabs.active(), Some("a"));
    tabs.activate_next(-1);
    assert_eq!(tabs.active(), Some("c"));
    tabs.move_active(1);
    assert_eq!(tabs.sessions(), ["a", "b", "c"]);
    tabs.move_active(-1);
    assert_eq!(tabs.sessions(), ["a", "c", "b"]);
    assert_eq!(tabs.active(), Some("c"));
    tabs.activate(7);
    assert_eq!(tabs.active(), Some("c"));
    tabs.activate_last();
    assert_eq!(tabs.active(), Some("b"));
}

#[test]
fn the_tab_of_a_removed_session_closes() {
    let host = sample::host(
        vec![
            sample::session("a", "/home/user/acme-web", "bash", SessionState::Running),
            sample::session("c", "/home/user/acme-web", "bash", SessionState::Running),
        ],
        vec![],
    );
    let mut tabs = tabs(&["a", "b", "c"], "b");
    tabs.retain_existing(&host);
    assert_eq!(tabs.sessions(), ["a", "c"]);
    assert_eq!(tabs.active(), Some("c"));
}

#[test]
fn the_saved_state_gives_the_same_tabs_and_ignores_damaged_lines() {
    let tabs = tabs(&["a1", "b2", "c3"], "b2");
    assert_eq!(tabs.to_text(), "a1\n* b2\nc3\n");
    assert_eq!(Tabs::from_text(&tabs.to_text()), tabs);
    let damaged = Tabs::from_text("a1\n../etc\n\n* a1\nb2\n");
    assert_eq!(damaged.sessions(), ["a1", "b2"]);
    assert_eq!(damaged.active(), Some("a1"));
}
