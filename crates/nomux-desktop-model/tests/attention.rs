//! The order of the attention list and the jump to the next session that
//! needs the user.

mod sample;

use nomux_client::{AgentState, SessionState};
use nomux_desktop_model::GlyphState::{
    self, DoneUnseen, Error, Idle, NeedsYou, NoProcess, Working,
};
use nomux_desktop_model::attention::{Attention, attention, attention_order, next_needing_you};
use nomux_desktop_model::sidebar::sidebar;
use nomux_desktop_model::{Selection, Tabs};

fn item(session_id: &str, state: GlyphState, since_ms: u64) -> Attention {
    Attention {
        session_id: session_id.to_owned(),
        state,
        since_ms,
    }
}

fn ids(list: &[Attention]) -> Vec<&str> {
    list.iter().map(|item| item.session_id.as_str()).collect()
}

#[test]
fn the_order_is_needs_you_then_done_not_seen_then_working() {
    let cases: [(&str, Vec<Attention>, Vec<&str>); 6] = [
        ("empty", vec![], vec![]),
        (
            "the groups",
            vec![
                item("working", Working, 1),
                item("done", DoneUnseen, 2),
                item("needs", NeedsYou, 3),
            ],
            vec!["needs", "done", "working"],
        ),
        (
            "ties go by the time: the oldest first",
            vec![
                item("late", NeedsYou, 30),
                item("early", NeedsYou, 10),
                item("middle", NeedsYou, 20),
            ],
            vec!["early", "middle", "late"],
        ),
        (
            "the same time goes by the identifier",
            vec![item("b", Working, 5), item("a", Working, 5)],
            vec!["a", "b"],
        ),
        (
            "states that are not in the list",
            vec![
                item("error", Error, 1),
                item("idle", Idle, 1),
                item("dormant", NoProcess, 1),
                item("working", Working, 9),
            ],
            vec!["working"],
        ),
        (
            "a late needs-you is before an old working",
            vec![item("old", Working, 1), item("new", NeedsYou, 99)],
            vec!["new", "old"],
        ),
    ];
    for (name, input, expected) in cases {
        let ordered = attention_order(input);
        assert_eq!(ids(&ordered), expected, "{name}");
    }
}

#[test]
fn the_order_does_not_depend_on_the_input_order() {
    let items = vec![
        item("a", NeedsYou, 2),
        item("b", NeedsYou, 2),
        item("c", DoneUnseen, 1),
        item("d", Working, 1),
    ];
    let mut reversed = items.clone();
    reversed.reverse();
    assert_eq!(attention_order(items), attention_order(reversed));
}

#[test]
fn the_jump_goes_to_the_next_session_that_needs_you_and_wraps_around() {
    let list = attention_order(vec![
        item("a", NeedsYou, 1),
        item("b", NeedsYou, 2),
        item("c", NeedsYou, 3),
        item("done", DoneUnseen, 1),
    ]);
    let cases = [
        (None, Some("a")),
        (Some("a"), Some("b")),
        (Some("b"), Some("c")),
        (Some("c"), Some("a")),
        // The current session is not one that needs you: the first one.
        (Some("done"), Some("a")),
        (Some("unknown"), Some("a")),
    ];
    for (current, expected) in cases {
        assert_eq!(next_needing_you(&list, current), expected, "{current:?}");
    }
}

#[test]
fn the_jump_has_no_target_when_no_session_needs_you() {
    let list = attention_order(vec![item("done", DoneUnseen, 1), item("w", Working, 2)]);
    assert_eq!(next_needing_you(&list, None), None);
    assert_eq!(next_needing_you(&[], Some("a")), None);
}

#[test]
fn the_list_has_the_sessions_of_all_projects() {
    let web = "/home/user/acme-web";
    let globex = "/home/user/globex-api";
    let host = sample::host(
        vec![
            sample::session("web-idle", web, "bash", SessionState::Running),
            sample::session("web-work", web, "claude", SessionState::Running),
            sample::session("globex-ask", globex, "claude", SessionState::Running),
            sample::session("ended", globex, "claude", SessionState::Ended),
        ],
        vec![
            sample::run("web-work", AgentState::Working),
            sample::run("globex-ask", AgentState::NeedsYou(None)),
            sample::run("ended", AgentState::NeedsYou(None)),
        ],
    );
    let list = attention(&host);
    assert_eq!(ids(&list), ["globex-ask", "web-work"]);
}

#[test]
fn a_dormant_session_is_not_in_the_list() {
    let host = sample::host(
        vec![sample::session(
            "a",
            "/home/user/acme-web",
            "claude",
            SessionState::Dormant,
        )],
        vec![sample::run("a", AgentState::NeedsYou(None))],
    );
    assert!(attention(&host).is_empty());
}

#[test]
fn the_sidebar_has_the_rows_of_the_list_and_none_when_it_is_empty() {
    let web = "/home/user/acme-web";
    let host = sample::host(
        vec![
            sample::session("a", web, "claude", SessionState::Running),
            sample::session(
                "b",
                "/home/user/globex-api",
                "claude",
                SessionState::Running,
            ),
        ],
        vec![sample::run("b", AgentState::NeedsYou(None))],
    );
    let mut selection = Selection::default();
    selection.reconcile(&host);
    let content =
        sidebar(&host, &selection, &Tabs::default(), 0, None).expect("a selected Project");
    // The shown Project is acme-web, the attention row is in globex-api.
    assert_eq!(content.rows.len(), 1);
    let attention_ids: Vec<&str> = content
        .attention
        .iter()
        .map(|row| row.session_id.as_str())
        .collect();
    assert_eq!(attention_ids, ["b"]);

    let quiet = sample::host(
        vec![sample::session("a", web, "bash", SessionState::Running)],
        vec![],
    );
    let content =
        sidebar(&quiet, &selection, &Tabs::default(), 0, None).expect("a selected Project");
    assert!(content.attention.is_empty());
}
