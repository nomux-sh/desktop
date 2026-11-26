//! The terminal tabs of the window: the [`Tabs`] of the model, one
//! [`TerminalTab`] for each open session, the tab bar, and the state file.
//!
//! Only the active tab is visible. A hidden tab keeps its attach: the
//! connection thread feeds its terminal, and the tab asks for no frame.
//! Closing a tab drops its [`TerminalTab`], which detaches; the session
//! continues.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gpui::{App, AppContext, Context, Entity, Focusable, Window};
use nomux_desktop_model::{HostState, Tabs};
use nomux_desktop_theme::DesktopTheme;
use nomux_sidebar::ContextHandler;

use crate::host_connection::HostConnection;
use crate::measure::Measurement;
use crate::tab_bar::{TabBarView, TabHandler};
use crate::tab_state;
use crate::terminal_tab::TerminalTab;
use crate::workbench::Workbench;

pub(crate) struct TerminalTabs {
    tabs: Tabs,
    /// The tabs of the state file, until the first list of the sessions.
    restored: Option<Tabs>,
    /// The tabs at the last save of the state file.
    saved: Tabs,
    terminals: HashMap<String, Entity<TerminalTab>>,
    bar: Entity<TabBarView>,
    state_path: Option<PathBuf>,
    connection: HostConnection,
    measurement: Option<Measurement>,
    /// The session that had the focus after the last sync.
    focused: Option<String>,
}

impl TerminalTabs {
    pub(crate) fn new(
        connection: HostConnection,
        theme: DesktopTheme,
        measurement: Option<Measurement>,
        handlers: (TabHandler, TabHandler, ContextHandler),
        cx: &mut Context<Workbench>,
    ) -> Self {
        let state_path = tab_state::path();
        let restored = tab_state::load(state_path.as_deref());
        Self {
            tabs: Tabs::default(),
            saved: restored.clone(),
            restored: Some(restored),
            terminals: HashMap::new(),
            bar: cx.new(|_| TabBarView::new(theme, handlers)),
            state_path,
            connection,
            measurement,
            focused: None,
        }
    }

    pub(crate) fn tabs(&self) -> &Tabs {
        &self.tabs
    }

    pub(crate) fn tabs_mut(&mut self) -> &mut Tabs {
        &mut self.tabs
    }

    pub(crate) fn bar(&self) -> &Entity<TabBarView> {
        &self.bar
    }

    /// The terminal of the active tab.
    pub(crate) fn active_terminal(&self) -> Option<&Entity<TerminalTab>> {
        self.terminals.get(self.tabs.active()?)
    }

    /// Opens the tabs of the state file whose sessions the host has, one
    /// time, at the first list of the sessions. Returns `false` if there was
    /// nothing to open.
    pub(crate) fn restore(&mut self, host: &HostState) -> bool {
        let Some(mut restored) = self.restored.take() else {
            return !self.tabs.is_empty();
        };
        restored.retain_existing(host);
        self.tabs = restored;
        !self.tabs.is_empty()
    }

    /// Makes the terminals, the tab bar and the state file agree with the
    /// tabs, and gives the focus to a newly active tab.
    pub(crate) fn sync(
        &mut self,
        host: &HostState,
        home: Option<&Path>,
        theme: DesktopTheme,
        window: &mut Window,
        cx: &mut Context<Workbench>,
    ) {
        if self.restored.is_none() {
            self.tabs.retain_existing(host);
        }
        let open = self.tabs.sessions().to_vec();
        self.terminals.retain(|id, _| open.contains(id));
        for id in &open {
            if !self.terminals.contains_key(id) {
                let terminal = self.new_terminal(id, theme, window, cx);
                self.terminals.insert(id.clone(), terminal);
            }
        }
        let active = self.tabs.active().map(str::to_owned);
        let size = self
            .active_terminal()
            .and_then(|terminal| terminal.read(cx).grid_size());
        for (id, terminal) in &self.terminals {
            let visible = active.as_deref() == Some(id.as_str());
            terminal.update(cx, |terminal, cx| {
                terminal.set_visible(visible, window, cx);
                if let (false, Some(size)) = (visible, size) {
                    terminal.ensure_attached(size, cx);
                }
            });
        }
        if active != self.focused {
            if let Some(terminal) = self.active_terminal() {
                window.focus(&terminal.focus_handle(cx), cx);
            }
            self.focused = active;
        }
        let items = self.tabs.items(host, home);
        self.bar.update(cx, |bar, cx| bar.set_items(items, cx));
        if self.restored.is_none() && self.tabs != self.saved {
            self.saved = self.tabs.clone();
            tab_state::save(self.state_path.clone(), &self.tabs, cx);
        }
    }

    fn new_terminal(
        &self,
        id: &str,
        theme: DesktopTheme,
        window: &mut Window,
        cx: &mut Context<Workbench>,
    ) -> Entity<TerminalTab> {
        let connection = self.connection.clone();
        let measurement = self.measurement.clone();
        let id = id.to_owned();
        cx.new(|cx| TerminalTab::new(id, connection, &theme, measurement, window, cx))
    }

    pub(crate) fn set_theme(&self, theme: DesktopTheme, cx: &mut App) {
        self.bar.update(cx, |bar, cx| bar.set_theme(theme, cx));
        for terminal in self.terminals.values() {
            terminal.update(cx, |terminal, cx| terminal.set_theme(&theme, cx));
        }
    }

    pub(crate) fn motion_changed(&self, window: &mut Window, cx: &mut App) {
        for terminal in self.terminals.values() {
            terminal.update(cx, |terminal, cx| terminal.motion_changed(window, cx));
        }
    }
}
