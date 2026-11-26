//! The choices of the context menus (right click) and of the list of
//! Projects, as plain data for a view. Each choice names what the window
//! does; the window does it.

use crate::host_state::HostState;

/// What a context menu belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuTarget {
    /// A Session row of the sidebar.
    Session {
        /// The session identifier.
        id: String,
        /// `true` if the session has an open tab.
        has_tab: bool,
    },
    /// A tab of the tab bar.
    Tab {
        /// The session identifier of the tab.
        id: String,
        /// The number of open tabs.
        tab_count: usize,
    },
    /// The tile of a Project in the rail.
    Project {
        /// The node identifier of the Project.
        id: String,
    },
}

/// What a choice of a menu does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuCommand {
    /// Opens the session in a tab.
    OpenInTab(String),
    /// Asks for a new name of the session.
    RenameSession(String),
    /// Closes the tab of the session. The session continues.
    CloseTab(String),
    /// Closes each tab except the tab of the session.
    CloseOtherTabs(String),
    /// Asks, then ends the session.
    KillSession(String),
    /// Starts a session in the directory of the Project (node identifier).
    NewSessionInProject(String),
    /// Removes the Project from the workspace tree. No file and no session
    /// changes.
    RemoveProject(String),
}

/// One choice of a menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuEntry {
    /// The text of the choice.
    pub label: String,
    /// What the choice does.
    pub command: MenuCommand,
    /// `true` for a choice that ends something that the user cannot start
    /// again: the view shows it in the colour of an error.
    pub destructive: bool,
}

impl MenuEntry {
    fn new(label: &str, command: MenuCommand) -> Self {
        Self {
            label: label.to_owned(),
            command,
            destructive: false,
        }
    }

    fn destructive(label: &str, command: MenuCommand) -> Self {
        Self {
            destructive: true,
            ..Self::new(label, command)
        }
    }
}

/// The choices of the context menu of `target`.
#[must_use]
pub fn entries(target: &MenuTarget) -> Vec<MenuEntry> {
    match target {
        MenuTarget::Session { id, has_tab } => {
            let mut entries = vec![
                MenuEntry::new("Open in tab", MenuCommand::OpenInTab(id.clone())),
                MenuEntry::new("Rename...", MenuCommand::RenameSession(id.clone())),
            ];
            if *has_tab {
                entries.push(MenuEntry::new(
                    "Close tab",
                    MenuCommand::CloseTab(id.clone()),
                ));
            }
            entries.push(MenuEntry::destructive(
                "Kill session...",
                MenuCommand::KillSession(id.clone()),
            ));
            entries
        }
        MenuTarget::Tab { id, tab_count } => {
            let mut entries = vec![MenuEntry::new("Close", MenuCommand::CloseTab(id.clone()))];
            if *tab_count > 1 {
                entries.push(MenuEntry::new(
                    "Close others",
                    MenuCommand::CloseOtherTabs(id.clone()),
                ));
            }
            entries
        }
        MenuTarget::Project { id } => vec![
            MenuEntry::new("New session", MenuCommand::NewSessionInProject(id.clone())),
            MenuEntry::new(
                "Remove from sidebar",
                MenuCommand::RemoveProject(id.clone()),
            ),
        ],
    }
}

/// One choice for each Project of the workspace tree, in the sequence of
/// the tree: the list of "New Session in Project...".
#[must_use]
pub fn project_entries(host: &HostState) -> Vec<MenuEntry> {
    host.workspace()
        .tree
        .nodes()
        .into_iter()
        .filter(|node| !node.is_group())
        .map(|node| {
            MenuEntry::new(
                &node.name,
                MenuCommand::NewSessionInProject(node.id.clone()),
            )
        })
        .collect()
}

/// The question before a session ends: it names the session.
#[must_use]
pub fn kill_question(name: &str) -> String {
    format!("Kill session \"{name}\"?")
}

/// What the answer to [`kill_question`] explains.
pub const KILL_DETAIL: &str = "Its program ends. This cannot be undone.";
