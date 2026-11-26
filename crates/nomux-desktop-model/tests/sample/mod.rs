//! A host with the sample data of the sidebar specification.

// Each test file uses a part of the sample.
#![allow(dead_code)]

use std::path::PathBuf;

use nomux_client::{
    AgentRun, AgentState, AgentStatus, AgentTree, ChildWork, Node, NodeKind, Session, SessionState,
    StatusSource, Workspace, WorkspaceTree,
};
use nomux_desktop_model::HostState;

pub(crate) fn group(id: &str, parent: Option<&str>, name: &str) -> Node {
    Node {
        id: id.to_owned(),
        parent_id: parent.map(str::to_owned),
        name: name.to_owned(),
        kind: NodeKind::Group,
    }
}

pub(crate) fn project(id: &str, parent: Option<&str>, name: &str) -> Node {
    Node {
        id: id.to_owned(),
        parent_id: parent.map(str::to_owned),
        name: name.to_owned(),
        kind: NodeKind::Project {
            directory: PathBuf::from(format!("/home/user/{name}")),
        },
    }
}

/// Acme (acme-web, acme-admin, acme-mobile, 2019 (acme-shop-2019)),
/// globex-api, Personal (dotfiles, docs).
pub(crate) fn workspace() -> Workspace {
    let nodes = [
        group("acme", None, "Acme"),
        project("web", Some("acme"), "acme-web"),
        project("admin", Some("acme"), "acme-admin"),
        project("mobile", Some("acme"), "acme-mobile"),
        group("2019", Some("acme"), "2019"),
        project("shop", Some("2019"), "acme-shop-2019"),
        project("globex", None, "globex-api"),
        group("personal", None, "Personal"),
        project("dotfiles", Some("personal"), "dotfiles"),
        project("docs", Some("personal"), "docs"),
    ];
    Workspace {
        revision: 1,
        tree: WorkspaceTree::from_nodes(nodes).expect("a valid tree"),
    }
}

pub(crate) fn session(id: &str, directory: &str, command: &str, state: SessionState) -> Session {
    Session {
        id: id.to_owned(),
        name: None,
        state,
        pid: None,
        command: vec![command.to_owned()],
        working_directory: Some(PathBuf::from(directory)),
        resolved_working_directory: None,
        size: None,
        started_at: Some(1_000),
        epoch: Some(1),
        restore_policy: None,
        exit: None,
        damage: None,
        last_attached_at: None,
        attached_clients: 0,
        hibernation: None,
        pinned: false,
    }
}

pub(crate) fn status(state: AgentState, seen: bool, source: StatusSource) -> AgentStatus {
    AgentStatus {
        state,
        source,
        since_ms: 1_000_000,
        seen,
        message: None,
        title: None,
        progress: None,
        tool: None,
    }
}

pub(crate) fn run(session_id: &str, state: AgentState) -> AgentRun {
    let status = status(state, false, StatusSource::Adapter);
    AgentRun {
        session_id: session_id.to_owned(),
        aggregate: status.clone(),
        status,
        children: Vec::new(),
        details: None,
    }
}

pub(crate) fn child(name: &str, state: AgentState) -> ChildWork {
    let status = status(state, false, StatusSource::Adapter);
    ChildWork {
        name: name.to_owned(),
        status: Some(status.clone()),
        aggregate: status,
        children: Vec::new(),
    }
}

/// The sample host: sessions in acme-web, one in acme-mobile, none in the
/// other Projects.
pub(crate) fn host(sessions: Vec<Session>, runs: Vec<AgentRun>) -> HostState {
    let mut host = HostState::default();
    host.set_workspace(workspace());
    host.set_sessions(sessions);
    host.set_agent_tree(AgentTree {
        runs,
        attention: None,
    });
    host
}
