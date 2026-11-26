//! The root view of the window: the rail, the sidebar and the main area.
//! It routes the updates of the host to the model and the clicks to the
//! selection, and gives each view its new content. The model, the views
//! and the terminal have their own types.

use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::{
    AnyElement, AnyView, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, StyleRefinement, Styled, Subscription, Task,
    Window, WindowAppearance, div,
};
use nomux_client::NodeKind;
use nomux_desktop_model::motion::{MotionContext, working_step};
use nomux_desktop_model::rail::TileTarget;
use nomux_desktop_model::{HostState, ProjectView, Selection, rail, sidebar};
use nomux_desktop_theme::{ColorToken, DesktopTheme, DurationToken, LengthToken, TextStyle};
use nomux_sidebar::{ClickHandler, RailView, SidebarView};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::host_connection::{HostConnection, HostRequest, HostUpdate};
use crate::measure::Measurement;
use crate::tab_bar::TabHandler;
use crate::terminal_tab::TerminalTab;
use crate::terminal_tabs::TerminalTabs;
use crate::{
    ActivateLastTab, ActivateTab1, ActivateTab2, ActivateTab3, ActivateTab4, ActivateTab5,
    ActivateTab6, ActivateTab7, ActivateTab8, CloseTab, MoveTabLeft, MoveTabRight, NewSession,
    NextTab, PreviousTab,
};

/// The key context of the window, for the key bindings of the tabs.
pub(crate) const KEY_CONTEXT: &str = "Workbench";

/// The appearance of the window: from the system, or fixed by the option
/// `--appearance`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    /// Follow the system.
    System,
    /// Always dark.
    Dark,
    /// Always light.
    Light,
}

/// The interval of the check of the ages in the sidebar: the shortest age
/// step is one minute.
const AGE_INTERVAL: Duration = Duration::from_secs(60);

/// What the main area shows: the root view renders again only when it
/// changes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MainArea {
    tab: Option<String>,
    message: String,
}

/// The root view.
pub struct Workbench {
    theme: DesktopTheme,
    appearance: Appearance,
    host: HostState,
    selection: Selection,
    connection: HostConnection,
    rail: Entity<RailView>,
    sidebar: Entity<SidebarView>,
    tabs: TerminalTabs,
    /// `true` after the first list of the sessions.
    loaded: bool,
    main_area: Option<MainArea>,
    /// The clock of the working glyphs: runs only while one is visible.
    clock: Option<Task<()>>,
    /// The session that the user created; it opens in a new tab when the
    /// list of the sessions has it.
    created_session: Option<String>,
    failure: Option<String>,
    home: Option<PathBuf>,
    measurement: Option<Measurement>,
    _tasks: Vec<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Workbench {
    /// The root view, with a connection that started.
    pub fn new(
        connection: HostConnection,
        mut updates: UnboundedReceiver<HostUpdate>,
        appearance: Appearance,
        measurement: Option<Measurement>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let updates_task = cx.spawn_in(window, async move |workbench, cx| {
            while let Some(update) = updates.recv().await {
                let applied = workbench.update_in(cx, |workbench, window, cx| {
                    workbench.apply(update, window, cx);
                });
                if applied.is_err() {
                    return;
                }
            }
        });
        let ages_task = cx.spawn_in(window, async move |workbench, cx| {
            loop {
                cx.background_executor().timer(AGE_INTERVAL).await;
                if workbench
                    .update_in(cx, |workbench, window, cx| workbench.refresh(window, cx))
                    .is_err()
                {
                    return;
                }
            }
        });
        let subscriptions = vec![
            cx.observe_window_appearance(window, |workbench, window, cx| {
                workbench.follow_appearance(window, cx);
            }),
            cx.observe_window_visibility(window, |workbench, _, window, cx| {
                workbench.sync_clock(window, cx);
            }),
        ];
        let theme = DesktopTheme::for_appearance(is_dark(appearance, window.appearance()));
        let on_tile: ClickHandler<TileTarget> = Self::handler(cx, Self::click_tile);
        let on_row: ClickHandler<String> =
            Self::handler(cx, |workbench, id: &String, window, cx| {
                workbench.click_session(id, window, cx);
            });
        let on_new: ClickHandler<()> =
            Self::handler(cx, |workbench, (): &(), _, _| workbench.new_session());
        let on_activate: TabHandler = Self::tab_handler(cx, |workbench, id, window, cx| {
            workbench.tabs.tabs_mut().show(id);
            workbench.follow_active_tab(window, cx);
        });
        let on_close: TabHandler = Self::tab_handler(cx, |workbench, id, window, cx| {
            workbench.tabs.tabs_mut().close(id);
            workbench.follow_active_tab(window, cx);
        });
        let tabs = TerminalTabs::new(
            connection.clone(),
            theme,
            measurement.clone(),
            (on_activate, on_close),
            cx,
        );
        Self {
            theme,
            appearance,
            host: HostState::default(),
            selection: Selection::default(),
            connection,
            rail: cx.new(|_| RailView::new(theme, on_tile)),
            sidebar: cx.new(|_| SidebarView::new(theme, on_row, on_new)),
            tabs,
            loaded: false,
            main_area: None,
            clock: None,
            created_session: None,
            failure: None,
            home: std::env::var_os("HOME").map(PathBuf::from),
            measurement,
            _tasks: vec![updates_task, ages_task],
            _subscriptions: subscriptions,
        }
    }

    /// The sidebar view.
    #[must_use]
    pub fn sidebar(&self) -> &Entity<SidebarView> {
        &self.sidebar
    }

    /// The system setting of reduced motion changed.
    pub fn motion_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_clock(window, cx);
        self.tabs.motion_changed(window, cx);
    }

