# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

[git-cliff](https://git-cliff.org) writes this file from the commit subjects.
To see the changes that are not in a version, run `git cliff --unreleased`.

## [0.1.1] - 2026-11-27

### Added

- Add menus, Add Project, context menus and the quit shortcut
- List the sessions that need you across all projects
- Show a system notification when a session needs you or is done

### Fixed

- Ask an older daemon to restart instead of showing a protocol error
- Focus the window so menu items and shortcuts work with no tab
## [0.1.0] - 2026-11-26

### Added

- Add the model of the workspace, sessions and Agent runs
- Add the terminal element that paints a nomux-term snapshot
- Add the project rail and the sidebar views
- Add the window with one terminal tab attached to a session
- Add the rules of motion and of animated rows to the model
- Add the open tabs and their saved state to the model
- Open sessions in terminal tabs and restore them at the start
- Match the tab bar, pane and terminal to the theme preview
- Tell how to stop a daemon of an other version
- Add a build option that writes a frame to a PNG file
- Add an option that sets the size of the window
- Draw the theme under a transparent title bar on macOS

### Fixed

- Show the session name in the rows and the tabs

### Performance

- Paint a view only when it changes and stop idle clocks
- Measure the parts of key to paint without extra frames
- Spread the keys of the measurement over the frame period
- Turn the working glyph in four steps each turn
- Measure key to paint with 60 keys for a stable median

### Documentation

- Describe the client, its architecture and the agent rules
- Write the frame budget rules and measure on Wayland
- Describe the tabs, their shortcuts and their state file
- Describe the packages and the first run on macOS
- Describe key to paint, the memory of the client and its budget
- Describe what 0.1.0 has, its requirements and the bundled core
