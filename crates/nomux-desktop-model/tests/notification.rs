//! The rules of the system notifications.

mod sample;

use nomux_client::{
    AgentRun, AgentState, EventBody, HostEvent, NeedsYouKind, SessionState, Sourced, StatusSource,
};
use nomux_desktop_model::HostState;
use nomux_desktop_model::notification::{NoticeKind, NotificationSettings, Notifier, Viewing};

const START_MS: u64 = 1_000_000;

fn host() -> HostState {
    sample::host(
        vec![
            sample::session("a", "/home/user/acme-web", "claude", SessionState::Running),
            sample::session("b", "/home/user/acme-web", "claude", SessionState::Running),
        ],
        vec![],
    )
}

fn changed(run: AgentRun) -> HostEvent {
    HostEvent {
        seq: 1,
        at_ms: START_MS + 10,
        body: EventBody::AgentRunChanged(Box::new(run)),
    }
}

fn run(session_id: &str, state: AgentState, since_ms: u64) -> AgentRun {
    let mut run = sample::run(session_id, state);
    run.status.since_ms = since_ms;
    run.aggregate = run.status.clone();
    run
}

fn away() -> Viewing<'static> {
    Viewing {
        window_active: false,
        session: None,
    }
}

fn notifier() -> Notifier {
    Notifier::new(START_MS, NotificationSettings::default())
}

#[test]
fn a_session_that_needs_you_and_a_result_that_is_not_seen_notify() {
    let host = host();
    let mut notifier = notifier();
    let ask = run(
        "a",
        AgentState::NeedsYou(Some(NeedsYouKind::Permission)),
        START_MS + 1,
    );
    let notice = notifier
        .on_event(&changed(ask), &host, &away())
        .expect("a notification");
    assert_eq!(notice.session_id, "a");
    assert_eq!(notice.kind, NoticeKind::NeedsYou);
    assert_eq!(notice.title, "acme-web");
    assert_eq!(notice.body, "Waiting for a permission");

    let done = run("b", AgentState::Done, START_MS + 2);
    let notice = notifier
        .on_event(&changed(done), &host, &away())
        .expect("a notification");
    assert_eq!(notice.kind, NoticeKind::Done);
    assert_eq!(notice.body, "Done");
}

#[test]
fn the_other_states_do_not_notify() {
    let host = host();
    let mut notifier = notifier();
    let mut seen = run("a", AgentState::Done, START_MS + 1);
    seen.status.seen = true;
    let cases = [
        ("working", run("a", AgentState::Working, START_MS + 1)),
        ("idle", run("a", AgentState::Idle, START_MS + 1)),
        ("error", run("a", AgentState::Error, START_MS + 1)),
        ("seen result", seen),
    ];
    for (name, case) in cases {
        assert!(
            notifier.on_event(&changed(case), &host, &away()).is_none(),
            "{name}"
        );
    }
}

#[test]
fn a_guess_and_a_restored_state_do_not_notify() {
    let host = host();
    let mut notifier = notifier();
    for source in [
        StatusSource::PromptMarks,
        StatusSource::ForegroundProgram,
        StatusSource::SessionRestore,
        StatusSource::None,
    ] {
        let mut guess = run("a", AgentState::NeedsYou(None), START_MS + 1);
        guess.status.source = source;
        assert!(
            notifier.on_event(&changed(guess), &host, &away()).is_none(),
            "{source:?}"
        );
    }
}

#[test]
fn no_notification_while_the_user_looks_at_the_session() {
    let host = host();
    let mut notifier = notifier();
    let looking = Viewing {
        window_active: true,
        session: Some("a"),
    };
    let ask = run("a", AgentState::NeedsYou(None), START_MS + 1);
    assert!(notifier.on_event(&changed(ask), &host, &looking).is_none());

    // Another session of the same window notifies.
    let other = run("b", AgentState::NeedsYou(None), START_MS + 2);
    assert!(
        notifier
            .on_event(&changed(other), &host, &looking)
            .is_some()
    );

    // The same session notifies when the window is not active.
    let inactive = Viewing {
        window_active: false,
        session: Some("a"),
    };
    let again = run("a", AgentState::NeedsYou(None), START_MS + 3);
    assert!(
        notifier
            .on_event(&changed(again), &host, &inactive)
            .is_some()
    );
}