    /// The number of open tabs.
    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.tabs.tabs().sessions().len()
    }

    /// Activates the tab at `index` (0 is the first).
    pub fn activate_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.tabs.tabs_mut().activate(index);
        self.follow_active_tab(window, cx);
    }

    /// The sidebar shows the Project of the active tab and selects its row.
    fn follow_active_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.tabs.tabs().active().map(str::to_owned) {
            self.select_session_of_host(&id);
        }
        self.refresh(window, cx);
    }

    /// Selects the row of the active tab if the sidebar shows its Project.
    fn sync_selection(&mut self) {
        let active = self.tabs.tabs().active().filter(|id| {
            self.host.session(id).is_some_and(|session| {
                self.selection.project().map(ProjectView::project_id)
                    == Some(self.host.project_of(session))
            })
        });
        match active {
            Some(id) => self.selection.select_session(id),
            None => self.selection.clear_session(),
        }
    }

    /// Gives the rail and the sidebar their content, the main area its
    /// session, and starts or stops the clock. Each view renders again only
    /// if its content changed.
    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tabs.sync(&self.host, self.theme, window, cx);
        self.sync_selection();
        let tiles = rail::rail(&self.host, &self.selection);
        let content = sidebar::sidebar(&self.host, &self.selection, now_ms(), self.home.as_deref());
        self.rail.update(cx, |rail, cx| rail.set_tiles(tiles, cx));
        self.sidebar
            .update(cx, |sidebar, cx| sidebar.set_content(content.as_ref(), cx));
        self.sync_clock(window, cx);
        let main_area = Some(self.main_area_content());
        if main_area != self.main_area {
            self.main_area = main_area;
            cx.notify();
        }
    }

    /// Runs the clock of the working glyphs while one is visible and motion
    /// is allowed; stops it otherwise.
    fn sync_clock(&mut self, window: &Window, cx: &mut Context<Self>) {
        let motion = MotionContext {
            window_visible: window.is_visible(),
            reduce_motion: cx.reduce_motion(),
        };
        let animates = self.rail.read(cx).animates()
            || self.sidebar.read(cx).animates(cx)
            || self.tabs.bar().read(cx).animates();
        if !(animates && motion.allows_motion()) {
            self.clock = None;
            return;
        }
        if self.clock.is_some() {
            return;
        }
        let step = working_step(self.theme.duration(DurationToken::MotionIndicator));
        self.clock = Some(cx.spawn(async move |workbench, cx| {
            loop {
                cx.background_executor().timer(step).await;
                let ticked = workbench.update(cx, |workbench, cx| {
                    workbench.rail.update(cx, |rail, cx| rail.tick(cx));
                    workbench.sidebar.update(cx, |sidebar, cx| sidebar.tick(cx));
                    workbench.tabs.bar().update(cx, |bar, cx| bar.tick(cx));
                });
                if ticked.is_err() {
                    return;
                }
            }
        }));
    }

    /// The terminal of the active tab.
    #[must_use]
    pub fn tab(&self) -> Option<&Entity<TerminalTab>> {
        self.tabs.active_terminal()
    }

    fn follow_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.theme = DesktopTheme::for_appearance(is_dark(self.appearance, window.appearance()));
        let theme = self.theme;
        self.rail.update(cx, |rail, cx| rail.set_theme(theme, cx));
        self.sidebar
            .update(cx, |sidebar, cx| sidebar.set_theme(theme, cx));
        self.tabs.set_theme(theme, cx);
        cx.notify();
    }

    fn apply(&mut self, update: HostUpdate, window: &mut Window, cx: &mut Context<Self>) {
        match update {
            HostUpdate::Workspace(workspace) => self.host.set_workspace(workspace),
            HostUpdate::Sessions(sessions) => {
                self.host.set_sessions(sessions);
                if !self.loaded {
                    self.loaded = true;
                    self.open_first_tabs();
                }
                if let Some(id) = self.created_session.take() {
                    if self.host.session(&id).is_some() {
                        self.tabs.tabs_mut().open(&id);
                        self.select_session_of_host(&id);
                    } else {
                        self.created_session = Some(id);
                    }
                }
            }
            HostUpdate::AgentTree(tree) => self.host.set_agent_tree(tree),
            HostUpdate::Event(event) => {
                let reload = self.host.apply_event(&event);
                if !reload.is_empty() {
                    self.connection.send(HostRequest::Load(reload));
                }
            }
            HostUpdate::SessionCreated(session) => self.created_session = Some(session.id),
            HostUpdate::Failed(message) => self.failure = Some(message),
        }
        self.selection.reconcile(&self.host);
        self.refresh(window, cx);
    }

    /// At the first list of the sessions: the tabs of the last run, else
    /// the first session of the first Project.
    fn open_first_tabs(&mut self) {
        if self.tabs.restore(&self.host) {
            if let Some(id) = self.tabs.tabs().active().map(str::to_owned) {
                self.select_session_of_host(&id);
            }
            return;
        }
        self.selection.reconcile(&self.host);
        if let Some(id) = self.selection.session().map(str::to_owned) {
            self.tabs.tabs_mut().show(&id);
        }
    }

    /// Shows the Project of a session and selects the session.
    fn select_session_of_host(&mut self, id: &str) {
        let Some(session) = self.host.session(id) else {
            return;
        };
        let view = self
            .host
            .project_of(session)
            .map_or(ProjectView::Unassigned, |project| {
                ProjectView::Project(project.to_owned())
            });
        self.selection.select_project(view, &self.host);
        self.selection.select_session(id);
    }

    fn click_tile(&mut self, target: &TileTarget, window: &mut Window, cx: &mut Context<Self>) {
        match target {
            TileTarget::Project(view) => {
                self.selection.select_project(view.clone(), &self.host);
                self.activate_tab_of_project(view);
            }
            TileTarget::Group(id) => self.selection.toggle_group(id),
        }
        self.refresh(window, cx);
    }

    /// Activates the first tab of a session of the Project `view`, if one is
    /// open. The other tabs stay as they are.
    fn activate_tab_of_project(&mut self, view: &ProjectView) {
        let host = &self.host;
        let tab = self.tabs.tabs().sessions().iter().position(|id| {
            host.session(id)
                .is_some_and(|session| host.project_of(session) == view.project_id())
        });
        if let Some(index) = tab {
            self.tabs.tabs_mut().activate(index);
        }
    }

    /// A click on a row shows the session in the active tab; with the
    /// platform modifier (Cmd on macOS, Ctrl on Linux) it opens a new tab.
    fn click_session(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if window.modifiers().secondary(cx) {
            self.tabs.tabs_mut().open(id);
        } else {
            self.tabs.tabs_mut().show(id);
        }
        self.follow_active_tab(window, cx);
        if let Some(terminal) = self.tabs.active_terminal() {
            window.focus(&terminal.focus_handle(cx), cx);
        }
    }

    /// Starts a session with the shell of the user in the directory of the
    /// shown Project, or in the home directory. It opens in a new tab.
    pub fn new_session(&mut self) {
        let directory = self.selection.project().and_then(|view| {
            let node = self.host.workspace().tree.get(view.project_id()?)?;
            match &node.kind {
                NodeKind::Project { directory } => Some(directory.clone()),
                NodeKind::Group => None,
            }
        });
        self.connection
            .send(HostRequest::CreateSession { directory });
    }

    fn tab_handler(
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &str, &mut Window, &mut Context<Self>) + 'static,
    ) -> TabHandler {
        let workbench = cx.entity().downgrade();
        let action = Rc::new(action);
        Rc::new(move |id: &str, window, cx| {
            let action = action.clone();
            let _ = workbench.update(cx, |workbench, cx| action(workbench, id, window, cx));
        })
    }

    fn handler<T: 'static>(
        cx: &mut Context<Self>,
        action: impl Fn(&mut Self, &T, &mut Window, &mut Context<Self>) + 'static,
    ) -> ClickHandler<T> {
        let workbench = cx.entity().downgrade();
        let action = Rc::new(action);
        Rc::new(move |value: &T, window, cx| {
            let action = action.clone();
            let _ = workbench.update(cx, |workbench, cx| action(workbench, value, window, cx));
        })
    }

    /// What the main area shows now.
    fn main_area_content(&self) -> MainArea {
        let tab = self.tabs.tabs().active().map(str::to_owned);
        let place = self
            .selection
            .project()
            .and_then(|view| self.host.workspace().tree.get(view.project_id()?))
            .map_or_else(
                || "your home directory".to_owned(),
                |node| node.name.clone(),
            );
        let message = self
            .failure
            .clone()
            .unwrap_or_else(|| format!("A new session starts a shell in {place}."));
        MainArea { tab, message }
    }

    fn main_area(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.theme;
        let content = self
            .main_area
            .clone()
            .unwrap_or_else(|| self.main_area_content());
        let Some(tab) = self.tabs.active_terminal() else {
            return self.empty_state(&content.message, cx);
        };
        let bar_height = theme.metrics().control_height + theme.length(LengthToken::Space1);
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(
                AnyView::from(self.tabs.bar().clone()).cached(
                    StyleRefinement::default()
                        .w_full()
                        .h(bar_height)
                        .flex_none(),
                ),
            )
            .child(
                div().flex_1().min_h_0().child(
                    AnyView::from(tab.clone()).cached(StyleRefinement::default().size_full()),
                ),
            )
            .into_any_element()
    }

    fn empty_state(&self, message: &str, cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.theme;
        let font = theme.font(TextStyle::Title);
        let meta = theme.font(TextStyle::Meta);
        let on_new: ClickHandler<()> =
            Self::handler(cx, |workbench, (): &(), _, _| workbench.new_session());
        let message = message.to_owned();
        div()
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(theme.length(LengthToken::Space2))
            .bg(theme.color(ColorToken::SurfaceBase))
            .child(
                div()
                    .font_family(font.family)
                    .font_weight(font.weight)
                    .text_size(theme.font_size(TextStyle::Title))
                    .text_color(theme.color(ColorToken::TextPrimary))
                    .child("No session"),
            )
            .child(
                div()
                    .font_family(meta.family)
                    .text_size(theme.font_size(TextStyle::Meta))
                    .text_color(theme.color(ColorToken::TextMuted))
                    .child(message),
            )
            .child(
                nomux_sidebar::button(theme, "New session")
                    .id("main-new-session")
                    .on_click(move |_, window, cx| on_new(&(), window, cx)),
            )
            .into_any_element()
    }
}

