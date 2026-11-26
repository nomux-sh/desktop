//! The content of the project rail: one tile for each Project and each
//! Group, in the sequence of the user. An open Group holds its children.

use nomux_client::NodeKind;

use crate::glyph_state::GlyphState;
use crate::host_state::HostState;
use crate::rail_labels::project_labels;
use crate::selection::{ProjectView, Selection};

/// The number of children that the mini grid of a closed Group shows.
pub const MINI_GRID_CELLS: usize = 4;

/// One tile of the rail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RailTile {
    /// What the tile selects or opens.
    pub target: TileTarget,
    /// The full name, for the tooltip.
    pub name: String,
    /// The face and the corner marks.
    pub kind: TileKind,
    /// `true` for the tile of the selected Project, and for a closed Group
    /// that holds it.
    pub selected: bool,
}

impl RailTile {
    /// `true` if the tile, or a tile in its strip, shows the working
    /// glyph: the glyph turns.
    #[must_use]
    pub fn animates(&self) -> bool {
        match &self.kind {
            TileKind::Project { state, .. } | TileKind::ClosedGroup { state, .. } => {
                *state == Some(GlyphState::Working)
            }
            TileKind::OpenGroup { children, .. } => children.iter().any(Self::animates),
        }
    }
}

/// What a click on a tile does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TileTarget {
    /// Shows a Project, or the sessions without one.
    Project(ProjectView),
    /// Opens or closes the Group with this node identifier.
    Group(String),
}

/// The face of a tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TileKind {
    /// A Project: its label and the most urgent state of its sessions.
    Project {
        /// The two characters of the label.
        label: String,
        /// The state mark. `None` for a Project without sessions.
        state: Option<GlyphState>,
    },
    /// A closed Group: the mini grid and the aggregate state.
    ClosedGroup {
        /// One square for each of the first children: `true` for a Project
        /// (filled), `false` for a Group (hollow).
        grid: Vec<bool>,
        /// The number of Projects in the Group and in its Groups.
        project_count: usize,
        /// The most urgent state of each Project in the Group.
        state: Option<GlyphState>,
    },
    /// An open Group: its initial, the open mark, and its children in the
    /// strip.
    OpenGroup {
        /// The first character of the name, in upper case.
        initial: String,
        /// The tiles of the children.
        children: Vec<RailTile>,
    },
}

/// The label of the tile of the sessions without a Project.
pub const UNASSIGNED_LABEL: &str = "~";

/// The tiles of the top level. The tile of the sessions without a Project
/// is last, and only if such sessions exist.
#[must_use]
pub fn rail(host: &HostState, selection: &Selection) -> Vec<RailTile> {
    let labels = project_labels(&host.workspace().tree);
    let builder = RailBuilder {
        host,
        selection,
        labels: &labels,
    };
    let mut tiles = builder.tiles(None);
    if host.has_unassigned_sessions() {
        let view = ProjectView::Unassigned;
        tiles.push(RailTile {
            selected: selection.project() == Some(&view),
            name: crate::sidebar::UNASSIGNED_TITLE.to_owned(),
            kind: TileKind::Project {
                label: UNASSIGNED_LABEL.to_owned(),
                state: builder.project_state(&view),
            },
            target: TileTarget::Project(view),
        });
    }
    tiles
}

struct RailBuilder<'a> {
    host: &'a HostState,
    selection: &'a Selection,
    labels: &'a std::collections::HashMap<String, String>,
}

impl RailBuilder<'_> {
    fn tiles(&self, parent: Option<&str>) -> Vec<RailTile> {
        let tree = &self.host.workspace().tree;
        tree.children(parent)
            .iter()
            .filter_map(|id| tree.get(id))
            .map(|node| match node.kind {
                NodeKind::Project { .. } => {
                    let view = ProjectView::Project(node.id.clone());
                    RailTile {
                        selected: self.selection.project() == Some(&view),
                        name: node.name.clone(),
                        kind: TileKind::Project {
                            label: self.labels.get(&node.id).cloned().unwrap_or_default(),
                            state: self.project_state(&view),
                        },
                        target: TileTarget::Project(view),
                    }
                }
                NodeKind::Group => self.group_tile(&node.id, &node.name),
            })
            .collect()
    }

    fn group_tile(&self, id: &str, name: &str) -> RailTile {
        let target = TileTarget::Group(id.to_owned());
        if self.selection.is_open(id) {
            return RailTile {
                target,
                name: name.to_owned(),
                selected: false,
                kind: TileKind::OpenGroup {
                    initial: name
                        .chars()
                        .next()
                        .map(char::to_uppercase)
                        .into_iter()
                        .flatten()
                        .collect(),
                    children: self.tiles(Some(id)),
                },
            };
        }
        let tree = &self.host.workspace().tree;
        let grid = tree
            .children(Some(id))
            .iter()
            .take(MINI_GRID_CELLS)
            .filter_map(|child| tree.get(child))
            .map(|child| !child.is_group())
            .collect();
        let projects = self.projects_below(id);
        let selected = self
            .selection
            .project()
            .and_then(ProjectView::project_id)
            .is_some_and(|selected| projects.iter().any(|project| project == selected));
        let state = GlyphState::most_urgent(
            projects
                .iter()
                .filter_map(|project| self.project_state(&ProjectView::Project(project.clone()))),
        );
        RailTile {
            target,
            name: name.to_owned(),
            selected,
            kind: TileKind::ClosedGroup {
                grid,
                project_count: projects.len(),
                state,
            },
        }
    }

    /// The node identifiers of the Projects in the Group and in its Groups.
    fn projects_below(&self, group_id: &str) -> Vec<String> {
        let tree = &self.host.workspace().tree;
        let mut projects = Vec::new();
        let mut stack = vec![group_id.to_owned()];
        while let Some(id) = stack.pop() {
            for child in tree.children(Some(&id)) {
                match tree.get(child).map(|node| node.is_group()) {
                    Some(true) => stack.push(child.clone()),
                    Some(false) => projects.push(child.clone()),
                    None => {}
                }
            }
        }
        projects
    }

    fn project_state(&self, view: &ProjectView) -> Option<GlyphState> {
        GlyphState::most_urgent(
            self.host
                .sessions_of(view)
                .map(|session| GlyphState::of_session(session, self.host.run(&session.id))),
        )
    }
}