#[test]
fn a_state_that_began_before_the_floor_does_not_notify() {
    let host = host();
    let mut notifier = notifier();
    // The state was there at the start: later changes of its message too.
    let old = run("a", AgentState::NeedsYou(None), START_MS - 1);
    assert!(notifier.on_event(&changed(old), &host, &away()).is_none());
    let mut message = run("a", AgentState::NeedsYou(None), START_MS - 1);
    message.status.message = Some(Sourced {
        value: "a new message".to_owned(),
        source: StatusSource::Adapter,
    });
    assert!(
        notifier
            .on_event(&changed(message), &host, &away())
            .is_none()
    );
    // A state after the floor notifies.
    let fresh = run("a", AgentState::NeedsYou(None), START_MS);
    assert!(notifier.on_event(&changed(fresh), &host, &away()).is_some());
}

#[test]
fn the_same_state_does_not_notify_twice() {
    let host = host();
    let mut notifier = notifier();
    let first = run("a", AgentState::NeedsYou(None), START_MS + 5);
    assert!(notifier.on_event(&changed(first), &host, &away()).is_some());
    // A new message in the same state, and the event again after a load.
    let mut message = run("a", AgentState::NeedsYou(None), START_MS + 5);
    message.status.message = Some(Sourced {
        value: "which file?".to_owned(),
        source: StatusSource::Adapter,
    });
    assert!(
        notifier
            .on_event(&changed(message), &host, &away())
            .is_none()
    );
    // A state that is seen by the user does not count as a new one either.
    let first_again = run("a", AgentState::NeedsYou(None), START_MS + 5);
    assert!(
        notifier
            .on_event(&changed(first_again), &host, &away())
            .is_none()
    );
}

#[test]
fn a_new_state_notifies_again_and_a_state_that_ended_resets() {
    let host = host();
    let mut notifier = notifier();
    let ask = run("a", AgentState::NeedsYou(None), START_MS + 5);
    assert!(notifier.on_event(&changed(ask), &host, &away()).is_some());
    // The user answers, the agent works, and it asks again: a new state.
    let working = run("a", AgentState::Working, START_MS + 6);
    assert!(
        notifier
            .on_event(&changed(working), &host, &away())
            .is_none()
    );
    let ask_again = run("a", AgentState::NeedsYou(None), START_MS + 7);
    assert!(
        notifier
            .on_event(&changed(ask_again), &host, &away())
            .is_some()
    );
    // The run goes away and comes back with a new state at the same time
    // value: the memory of the old one is gone.
    let removed = HostEvent {
        seq: 2,
        at_ms: START_MS + 20,
        body: EventBody::AgentRunRemoved {
            session_id: "a".to_owned(),
        },
    };
    assert!(notifier.on_event(&removed, &host, &away()).is_none());
    let ask_third = run("a", AgentState::NeedsYou(None), START_MS + 7);
    assert!(
        notifier
            .on_event(&changed(ask_third), &host, &away())
            .is_some()
    );
}

#[test]
fn a_kind_that_is_off_does_not_notify() {
    let host = host();
    let settings = NotificationSettings {
        needs_you: true,
        done: false,
    };
    let mut notifier = Notifier::new(START_MS, settings);
    let done = run("a", AgentState::Done, START_MS + 1);
    assert!(notifier.on_event(&changed(done), &host, &away()).is_none());
    let ask = run("b", AgentState::NeedsYou(None), START_MS + 2);
    assert!(notifier.on_event(&changed(ask), &host, &away()).is_some());
}

#[test]
fn the_title_is_the_agent_title_and_the_body_the_message() {
    let host = host();
    let mut notifier = notifier();
    let mut ask = run(
        "a",
        AgentState::NeedsYou(Some(NeedsYouKind::Question)),
        START_MS + 1,
    );
    ask.status.title = Some(Sourced {
        value: "Refactor".to_owned(),
        source: StatusSource::Adapter,
    });
    ask.status.message = Some(Sourced {
        value: "Which parser?".to_owned(),
        source: StatusSource::Adapter,
    });
    let notice = notifier
        .on_event(&changed(ask), &host, &away())
        .expect("a notification");
    assert_eq!(notice.title, "Refactor");
    assert_eq!(notice.body, "Which parser?");
}

#[test]
fn the_settings_text_turns_a_kind_off() {
    let cases = [
        ("", true, true),
        ("notify.done = off", true, false),
        ("notify.needs-you=off\nnotify.done=off", false, false),
        ("# a comment\n\nnotify.done = on", true, true),
        ("notify.done = maybe\nunknown = off", true, true),
    ];
    for (text, needs_you, done) in cases {
        let settings = NotificationSettings::from_text(text);
        assert_eq!(
            (settings.needs_you, settings.done),
            (needs_you, done),
            "{text:?}"
        );
    }
}