fn is_dark(appearance: Appearance, system: WindowAppearance) -> bool {
    match appearance {
        Appearance::Dark => true,
        Appearance::Light => false,
        Appearance::System => matches!(
            system,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(measurement) = &self.measurement {
            let measurement = measurement.clone();
            window.on_next_frame(move |_, _| measurement.window_painted());
        }
        let theme = self.theme;
        let full_height = || StyleRefinement::default().h_full().flex_none();
        div()
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|workbench, _: &NewSession, _, _| workbench.new_session()))
            .on_action(cx.listener(|workbench, _: &CloseTab, window, cx| {
                workbench.tabs.tabs_mut().close_active();
                workbench.follow_active_tab(window, cx);
            }))
            .on_action(cx.listener(|workbench, _: &NextTab, window, cx| {
                workbench.tabs.tabs_mut().activate_next(1);
                workbench.follow_active_tab(window, cx);
            }))
            .on_action(cx.listener(|workbench, _: &PreviousTab, window, cx| {
                workbench.tabs.tabs_mut().activate_next(-1);
                workbench.follow_active_tab(window, cx);
            }))
            .on_action(cx.listener(|workbench, _: &MoveTabLeft, window, cx| {
                workbench.tabs.tabs_mut().move_active(-1);
                workbench.refresh(window, cx);
            }))
            .on_action(cx.listener(|workbench, _: &MoveTabRight, window, cx| {
                workbench.tabs.tabs_mut().move_active(1);
                workbench.refresh(window, cx);
            }))
            .on_action(cx.listener(|workbench, _: &ActivateLastTab, window, cx| {
                workbench.tabs.tabs_mut().activate_last();
                workbench.follow_active_tab(window, cx);
            }))
            .on_action(cx.listener(|w, _: &ActivateTab1, window, cx| w.activate_tab(0, window, cx)))
            .on_action(cx.listener(|w, _: &ActivateTab2, window, cx| w.activate_tab(1, window, cx)))
            .on_action(cx.listener(|w, _: &ActivateTab3, window, cx| w.activate_tab(2, window, cx)))
            .on_action(cx.listener(|w, _: &ActivateTab4, window, cx| w.activate_tab(3, window, cx)))
            .on_action(cx.listener(|w, _: &ActivateTab5, window, cx| w.activate_tab(4, window, cx)))
            .on_action(cx.listener(|w, _: &ActivateTab6, window, cx| w.activate_tab(5, window, cx)))
            .on_action(cx.listener(|w, _: &ActivateTab7, window, cx| w.activate_tab(6, window, cx)))
            .on_action(cx.listener(|w, _: &ActivateTab8, window, cx| w.activate_tab(7, window, cx)))
            .size_full()
            .flex()
            .bg(theme.color(ColorToken::SurfaceBase))
            .text_color(theme.color(ColorToken::TextPrimary))
            .child(
                AnyView::from(self.rail.clone())
                    .cached(full_height().w(nomux_sidebar::rail_width(&theme))),
            )
            // Not cached: a cached view that renders again renders each
            // cached view in it again. The rows in it are cached.
            .child(self.sidebar.clone())
            .child(self.main_area(cx))
    }
}
