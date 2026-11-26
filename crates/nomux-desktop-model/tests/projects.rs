//! Finding a Project by its folder, and the name of a new Project.

mod sample;

use std::path::Path;

use nomux_desktop_model::project_name;

#[test]
fn a_folder_that_is_a_project_is_found_and_another_folder_is_not() {
    let host = sample::host(vec![], vec![]);
    assert_eq!(
        host.project_with_directory(Path::new("/home/user/acme-web")),
        Some("web")
    );
    assert_eq!(
        host.project_with_directory(Path::new("/home/user/other")),
        None
    );
    // A folder inside a Project is not the Project.
    assert_eq!(
        host.project_with_directory(Path::new("/home/user/acme-web/src")),
        None
    );
}

#[test]
fn the_directory_of_a_project_is_known_and_a_group_has_none() {
    let host = sample::host(vec![], vec![]);
    assert_eq!(
        host.project_directory("web"),
        Some(Path::new("/home/user/acme-web"))
    );
    assert_eq!(host.project_directory("acme"), None);
    assert_eq!(host.project_directory("missing"), None);
}

#[test]
fn a_new_project_is_named_after_its_folder() {
    assert_eq!(project_name(Path::new("/home/user/acme-web")), "acme-web");
    assert_eq!(project_name(Path::new("/home/user/acme-web/")), "acme-web");
    assert_eq!(project_name(Path::new("/")), "/");
}
