//! The choices of the context menus and of the list of Projects.

mod sample;

use nomux_desktop_model::menu::{
    KILL_DETAIL, MenuCommand, MenuTarget, entries, kill_question, project_entries,
};

fn commands(target: &MenuTarget) -> Vec<MenuCommand> {
    entries(target)
        .into_iter()
        .map(|entry| entry.command)
        .collect()
}

#[test]
fn a_row_with_a_tab_can_close_it_and_a_row_without_one_cannot() {
    let with_tab = MenuTarget::Session {
        id: "a".to_owned(),
        has_tab: true,
    };
    assert_eq!(
        commands(&with_tab),
        [
            MenuCommand::OpenInTab("a".to_owned()),
            MenuCommand::RenameSession("a".to_owned()),
            MenuCommand::CloseTab("a".to_owned()),
            MenuCommand::KillSession("a".to_owned()),
        ]
    );
    let without_tab = MenuTarget::Session {
        id: "a".to_owned(),
        has_tab: false,
    };
    assert!(
        !commands(&without_tab).contains(&MenuCommand::CloseTab("a".to_owned())),
        "a row without a tab has nothing to close"
    );
}

#[test]
fn only_the_kill_choice_is_destructive_and_it_says_that_it_asks() {
    let target = MenuTarget::Session {
        id: "a".to_owned(),
        has_tab: true,
    };
    let destructive: Vec<String> = entries(&target)
        .into_iter()
        .filter(|entry| entry.destructive)
        .map(|entry| entry.label)
        .collect();
    assert_eq!(destructive, ["Kill session..."]);
}

#[test]
fn a_tab_offers_close_others_only_when_there_are_other_tabs() {
    let one = MenuTarget::Tab {
        id: "a".to_owned(),
        tab_count: 1,
    };
    assert_eq!(commands(&one), [MenuCommand::CloseTab("a".to_owned())]);
    let three = MenuTarget::Tab {
        id: "a".to_owned(),
        tab_count: 3,
    };
    assert_eq!(
        commands(&three),
        [
            MenuCommand::CloseTab("a".to_owned()),
            MenuCommand::CloseOtherTabs("a".to_owned()),
        ]
    );
}

#[test]
fn a_project_tile_starts_a_session_and_removes_the_project() {
    let target = MenuTarget::Project {
        id: "web".to_owned(),
    };
    let labels: Vec<String> = entries(&target)
        .into_iter()
        .map(|entry| entry.label)
        .collect();
    assert_eq!(labels, ["New session", "Remove from sidebar"]);
    assert_eq!(
        commands(&target),
        [
            MenuCommand::NewSessionInProject("web".to_owned()),
            MenuCommand::RemoveProject("web".to_owned()),
        ]
    );
}

#[test]
fn the_list_of_projects_has_each_project_and_no_group_in_the_sequence_of_the_tree() {
    let host = sample::host(vec![], vec![]);
    let names: Vec<String> = project_entries(&host)
        .into_iter()
        .map(|entry| entry.label)
        .collect();
    assert_eq!(
        names,
        [
            "acme-web",
            "acme-admin",
            "acme-mobile",
            "acme-shop-2019",
            "globex-api",
            "dotfiles",
            "docs"
        ]
    );
    let first = project_entries(&host).remove(0);
    assert_eq!(
        first.command,
        MenuCommand::NewSessionInProject("web".to_owned())
    );
}

#[test]
fn the_question_before_a_kill_names_the_session() {
    assert_eq!(kill_question("deploy"), "Kill session \"deploy\"?");
    assert!(KILL_DETAIL.contains("cannot be undone"));
}
