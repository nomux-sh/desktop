//! A sidebar row renders again only when its displayed values change: the
//! paints of each row for a sequence of events of the host.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, TestAppContext,
    VisualTestContext, Window, div,
};
use nomux_client::{
    AgentRun, AgentState, AgentStatus, AgentTree, EventBody, HostEvent, Node, NodeKind, Session,
    SessionState, Sourced, StatusSource, Workspace, WorkspaceChange, WorkspaceTree,
};
use nomux_desktop_model::{HostState, ProjectView, Selection, sidebar};
use nomux_desktop_theme::DesktopTheme;
use nomux_sidebar::SidebarView;

struct Root {
    sidebar: Entity<SidebarView>,
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // As in the window: the sidebar itself is not cached, its rows are.
        div().size_full().child(self.sidebar.clone())
    }
}

fn session(id: &str) -> Session {
    Session {
        id: id.to_owned(),
        state: SessionState::Running,
        pid: None,
        command: vec!["bash".to_owned()],
        working_directory: Some(PathBuf::from("/home/user/web")),
        resolved_working_directory: None,
        size: None,
        started_at: Some(1_000),
        epoch: Some(1),
        restore_policy: None,
        exit: None,
        damage: None,
    }
}

fn run(session_id: &str, state: AgentState, message: Option<&str>) -> AgentRun {
    let status = AgentStatus {
        state,
        source: StatusSource::Adapter,
        since_ms: 1_000_000,
        seen: false,
        message: message.map(|text| Sourced {
            value: text.to_owned(),
            source: StatusSource::Adapter,
        }),
        title: None,
        progress: None,
        tool: None,
    };
    AgentRun {
        session_id: session_id.to_owned(),
        aggregate: status.clone(),
        status,
        children: Vec::new(),
        details: None,
    }
}

fn event(body: EventBody) -> HostEvent {
    HostEvent {
        seq: 1,
        at_ms: 0,
        body,
    }
}

/// One Project, `web`, with the sessions `a`, `b` and `c`.
fn host() -> (HostState, Selection) {
    let node = Node {
        id: "web".to_owned(),
        parent_id: None,
        name: "web".to_owned(),
        kind: NodeKind::Project {
            directory: PathBuf::from("/home/user/web"),
        },
    };
    let mut host = HostState::default();
    host.set_workspace(Workspace {
        revision: 1,
        tree: WorkspaceTree::from_nodes([node]).expect("a valid tree"),
    });
    host.set_sessions(vec![session("a"), session("b"), session("c")]);
    host.set_agent_tree(AgentTree {
        runs: Vec::new(),
        attention: None,
    });
    let mut selection = Selection::default();
    selection.select_project(ProjectView::Project("web".to_owned()), &host);
    (host, selection)
}

/// Gives the sidebar the content of `host`, draws a frame and returns the
/// paints of each row.
fn show(
    host: &HostState,
    selection: &Selection,
    view: &Entity<SidebarView>,
    cx: &mut VisualTestContext,
) -> Vec<(String, u64)> {
    let content = sidebar::sidebar(host, selection, 1_000_000, None);
    view.update(cx, |view, cx| view.set_content(content.as_ref(), cx));
    draw(view, cx)
}

fn draw(view: &Entity<SidebarView>, cx: &mut VisualTestContext) -> Vec<(String, u64)> {
    cx.update(|window, cx| window.draw(cx).clear(cx));
    view.read_with(cx, |view, cx| {
        view.rows()
            .iter()
            .map(|row| {
                let row = row.read(cx);
                (row.row().session_id.clone(), row.paints())
            })
            .collect()
    })
}

fn paints(rows: &[(String, u64)]) -> Vec<u64> {
    rows.iter().map(|(_, paints)| *paints).collect()
}

#[gpui::test]
fn a_row_paints_again_only_when_its_values_change(cx: &mut TestAppContext) {
    let theme = DesktopTheme::for_appearance(true);
    let sidebar = cx.new(|_| SidebarView::new(theme, Rc::new(|_, _, _| {}), Rc::new(|_, _, _| {})));
    let root = sidebar.clone();
    let (_, cx) = cx.add_window_view(|_, _| Root { sidebar: root });
    let (mut host, selection) = host();

    let first = show(&host, &selection, &sidebar, cx);
    let ids: Vec<&str> = first.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(ids, ["a", "b", "c"]);
    assert_eq!(paints(&first), [1, 1, 1]);

    // The agent in `c` starts to work: only `c` paints.
    host.apply_event(&event(EventBody::AgentRunChanged(Box::new(run(
        "c",
        AgentState::Working,
        None,
    )))));
    assert_eq!(paints(&show(&host, &selection, &sidebar, cx)), [1, 1, 2]);

    // A load of the same sessions changes no row.
    host.set_sessions(vec![session("a"), session("b"), session("c")]);
    assert_eq!(paints(&show(&host, &selection, &sidebar, cx)), [1, 1, 2]);

    // A new message of `c`: only `c` paints.
    host.apply_event(&event(EventBody::AgentRunChanged(Box::new(run(
        "c",
        AgentState::Working,
        Some("running the tests"),
    )))));
    assert_eq!(paints(&show(&host, &selection, &sidebar, cx)), [1, 1, 3]);

    // A rename of the Project changes the header, not the rows.
    host.apply_event(&event(EventBody::WorkspaceChanged {
        revision: 2,
        change: WorkspaceChange::Renamed {
            id: "web".to_owned(),
            name: "storefront".to_owned(),
        },
    }));
    assert_eq!(paints(&show(&host, &selection, &sidebar, cx)), [1, 1, 3]);

    // A tick of the clock paints the working glyph of `c` only.
    sidebar.update(cx, |view, cx| view.tick(cx));
    assert_eq!(paints(&draw(&sidebar, cx)), [1, 1, 4]);

    // A frame without a change paints nothing.
    assert_eq!(paints(&draw(&sidebar, cx)), [1, 1, 4]);
}
