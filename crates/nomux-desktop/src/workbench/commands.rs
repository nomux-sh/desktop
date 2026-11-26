//! What the context menus, the menu bar and the dialogs ask the window to do:
//! adding and removing Projects, renaming and ending sessions, and the
//! overlays that these use. The rules that decide which choices exist are in
//! `nomux_desktop_model::menu`.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{AnyElement, AppContext, Context, Focusable, Pixels, Point, PromptLevel, Window, point};
use nomux_desktop_model::menu::{self, MenuCommand, MenuTarget};
use nomux_desktop_model::rail::TileTarget;
use nomux_desktop_model::{ProjectView, project_name, sidebar};

use super::{OpenMenu, Workbench};
use crate::host_connection::HostRequest;
use crate::rename_dialog::{CancelHandler, RenameDialog, SubmitHandler};

impl Workbench {
    /// Shows the context menu of `target` at `position`.
    pub(super) fn open_context_menu(
        &mut self,
        target: &MenuTarget,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.context_menu = Some(OpenMenu {
            position,
            entries: menu::entries(target),
        });
        cx.notify();
    }

    pub(super) fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        if self.context_menu.take().is_some() {
            cx.notify();
        }
    }

    /// The menu that the window shows, as an element.
    pub(super) fn context_menu_element(&self) -> Option<AnyElement> {
        let open = self.context_menu.as_ref()?;
        Some(nomux_sidebar::context_menu(
            &self.theme,
            open.position,
            &open.entries,
            &self.on_choose,
            &self.on_dismiss,
        ))
    }

    /// Does what a choice of a menu says.
    pub(super) fn run_command(
        &mut self,
        command: MenuCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.context_menu = None;
        match command {
            MenuCommand::OpenInTab(id) => self.open_in_tab(&id, window, cx),
            MenuCommand::RenameSession(id) => self.ask_new_name(&id, window, cx),
            MenuCommand::CloseTab(id) => {
                self.tabs.tabs_mut().close(&id);
                self.follow_active_tab(window, cx);
            }
            MenuCommand::CloseOtherTabs(id) => {
                self.tabs.tabs_mut().close_others(&id);
                self.follow_active_tab(window, cx);
            }
            MenuCommand::KillSession(id) => self.confirm_kill(id, window, cx),
            MenuCommand::NewSessionInProject(id) => {
                self.click_tile(&TileTarget::Project(ProjectView::Project(id)), window, cx);
                self.new_session();
            }
            MenuCommand::RemoveProject(id) => {
                self.connection.send(HostRequest::RemoveProject { id });
            }
        }
        cx.notify();
    }

    fn open_in_tab(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.tabs.tabs_mut().open(id);
        self.follow_active_tab(window, cx);
        if let Some(terminal) = self.tabs.active_terminal() {
            window.focus(&terminal.focus_handle(cx), cx);
        }
    }

    /// Asks for a folder with the dialog of the system and adds it as a
    /// Project. A folder that is a Project already is shown.
    pub(super) fn choose_project_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add project".into()),
        });
        cx.spawn_in(window, async move |workbench, cx| {
            let result = chosen.await;
            let _ = workbench.update_in(cx, |workbench, window, cx| match result {
                Ok(Ok(Some(paths))) => {
                    if let Some(directory) = paths.into_iter().next() {
                        workbench.add_folder(directory, window, cx);
                    }
                }
                Ok(Err(error)) => workbench.report_failure(error.to_string(), window, cx),
                Ok(Ok(None)) | Err(_) => {}
            });
        })
        .detach();
    }

    fn add_folder(&mut self, directory: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let known = self
            .host
            .project_with_directory(&directory)
            .map(str::to_owned);
        match known {
            Some(id) => self.click_tile(&TileTarget::Project(ProjectView::Project(id)), window, cx),
            None => self.connection.send(HostRequest::AddProject {
                name: project_name(&directory),
                directory,
            }),
        }
    }

    /// "New Session in Project...": lists the Projects, and the choice starts
    /// the session. Without a Project it asks for a folder.
    pub(super) fn choose_project_for_session(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entries = menu::project_entries(&self.host);
        if entries.is_empty() {
            return self.choose_project_folder(window, cx);
        }
        let metrics = self.theme.metrics();
        self.context_menu = Some(OpenMenu {
            position: point(
                nomux_sidebar::rail_width(&self.theme),
                metrics.control_height * 2.0,
            ),
            entries,
        });
        cx.notify();
    }

    /// Asks "Kill session ...?" in a dialog of the system. Only the answer
    /// "Kill session" ends the session.
    fn confirm_kill(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = sidebar::session_label(&self.host, &id, self.home.as_deref()) else {
            return;
        };
        let answer = window.prompt(
            PromptLevel::Warning,
            &menu::kill_question(&name),
            Some(menu::KILL_DETAIL),
            &["Cancel", "Kill session"],
            cx,
        );
        cx.spawn_in(window, async move |workbench, cx| {
            if answer.await == Ok(KILL_ANSWER) {
                let _ = workbench.update_in(cx, |workbench, window, cx| {
                    workbench.kill_session(id, window, cx);
                });
            }
        })
        .detach();
    }

    fn kill_session(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.tabs.tabs_mut().close(&id);
        self.follow_active_tab(window, cx);
        self.connection.send(HostRequest::KillSession { id });
    }

    /// Shows the dialog that asks for the new name of a session.
    fn ask_new_name(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.host.session(id) else {
            return;
        };
        let current = session.name.clone().unwrap_or_default();
        let workbench = cx.entity().downgrade();
        let on_submit: SubmitHandler = {
            let workbench = workbench.clone();
            let id = id.to_owned();
            Rc::new(move |name, window, cx| {
                let workbench = workbench.clone();
                let id = id.clone();
                window.defer(cx, move |window, cx| {
                    let _ = workbench.update(cx, |workbench, cx| {
                        workbench.finish_rename(id, name, window, cx);
                    });
                });
            })
        };
        let on_cancel: CancelHandler = Rc::new(move |window, cx| {
            let workbench = workbench.clone();
            window.defer(cx, move |window, cx| {
                let _ = workbench.update(cx, |workbench, cx| workbench.close_rename(window, cx));
            });
        });
        let theme = self.theme;
        let dialog = cx.new(|cx| RenameDialog::new(theme, &current, (on_submit, on_cancel), cx));
        window.focus(&dialog.focus_handle(cx), cx);
        self.rename = Some(dialog);
        cx.notify();
    }

    fn finish_rename(
        &mut self,
        id: String,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_rename(window, cx);
        self.connection
            .send(HostRequest::RenameSession { id, name });
    }

    /// Closes the dialog and gives the focus back to the terminal.
    fn close_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.rename = None;
        if let Some(terminal) = self.tabs.active_terminal() {
            window.focus(&terminal.focus_handle(cx), cx);
        }
        cx.notify();
    }

    /// A request or the connection failed. Before the first list of the
    /// sessions the main area says it; after it, a dialog of the system
    /// does, because the main area shows a terminal.
    pub(super) fn report_failure(
        &mut self,
        message: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.loaded {
            self.failure = Some(message);
            return;
        }
        drop(window.prompt(PromptLevel::Warning, "nomux", Some(&message), &["OK"], cx));
    }
}

