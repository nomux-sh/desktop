//! What the user selected.

use std::collections::HashSet;

use nomux_client::NodeKind;

use crate::host_state::HostState;

/// What the sidebar shows: the sessions of one Project, or the sessions that
/// have no Project.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProjectView {
    /// The Project with this node identifier.
    Project(String),
    /// The sessions whose working directory is in no Project.
    Unassigned,
}

impl ProjectView {
    /// The node identifier of the Project, `None` for [`ProjectView::Unassigned`].
    #[must_use]
    pub fn project_id(&self) -> Option<&str> {
        match self {
            Self::Project(id) => Some(id),
            Self::Unassigned => None,
        }
    }
}

/// The selected Project, the selected session and the open Groups of the
/// rail.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    project: Option<ProjectView>,
    session: Option<String>,
    open_groups: HashSet<String>,
}

impl Selection {
    /// The Project that the sidebar shows.
    #[must_use]
    pub fn project(&self) -> Option<&ProjectView> {
        self.project.as_ref()
    }

    /// The session that the terminal tab shows.
    #[must_use]
    pub fn session(&self) -> Option<&str> {
        self.session.as_deref()
    }

    /// `true` if the Group `id` is open in the rail.
    #[must_use]
    pub fn is_open(&self, group_id: &str) -> bool {
        self.open_groups.contains(group_id)
    }

    /// Opens a closed Group and closes an open Group. The Groups in it keep
    /// their state.
    pub fn toggle_group(&mut self, group_id: &str) {
        if !self.open_groups.remove(group_id) {
            self.open_groups.insert(group_id.to_owned());
        }
    }

    /// Shows a Project in the sidebar and selects its first session.
    pub fn select_project(&mut self, view: ProjectView, host: &HostState) {
        if self.project.as_ref() == Some(&view) {
            return;
        }
        self.session = None;
        self.project = Some(view);
        self.reconcile(host);
    }

    /// Selects a session of the shown Project.
    pub fn select_session(&mut self, session_id: &str) {
        self.session = Some(session_id.to_owned());
    }

    /// Makes the selection agree with the state of the host: a removed
    /// Project or session is not selected, and when nothing is selected, the
    /// first Project with sessions and its first session are.
    pub fn reconcile(&mut self, host: &HostState) {
        if self
            .project
            .as_ref()
            .is_some_and(|view| !exists(view, host))
        {
            self.project = None;
        }
        if self.project.is_none() {
            self.project = default_project(host);
        }
        let Some(view) = &self.project else {
            self.session = None;
            return;
        };
        let session_is_shown = self.session.as_deref().is_some_and(|id| {
            host.session(id)
                .is_some_and(|session| host.project_of(session) == view.project_id())
        });
        if !session_is_shown {
            self.session = crate::sidebar::ordered_sessions(host, view)
                .first()
                .map(|session| session.id.clone());
        }
    }
}

fn exists(view: &ProjectView, host: &HostState) -> bool {
    match view {
        ProjectView::Project(id) => host.workspace().tree.get(id).is_some(),
        ProjectView::Unassigned => host.has_unassigned_sessions(),
    }
}

/// The first Project that has a session, else the sessions without a
/// Project if there are some, else the first Project.
fn default_project(host: &HostState) -> Option<ProjectView> {
    let projects: Vec<ProjectView> = host
        .workspace()
        .tree
        .nodes()
        .into_iter()
        .filter(|node| matches!(node.kind, NodeKind::Project { .. }))
        .map(|node| ProjectView::Project(node.id.clone()))
        .collect();
    projects
        .iter()
        .find(|view| host.sessions_of(view).next().is_some())
        .cloned()
        .or_else(|| {
            host.has_unassigned_sessions()
                .then_some(ProjectView::Unassigned)
        })
        .or_else(|| projects.first().cloned())
}
