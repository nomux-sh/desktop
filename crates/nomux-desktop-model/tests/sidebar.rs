//! The rows of the sidebar, their sequence, their states and the selection.

mod sample;

use nomux_client::{AgentState, SessionState, Sourced, StatusSource};
use nomux_desktop_model::sidebar::{SessionKind, sidebar};
use nomux_desktop_model::{GlyphState, ProjectView, Selection};

const NOW_MS: u64 = 1_000_000 + 5 * 60_000;

#[test]
fn rows_that_need_the_user_come_first_and_the_others_keep_their_sequence() {
    let directory = "/home/user/acme-web";
    let started = |id: &str, started_at: u64| {
        let mut session = sample::session(id, directory, "bash", SessionState::Running);
        session.started_at = Some(started_at);
        session
    };
    // The daemon lists by identifier; the start time is the sequence.
    let host = sample::host(
        vec![
            started("a", 30),
            started("b", 10),
            started("c", 40),
            started("d", 20),
        ],
        vec![
            sample::run("c", AgentState::NeedsYou(None)),
            sample::run("b", AgentState::Working),
        ],
    );
    let mut selection = Selection::default();
    selection.reconcile(&host);
    let content = sidebar(&host, &selection, NOW_MS, None).expect("a selected Project");
    let order: Vec<&str> = content
        .rows
        .iter()
        .map(|row| row.session_id.as_str())
        .collect();
    assert_eq!(order, ["c", "b", "d", "a"]);
    assert_eq!(content.needs_you, 1);
    assert_eq!(content.title, "acme-web");
    // The first row is selected when nothing was.
    assert!(content.rows[0].selected);
}

#[test]
fn a_seen_result_is_idle_and_an_unseen_one_is_the_check() {
    let mut done = sample::status(AgentState::Done, false, StatusSource::Adapter);
    assert_eq!(GlyphState::of_status(&done), GlyphState::DoneUnseen);
    done.seen = true;
    assert_eq!(GlyphState::of_status(&done), GlyphState::Idle);
}

#[test]
fn the_priority_is_the_sequence_of_the_specification() {
    let states = [
        GlyphState::NoProcess,
        GlyphState::Idle,
        GlyphState::DoneUnseen,
        GlyphState::Working,
        GlyphState::Error,
    ];
    assert_eq!(GlyphState::most_urgent(states), Some(GlyphState::Error));
    assert_eq!(
        GlyphState::most_urgent([GlyphState::Idle, GlyphState::NoProcess]),
        Some(GlyphState::Idle)
    );
}

#[test]
fn an_agent_row_and_a_dormant_shell_row() {
    let directory = "/home/user/acme-web";
    let mut agent = sample::run("agent", AgentState::Working);
    agent.status.title = Some(Sourced {
        value: "Refactor".to_owned(),
        source: StatusSource::Adapter,
    });
    agent.status.message = Some(Sourced {
        value: "editing parser.rs".to_owned(),
        source: StatusSource::Adapter,
    });
    agent.children = vec![
        sample::child("search", AgentState::Done),
        sample::child("tests", AgentState::Error),
    ];
    let mut shell = sample::session(
        "shell",
        &format!("{directory}/src"),
        "/bin/bash",
        SessionState::Dormant,
    );
    shell.started_at = None;
    let host = sample::host(
        vec![
            sample::session("agent", directory, "claude", SessionState::Running),
            shell,
        ],
        vec![agent],
    );
    let mut selection = Selection::default();
    selection.select_project(ProjectView::Project("web".to_owned()), &host);
    let content = sidebar(&host, &selection, NOW_MS, None).expect("a selected Project");
    let agent_row = &content.rows[0];
    assert_eq!(agent_row.kind, SessionKind::Agent);
    assert_eq!(agent_row.name, "Refactor");
    assert_eq!(agent_row.secondary.as_deref(), Some("editing parser.rs"));
    assert_eq!(agent_row.children, Some((GlyphState::Error, 2)));
    assert_eq!(agent_row.age.as_deref(), Some("5m"));
    let shell_row = &content.rows[1];
    assert_eq!(shell_row.kind, SessionKind::Shell);
    assert_eq!(shell_row.name, "bash");
    assert_eq!(shell_row.state, GlyphState::NoProcess);
    assert_eq!(
        shell_row.secondary.as_deref(),
        Some("dormant · restores on open")
    );
    assert_eq!(shell_row.age, None);
}

#[test]
fn a_removed_session_gives_the_selection_to_the_first_row() {
    let directory = "/home/user/acme-web";
    let mut host = sample::host(
        vec![
            sample::session("a", directory, "bash", SessionState::Running),
            sample::session("b", directory, "bash", SessionState::Running),
        ],
        vec![],
    );
    let mut selection = Selection::default();
    selection.reconcile(&host);
    selection.select_session("b");
    host.set_sessions(vec![
        sample::session("a", directory, "bash", SessionState::Running),
        sample::session("b", directory, "bash", SessionState::Ended),
    ]);
    selection.reconcile(&host);
    assert_eq!(selection.session(), Some("a"));
}

#[test]
fn sessions_outside_each_project_have_their_own_view() {
    let host = sample::host(
        vec![sample::session(
            "home",
            "/tmp",
            "bash",
            SessionState::Running,
        )],
        vec![],
    );
    let mut selection = Selection::default();
    selection.reconcile(&host);
    assert_eq!(selection.project(), Some(&ProjectView::Unassigned));
    assert_eq!(selection.session(), Some("home"));
}
