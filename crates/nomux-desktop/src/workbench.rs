//! The root view of the window: the rail, the sidebar and the main area.
//! It routes the updates of the host to the model and the clicks to the
//! selection. The model and the terminal have their own types.

use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{
    AnyElement, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Task, Window, WindowAppearance, div,
};
use nomux_client::NodeKind;
use nomux_desktop_model::rail::TileTarget;
use nomux_desktop_model::{HostState, ProjectView, Selection, rail, sidebar};
use nomux_desktop_theme::{ColorToken, DesktopTheme, LengthToken, TextStyle};
use nomux_sidebar::ClickHandler;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::host_connection::{HostConnection, HostRequest, HostUpdate};
use crate::measure::Measurement;
use crate::terminal_tab::TerminalTab;

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

/// The root view.
pub struct Workbench {
    theme: DesktopTheme,
    appearance: Appearance,
    host: HostState,
    selection: Selection,
    connection: HostConnection,
    tab: Option<Entity<TerminalTab>>,
    /// The session that the user created; it is selected when the list of
    /// the sessions has it.
    created_session: Option<String>,
    failure: Option<String>,
    home: Option<PathBuf>,
    measurement: Option<Measurement>,
    _tasks: Vec<Task<()>>,
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
        cx.observe_window_appearance(window, |workbench, window, cx| {
            workbench.follow_appearance(window, cx);
        })
        .detach();
        Self {
            theme: DesktopTheme::for_appearance(is_dark(appearance, window.appearance())),
            appearance,
            host: HostState::default(),
            selection: Selection::default(),
            connection,
            tab: None,
            created_session: None,
            failure: None,
            home: std::env::var_os("HOME").map(PathBuf::from),
            measurement,
            _tasks: vec![updates_task],
        }
    }

    /// The terminal tab, if a session is selected.
    #[must_use]
    pub fn tab(&self) -> Option<&Entity<TerminalTab>> {
        self.tab.as_ref()
    }

    fn follow_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.theme = DesktopTheme::for_appearance(is_dark(self.appearance, window.appearance()));
        if let Some(tab) = &self.tab {
            let theme = self.theme;
            tab.update(cx, |tab, cx| tab.set_theme(&theme, cx));
        }
        cx.notify();
    }

    fn apply(&mut self, update: HostUpdate, window: &mut Window, cx: &mut Context<Self>) {
        match update {
            HostUpdate::Workspace(workspace) => self.host.set_workspace(workspace),
            HostUpdate::Sessions(sessions) => {
                self.host.set_sessions(sessions);
                if let Some(id) = self.created_session.take() {
                    self.select_session_of_host(&id);
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
        self.sync_tab(window, cx);
        cx.notify();
    }

    /// Shows the Project of a session and selects the session.
    fn select_session_of_host(&mut self, id: &str) {
        let Some(session) = self.host.session(id) else {
            self.created_session = Some(id.to_owned());
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

    /// Makes the terminal tab show the selected session. Dropping the old
    /// tab detaches it; its session continues.
    fn sync_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.selection.session();
        let current = self
            .tab
            .as_ref()
            .map(|tab| tab.read(cx).session_id().to_owned());
        if selected == current.as_deref() {
            return;
        }
        let Some(id) = selected.map(str::to_owned) else {
            self.tab = None;
            return;
        };
        let connection = self.connection.clone();
        let theme = self.theme;
        let measurement = self.measurement.clone();
        let tab = cx.new(|cx| TerminalTab::new(id, connection, &theme, measurement, window, cx));
        window.focus(&tab.focus_handle(cx), cx);
        self.tab = Some(tab);
    }

    fn click_tile(&mut self, target: &TileTarget, window: &mut Window, cx: &mut Context<Self>) {
        match target {
            TileTarget::Project(view) => self.selection.select_project(view.clone(), &self.host),
            TileTarget::Group(id) => self.selection.toggle_group(id),
        }
        self.sync_tab(window, cx);
        cx.notify();
    }

    fn click_session(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.selection.select_session(id);
        self.sync_tab(window, cx);
        if let Some(tab) = &self.tab {
            let focus = tab.focus_handle(cx);
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    /// Starts a session with the shell of the user in the directory of the
    /// shown Project, or in the home directory.
    fn new_session(&mut self) {
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

    fn main_area(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.theme;
        let Some(tab) = &self.tab else {
            return self.empty_state(cx);
        };
        let title = tab.read(cx).session_id().to_owned();
        let name = self
            .host
            .session(&title)
            .and_then(|session| session.command.first().cloned())
            .and_then(|program| {
                std::path::Path::new(&program)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or(title);
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(tab_bar(theme, &name))
            .child(div().flex_1().min_h_0().child(tab.clone()))
            .into_any_element()
    }

    fn empty_state(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.theme;
        let font = theme.font(TextStyle::Title);
        let meta = theme.font(TextStyle::Meta);
        let place = self
            .selection
            .project()
            .and_then(|view| self.host.workspace().tree.get(view.project_id()?))
            .map_or_else(
                || "your home directory".to_owned(),
                |node| node.name.clone(),
            );
        let on_new: ClickHandler<()> =
            Self::handler(cx, |workbench, (): &(), _, _| workbench.new_session());
        let message = self
            .failure
            .clone()
            .unwrap_or_else(|| format!("A new session starts a shell in {place}."));
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

/// The tab bar with the one tab: `surface.sunken` track, the active Tab in
/// `surface.raised` with the accent indicator.
fn tab_bar(theme: &DesktopTheme, title: &str) -> impl IntoElement {
    let metrics = theme.metrics();
    let font = theme.font(TextStyle::Title);
    let space_1 = theme.length(LengthToken::Space1);
    div()
        .flex_none()
        .flex()
        .items_end()
        .h(metrics.control_height + space_1)
        .px(space_1)
        .bg(theme.color(ColorToken::SurfaceSunken))
        .child(
            div()
                .relative()
                .flex()
                .items_center()
                .h(metrics.control_height)
                .px(metrics.padding)
                .rounded_t(theme.length(LengthToken::RadiusButton))
                .bg(theme.color(ColorToken::SurfaceRaised))
                .font_family(font.family)
                .font_weight(font.weight)
                .text_size(theme.font_size(TextStyle::Title))
                .text_color(theme.color(ColorToken::TextPrimary))
                .child(title.to_owned())
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top_0()
                        .h(theme.length(LengthToken::FocusRingWidth))
                        .bg(theme.color(ColorToken::Accent)),
                ),
        )
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
        let tiles = rail::rail(&self.host, &self.selection);
        let content = sidebar::sidebar(&self.host, &self.selection, now_ms(), self.home.as_deref());
        let on_tile: ClickHandler<TileTarget> = Self::handler(cx, Self::click_tile);
        let on_row: ClickHandler<String> =
            Self::handler(cx, |workbench, id: &String, window, cx| {
                workbench.click_session(id, window, cx)
            });
        let on_new: ClickHandler<()> =
            Self::handler(cx, |workbench, (): &(), _, _| workbench.new_session());
        div()
            .size_full()
            .flex()
            .bg(theme.color(ColorToken::SurfaceBase))
            .text_color(theme.color(ColorToken::TextPrimary))
            .child(nomux_sidebar::rail(&tiles, &theme, &on_tile))
            .child(nomux_sidebar::sidebar(
                content.as_ref(),
                &theme,
                &on_row,
                &on_new,
            ))
            .child(self.main_area(cx))
    }
}
