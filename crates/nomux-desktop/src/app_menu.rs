//! The menu bar of the application and the actions that need no window:
//! quit, hide, the configuration file and the documentation.
//!
//! macOS shows the menus in the menu bar. The key bindings come first:
//! the menu shows the keys of the bindings that exist when it is installed.

use gpui::{App, Menu, MenuItem, OsAction};

use crate::{
    About, AddProject, CloseTab, Copy, Hide, Minimize, NewSession, NewSessionInProject, NextTab,
    OpenDocumentation, OpenSettings, Paste, PreviousTab, Quit, SelectAll, ToggleSidebar, Zoom,
};

/// The address of the documentation.
const DOCUMENTATION: &str = "https://nomux-sh.github.io/docs/";

/// Installs the menu bar and the handlers of the actions without a window.
pub fn install_menus(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &OpenDocumentation, cx| cx.open_url(DOCUMENTATION));
    cx.on_action(|_: &OpenSettings, cx| open_settings(cx));
    cx.set_menus(menus());
}

/// The menus: nomux, File, Edit, View, Window and Help. The first menu is
/// the application menu of macOS.
fn menus() -> Vec<Menu> {
    vec![
        Menu::new("nomux").items([
            MenuItem::action("About nomux", About),
            MenuItem::separator(),
            MenuItem::action("Settings...", OpenSettings),
            MenuItem::separator(),
            MenuItem::action("Hide nomux", Hide),
            MenuItem::separator(),
            MenuItem::action("Quit nomux", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("New Session", NewSession),
            MenuItem::action("New Session in Project...", NewSessionInProject),
            MenuItem::action("Add Project...", AddProject),
            MenuItem::separator(),
            MenuItem::action("Close Tab", CloseTab),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Copy", Copy, OsAction::Copy),
            MenuItem::os_action("Paste", Paste, OsAction::Paste),
            MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
        ]),
        Menu::new("View").items([
            MenuItem::action("Toggle Sidebar", ToggleSidebar),
            MenuItem::separator(),
            MenuItem::action("Next Tab", NextTab),
            MenuItem::action("Previous Tab", PreviousTab),
        ]),
        Menu::new("Window").items([
            MenuItem::action("Minimize", Minimize),
            MenuItem::action("Zoom", Zoom),
        ]),
        Menu::new("Help").items([MenuItem::action("Documentation", OpenDocumentation)]),
    ]
}

/// Shows the configuration file in the file manager. A file that does not
/// exist yet shows its closest folder.
fn open_settings(cx: &mut App) {
    let Some(file) = nomux_core::configuration::default_configuration_path() else {
        return;
    };
    if file.exists() {
        cx.reveal_path(&file);
    } else if let Some(folder) = file.ancestors().find(|folder| folder.is_dir()) {
        cx.open_with_system(folder);
    }
}
