//! The events of a host change the copy of the client and say what to load
//! again.

mod sample;

use nomux_client::{AgentState, EventBody, HostEvent, SessionState, WorkspaceChange};
use nomux_desktop_model::{GlyphState, Reload};

fn event(body: EventBody) -> HostEvent {
    HostEvent {
        seq: 1,
        at_ms: 0,
        body,
    }
}

#[test]
fn a_run_of_an_unknown_session_loads_the_sessions_again() {
    let mut host = sample::host(
        vec![sample::session(
            "a",
            "/home/user/acme-web",
            "bash",
            SessionState::Running,
        )],
        vec![],
    );
    let known = host.apply_event(&event(EventBody::AgentRunChanged(Box::new(sample::run(
        "a",
        AgentState::Working,
    )))));
    assert!(known.is_empty());
    let unknown = host.apply_event(&event(EventBody::AgentRunChanged(Box::new(sample::run(
        "b",
        AgentState::Working,
    )))));
    assert_eq!(
        unknown,
        Reload {
            sessions: true,
            ..Reload::default()
        }
    );
}

#[test]
fn a_removed_run_loads_the_sessions_again_and_clears_the_state() {
    let session = sample::session("a", "/home/user/acme-web", "bash", SessionState::Running);
    let mut host = sample::host(
        vec![session.clone()],
        vec![sample::run("a", AgentState::NeedsYou(None))],
    );
    assert_eq!(
        GlyphState::of_session(&session, host.run("a")),
        GlyphState::NeedsYou
    );
    let reload = host.apply_event(&event(EventBody::AgentRunRemoved {
        session_id: "a".to_owned(),
    }));
    assert!(reload.sessions);
    assert!(host.run("a").is_none());
}

#[test]
fn a_missed_workspace_change_loads_the_tree_again() {
    let mut host = sample::host(vec![], vec![]);
    let rename = WorkspaceChange::Renamed {
        id: "web".to_owned(),
        name: "storefront".to_owned(),
    };
    let next = host.apply_event(&event(EventBody::WorkspaceChanged {
        revision: 2,
        change: rename.clone(),
    }));
    assert!(next.is_empty());
    assert_eq!(
        host.workspace()
            .tree
            .get("web")
            .map(|node| node.name.as_str()),
        Some("storefront")
    );
    let gap = host.apply_event(&event(EventBody::WorkspaceChanged {
        revision: 4,
        change: rename,
    }));
    assert_eq!(
        gap,
        Reload {
            workspace: true,
            ..Reload::default()
        }
    );
}

#[test]
fn a_snapshot_loads_everything_again() {
    let mut host = sample::host(vec![], vec![]);
    assert_eq!(host.apply_event(&event(EventBody::Snapshot)), Reload::ALL);
}
