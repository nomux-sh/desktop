# desktop

The desktop client of nomux. It shows the sessions, the agents and the projects of one or more hosts in a native window. It is written in Rust with GPUI and connects to the nomux daemon; it starts no session process of its own.

This repository is a part of [nomux](https://github.com/nomux-sh), a native and lightweight agentic development environment. Sessions stay alive on the host when the client disconnects.

## Status

Version 0.1.0 is the first pre-release. Versions earlier than 1.0.0 are pre-releases: interfaces can change between minor versions.

### What works

- One window with the project rail, the sidebar of the selected Project, and terminal tabs: a tab for each open session, the platform shortcuts to open, close, switch and move tabs, and the same tabs at the next start ([Tabs](docs/architecture.md#tabs)).
- The client connects to the daemon of the local host and starts it if it does not run.
- The menu bar of macOS: nomux (About, Settings..., Hide, Quit), File (New Session, New Session in Project..., Add Project..., Close Tab), Edit (Copy, Paste, Select All), View (Toggle Sidebar, Next Tab, Previous Tab), Window (Minimize, Zoom) and Help (Documentation). Cmd-Q quits the application, and so does the close button of the last window. The sessions continue in the daemon. Settings... shows the configuration file of nomux in the file manager ([Menus and dialogs](docs/architecture.md#menus-and-dialogs)).
- Add Project: a folder dialog of the system (Cmd-O, the `+` at the bottom of the rail, or the button of the empty state) adds the folder as a Project and selects it.
- Context menus (right click): a Session row (open in tab, rename, close tab, kill with a question), a tab (close, close others) and a Project tile (new session, remove from sidebar). A row with an open tab has a small mark.
- The rail: Projects and Groups in the sequence of the user, Groups that open in place, the label rule of a project tile, the aggregate state of a closed Group, the selection bar.
- The sidebar: the Sessions of the Project with their name (the name of the directory when the session has none) and their program, their state glyphs from the Agent runs and the events of the daemon (needs you first), the closed Child work of an agent ("+N"), the age. The sessions outside each Project have their own tile (`~`).
- The attention list at the top of the sidebar: the sessions of all Projects that need you, then the ones that are done and not seen, then the ones that work (oldest state first). It is hidden when empty. Cmd-J (macOS) or Ctrl-Shift-J (Linux) opens the tab of the next session that needs you, in any Project ([Attention list](docs/architecture.md#attention-list)).
- Each terminal tab attaches to its session. Keys with modifiers, function keys, the application cursor mode, bracketed paste, text input and IME composition, mouse reports when the program asks for them, the wheel on the alternate screen, the wheel into the history, selection and copy, paste, resize.
- A program that asks the terminal (device attributes, cursor position) gets the answer from the session holder of nomux; a colour query gets a fixed dark palette.
- Closing the window detaches: the sessions continue (`nomux ls`).
- When no session exists, the client creates none: "New session" starts a shell in the directory of the selected Project or in the home directory.
- nomux Dark and nomux Light, from the appearance of the system. On macOS the theme reaches the top edge of the window: the window buttons are at the top of the rail, and the line of the sidebar header and the tab bar is the title bar.
- No work without a change: the working glyph turns and the cursor blinks only while they are visible, and not with reduced motion ([the frame budget](docs/architecture.md#the-frame-budget)).

### What is not built yet

- Worktree rows: the protocol has no worktrees yet. The Sessions are at level 1 of the tree without a worktree row above them.
- Splits, a settings window, the agent view, diffs, extensions, remote hosts, Linux packages (`.deb`, `.rpm`), a notarised macOS image, automatic updates, Windows.
- The kitty keyboard protocol: keys use the xterm encoding. The application keypad mode: GPUI does not tell a keypad key from the same key of the main block, thus keypad keys send the bytes of the main keys.
- The foreground program of a shell session (the protocol has no such field: a row shows the program of the command, and the tool that an adapter or the daemon reports), the shell chips and the team roster of the tab bar, the hover actions of a row, the tooltips, the expanded rail, the motion of a Group that opens, child rows of an agent, the compact density.
- A session that the CLI starts or ends outside the client appears or goes away only at the next event of the host: the protocol has no event for it (nomux-sh/proto#23).
- A lost connection to the daemon shows a message instead of the last screen (#4). No list of the sessions that need you across all Projects (#2), no notifications (#3).

## Install

Each release has a disk image for macOS on Apple silicon (`nomux-<version>-macos-arm64.dmg`), one for macOS on Intel (`nomux-<version>-macos-x86_64.dmg`), an archive for Linux x86_64 (`nomux-desktop-<version>-x86_64-unknown-linux-gnu.tar.gz`) and `SHA256SUMS`. The releases before 1.0.0 are pre-releases.

### System requirements

- macOS 12 or later, on Apple silicon (`arm64` image) or on Intel (`x86_64` image). Tested on macOS 26 on Apple silicon. The Intel image was not tested on an Intel Mac.
- Linux x86_64 with X11 or Wayland and a Vulkan driver. The archive has no package for a distribution.

### The bundled core and an installed nomux

Each package has the nomux core of the same version: `nomux`, `nomuxd` and `nomux-holder` are next to the client binary (in `nomux.app/Contents/MacOS` on macOS). When no daemon runs, the client starts this `nomuxd`. Do not install the core a second time for the client.

Check the download before you open it:

```sh
shasum -a 256 -c SHA256SUMS --ignore-missing
```

### First run on macOS

The disk image has an ad-hoc signature and is not notarised by Apple. Thus macOS stops the first start. Do these steps one time:

1. Open the disk image and drag `nomux` to `Applications`.
2. Open `nomux` in `Applications`. macOS shows the alert "“nomux” Not Opened": Apple could not verify that the application is free of malware. Click **Done**.
3. Open **System Settings**, then **Privacy & Security**. Scroll down to **Security**. The text says that "nomux" was blocked. Click **Open Anyway**, then give your password or Touch ID.
4. Open `nomux` again and click **Open**. The next starts do not ask again.

On macOS 14 and earlier, step 2 and 3 can also be: Control-click `nomux` in `Applications`, select **Open**, then click **Open**. macOS 15 and later do not accept this path.

As an alternative to steps 2 to 4, remove the quarantine attribute in Terminal. Do this only for a disk image whose checksum you checked:

```sh
xattr -dr com.apple.quarantine /Applications/nomux.app
```

To use the `nomux` CLI in a terminal, make a link to the CLI of the application in a directory of your `PATH`, for example:

```sh
mkdir -p ~/.local/bin
ln -sf /Applications/nomux.app/Contents/MacOS/nomux ~/.local/bin/nomux
```

### A daemon of a different version

The client and the daemon agree on a protocol version when they connect. Version 0.1.0 has the core of nomux 0.5.0 with the query replies of the holder (a revision of `main` after the tag `v0.5.0`).

- If a daemon of nomux 0.5.0 or later already runs (for example from Homebrew or `cargo install`), the client uses it, and the client and your terminal show the same sessions.
- If the running daemon and the client have no common protocol version, the window shows the message of the daemon and the command that stops it: `nomux daemon stop`. The sessions continue without the daemon. Open the client again: it starts its own `nomuxd`.
- One host has one daemon. Use one `nomux` CLI: the one of the application (see the link above) or the one that you installed. Do not run two daemons of different versions.

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
| `nomux-client`, `nomux-core`, `nomux-term` (git, exact revision) | The protocol client, the place of the configuration file, and the terminal state. Not on crates.io yet. |
| `nomux-theme` `=0.3.1` | The design tokens and the shipped themes. |
| `tokio` (`rt`, `sync`, `macros`) | The runtime of the connection thread: `nomux-client` is async. |

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option. The fonts in `assets/fonts/` are under the SIL Open Font License 1.1: see [NOTICE](NOTICE).

The security policy is in the [`.github`](https://github.com/nomux-sh/.github/blob/main/SECURITY.md) repository.
