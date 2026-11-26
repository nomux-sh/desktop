# desktop

The desktop client of nomux. It shows the sessions, the agents and the projects of one or more hosts in a native window. It is written in Rust with GPUI and connects to the nomux daemon; it starts no session process of its own.

This repository is a part of [nomux](https://github.com/nomux-sh), a native and lightweight agentic development environment. Sessions stay alive on the host when the client disconnects.

## Status

In development. Versions earlier than 1.0.0 are pre-releases: interfaces can change between minor versions.

### What works

- One window with the project rail, the sidebar of the selected Project, and terminal tabs: a tab for each open session, the platform shortcuts to open, close, switch and move tabs, and the same tabs at the next start ([Tabs](docs/architecture.md#tabs)).
- The client connects to the daemon of the local host and starts it if it does not run.
- The rail: Projects and Groups in the sequence of the user, Groups that open in place, the label rule of a project tile, the aggregate state of a closed Group, the selection bar.
- The sidebar: the Sessions of the Project with their state glyphs from the Agent runs and the events of the daemon (needs you first), the closed Child work of an agent ("+N"), the age. The sessions outside each Project have their own tile (`~`).
- Each terminal tab attaches to its session. Keys with modifiers, function keys, the application cursor mode, bracketed paste, text input and IME composition, mouse reports when the program asks for them, the wheel on the alternate screen, the wheel into the history, selection and copy, paste, resize.
- Closing the window detaches: the sessions continue (`nomux ls`).
- When no session exists, the client creates none: "New session" starts a shell in the directory of the selected Project or in the home directory.
- nomux Dark and nomux Light, from the appearance of the system.
- No work without a change: the working glyph turns and the cursor blinks only while they are visible, and not with reduced motion ([the frame budget](docs/architecture.md#the-frame-budget)).

### What is not built yet

- Worktree rows: the protocol has no worktrees yet. The Sessions are at level 1 of the tree without a worktree row above them.
- Splits, the settings, the agent view, diffs, extensions, remote hosts, packages (`.dmg`, `.deb`), Windows.
- Answers to the queries of a program (device attributes, cursor position, colours): `nomux-term` drops them (nomux-sh/nomux#89).
- The kitty keyboard protocol: keys use the xterm encoding. The application keypad mode: GPUI does not tell a keypad key from the same key of the main block, thus keypad keys send the bytes of the main keys.
- The shell chips and the team roster of the tab bar, the hover actions of a row (stop, menu), the tooltips, the expanded rail, the motion of a Group that opens, child rows of an agent, the compact density.

## Build and run

The toolchain is in `rust-toolchain.toml`. The client needs the daemon of nomux: `nomuxd`, `nomux-holder` and the `nomux` CLI of the revision in `Cargo.toml`, on `PATH`.

```sh
cargo run --release -p nomux-desktop
```

### Linux

Packages to build and to run (the X11 and Wayland keyboard libraries, the Vulkan loader, FreeType):

| Distribution | Packages |
|---|---|
| Fedora | `gcc gcc-c++ libxcb-devel libxkbcommon-devel libxkbcommon-x11-devel freetype-devel vulkan-loader` |
| Debian, Ubuntu | `build-essential libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev libfreetype-dev libvulkan1` |

The client needs a Vulkan driver. Without a GPU, install the software driver (lavapipe): `mesa-vulkan-drivers` on Fedora, Debian and Ubuntu. fontconfig is loaded at run time (`.cargo/config.toml`), thus its headers are not necessary.

### macOS

Xcode command line tools. The Metal shaders compile at run time, thus the Metal toolchain of Xcode is not necessary.

### Without the system packages: the development container

`tools/dev-container/` has a Fedora container with the packages, the toolchain, Xvfb, lavapipe, xdotool and screenshot tools. It needs `podman` (rootless). The first run builds the image.

```sh
tools/dev-container/run.sh cargo build
tools/dev-container/run.sh cargo nextest run --workspace
```

The repository is at `/work` in the container, and the cargo registry of the user is shared.

## Tests

```sh
cargo nextest run --workspace
```

The smoke test opens the window against a real daemon in a temporary HOME, waits for the text of a session, and saves a screenshot to `target/tmp/smoke.png`. It needs the nomux binaries on `PATH` and a Vulkan driver:

```sh
PATH="<nomux binaries>:$PATH" cargo nextest run -p nomux-desktop --test smoke --run-ignored only
```

## Measurements

`tools/measure.sh` builds the release binary and runs the hidden option `--measure <file>` on Linux under Xvfb: the time to the first frame, the resident memory, the wake-ups and the CPU time after 60 seconds without input, the time from a key to the paint of its echo, and the paint time of each frame of a flood. `crates/nomux-desktop/src/measure.rs` describes the scenario.

```sh
tools/dev-container/run.sh tools/measure.sh
```

## Crates

| Crate | Responsibility |
|---|---|
| `nomux-desktop` | The application: the window, the connection to the daemon, the terminal tabs. |
| `nomux-desktop-model` | The state without GPUI: workspace tree, sessions, Agent runs, selection, events. |
| `nomux-term-view` | The GPUI terminal element: paints a snapshot of `nomux-term`. No I/O. |
| `nomux-sidebar` | The views of the project rail and the sidebar. |
| `nomux-desktop-theme` | The nomux themes as GPUI values. |

Why this split: the model holds the rules of the client and must be testable without a window and without the long build of GPUI; the terminal element must not know the connection, so that later panes and tabs use it as it is; the views of the rail and the sidebar change with the design specification, not with the protocol; the theme crate is the one place where tokens become GPUI values. [docs/architecture.md](docs/architecture.md) describes the threads and the data flow.

### Dependencies

| Dependency | Why |
|---|---|
| `gpui-pre`, `gpui-pre-platform` `=0.3.14` | The UI framework (a weekly snapshot of the GPUI of Zed), and its platform backends. Exact pins: the pair of `gpui-component` 0.11.0. |
| `nomux-client`, `nomux-term` (git, exact revision) | The protocol client and the terminal state. Not on crates.io yet. |
| `nomux-theme` `=0.3.1` | The design tokens and the shipped themes. |
| `tokio` (`rt`, `sync`, `macros`) | The runtime of the connection thread: `nomux-client` is async. |

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option. The fonts in `assets/fonts/` are under the SIL Open Font License 1.1: see [NOTICE](NOTICE).

The security policy is in the [`.github`](https://github.com/nomux-sh/.github/blob/main/SECURITY.md) repository.