/// The index of "Kill session" in the answers of the question.
const KILL_ANSWER: usize = 1;

#[cfg(feature = "frame-capture")]
impl Workbench {
    /// Opens a context menu for the picture of the capture mode.
    pub fn open_menu_for_capture(
        &mut self,
        which: crate::capture::CaptureMenu,
        cx: &mut Context<Self>,
    ) {
        use crate::capture::CaptureMenu;

        let rail = nomux_sidebar::rail_width(&self.theme);
        let bar = crate::tab_bar::tab_bar_height(&self.theme);
        let row = self.theme.metrics().row_height;
        let active = self.tabs.tabs().active().map(str::to_owned);
        let (target, position) = match which {
            CaptureMenu::Session => {
                let Some(id) = self.selection.session().map(str::to_owned) else {
                    return;
                };
                let has_tab = self.tabs.tabs().sessions().contains(&id);
                (
                    MenuTarget::Session { id, has_tab },
                    point(rail + row * 3.0, bar + row),
                )
            }
            CaptureMenu::Tab => {
                let Some(id) = active else {
                    return;
                };
                let tab_count = self.tabs.tabs().sessions().len();
                (
                    MenuTarget::Tab { id, tab_count },
                    point(rail + row * 11.0, bar / 2.0),
                )
            }
            CaptureMenu::Project => {
                let Some(id) = self
                    .selection
                    .project()
                    .and_then(ProjectView::project_id)
                    .map(str::to_owned)
                else {
                    return;
                };
                (MenuTarget::Project { id }, point(rail, bar))
            }
        };
        self.open_context_menu(&target, position, cx);
    }
}
