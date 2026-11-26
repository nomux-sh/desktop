//! The tiles of the project rail: labels, aggregate states, open Groups and
//! the selection.

mod sample;

use nomux_client::{AgentState, SessionState};
use nomux_desktop_model::rail::{RailTile, TileKind, rail};
use nomux_desktop_model::{GlyphState, ProjectView, Selection};

fn find<'a>(tiles: &'a [RailTile], name: &str) -> &'a RailTile {
    search(tiles, name).unwrap_or_else(|| panic!("no tile {name}"))
}

fn search<'a>(tiles: &'a [RailTile], name: &str) -> Option<&'a RailTile> {
    tiles.iter().find_map(|tile| {
        if tile.name == name {
            return Some(tile);
        }
        match &tile.kind {
            TileKind::OpenGroup { children, .. } => search(children, name),
            _ => None,
        }
    })
}

fn label(tile: &RailTile) -> &str {
    match &tile.kind {
        TileKind::Project { label, .. } => label,
        other => panic!("not a project tile: {other:?}"),
    }
}

#[test]
fn labels_follow_the_rule_of_the_specification() {
    let host = sample::host(vec![], vec![]);
    let mut selection = Selection::default();
    for group in ["acme", "2019", "personal"] {
        selection.toggle_group(group);
    }
    let tiles = rail(&host, &selection);
    let labels: Vec<(&str, &str)> = [
        "acme-web",
        "acme-admin",
        "acme-mobile",
        "acme-shop-2019",
        "globex-api",
        "dotfiles",
        "docs",
    ]
    .iter()
    .map(|name| (*name, label(find(&tiles, name))))
    .collect();
    assert_eq!(
        labels,
        [
            ("acme-web", "We"),
            ("acme-admin", "Ad"),
            ("acme-mobile", "Mo"),
            ("acme-shop-2019", "Sh"),
            ("globex-api", "GA"),
            ("dotfiles", "Do"),
            ("docs", "Dc"),
        ]
    );
}

#[test]
fn a_closed_group_shows_the_most_urgent_state_and_its_grid() {
    let host = sample::host(
        vec![
            sample::session("a", "/home/user/acme-web", "bash", SessionState::Running),
            sample::session(
                "b",
                "/home/user/acme-shop-2019",
                "bash",
                SessionState::Running,
            ),
        ],
        vec![
            sample::run("a", AgentState::Working),
            sample::run("b", AgentState::NeedsYou(None)),
        ],
    );
    let tiles = rail(&host, &Selection::default());
    let TileKind::ClosedGroup {
        grid,
        project_count,
        state,
    } = &find(&tiles, "Acme").kind
    else {
        panic!("Acme is closed");
    };
    // Three Projects and the Group 2019: filled, filled, filled, hollow.
    assert_eq!(grid, &[true, true, true, false]);
    assert_eq!(*project_count, 4);
    assert_eq!(*state, Some(GlyphState::NeedsYou));
}

#[test]
fn an_open_group_has_no_state_and_its_children_keep_their_state() {
    let host = sample::host(
        vec![sample::session(
            "a",
            "/home/user/acme-web",
            "bash",
            SessionState::Running,
        )],
        vec![sample::run("a", AgentState::Error)],
    );
    let mut selection = Selection::default();
    selection.toggle_group("acme");
    let tiles = rail(&host, &selection);
    let acme = find(&tiles, "Acme");
    assert!(matches!(&acme.kind, TileKind::OpenGroup { initial, .. } if initial == "A"));
    assert!(matches!(
        find(&tiles, "acme-web").kind,
        TileKind::Project {
            state: Some(GlyphState::Error),
            ..
        }
    ));
    // The Group 2019 in Acme stays closed: the state of each Group is its own.
    assert!(matches!(
        find(&tiles, "2019").kind,
        TileKind::ClosedGroup { .. }
    ));
}

#[test]
fn a_closed_group_that_holds_the_selected_project_is_selected() {
    let host = sample::host(vec![], vec![]);
    let mut selection = Selection::default();
    selection.select_project(ProjectView::Project("shop".to_owned()), &host);
    let closed = rail(&host, &selection);
    assert!(find(&closed, "Acme").selected);
    selection.toggle_group("acme");
    let open = rail(&host, &selection);
    assert!(!find(&open, "Acme").selected);
    assert!(find(&open, "2019").selected);
    selection.toggle_group("2019");
    let both_open = rail(&host, &selection);
    assert!(find(&both_open, "acme-shop-2019").selected);
}
