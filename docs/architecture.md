# Architecture of the desktop client

This document shows the threads of the client, the path of the terminal data from the daemon to a painted frame, and the crates. The daemon side is in [`docs/architecture.md`](https://github.com/nomux-sh/nomux/blob/main/docs/architecture.md) of the `nomux` repository.

## Threads

| Thread | Owner | What it does |
|---|---|---|
| UI thread | GPUI | Events of the window, the views, layout and paint. It never reads a socket and never feeds a terminal. |
| `nomux-client` | `HostConnection` | A single-threaded tokio runtime. It connects to the daemon (and starts it if it does not run, as the CLI does), sends the requests, receives the events of the host, and runs each attach. |
| GPUI platform threads | GPUI | The background executor and the renderer of the platform. |

The two sides talk through channels:

- UI to the connection thread: `HostRequest` (load the state again, create a session, attach) and, for each attach, `TabInput` (bytes, a new size).
- Connection thread to the UI: `HostUpdate` (workspace tree, sessions, Agent runs, events, a failure) and, for each attach, `TabUpdate` (the screen changed, connection lost or back, the end).

The UI side receives on a GPUI task. A tokio channel does not need a tokio runtime to receive, thus the UI thread runs no runtime.

## From the daemon to a painted frame

```
daemon ──frames──▶ TerminalOutput::next  (connection thread)
                      │ Snapshot bytes / Output bytes
                      ▼
                   SharedScreen::feed ── Mutex<nomux_term::Terminal> (with 5000 history rows)
                      │ sets `changed`; only if it was false:
                      ▼
                   TabUpdate::Changed ──channel──▶ TerminalTab (UI thread): cx.notify()
                                                      │ render(): one snapshot for each frame
                                                      ▼
                                                   TerminalSnapshot::capture (the rows of the view)
                                                      ▼
                                                   TerminalElement::paint
                                                      PaintPlan::build → quads, glyphs, lines, cursor
```

- The first content of an attach is the redraw sequence of the daemon (`TerminalEvent::Snapshot`): the terminal goes to its initial state and applies it. The output then continues from the position of the snapshot.
- The connection thread wakes the UI only when the `changed` flag goes from false to true. The UI clears the flag when it takes the snapshot in `render`. Thus a flood of output makes at most one wake and one snapshot for each frame, and the frames that the UI does not paint are dropped.
- The snapshot copies only the rows that the view shows: the screen, or history rows above it after a scroll of the wheel.
- The paint plan merges cells of one background colour into one quad for each run. Each cell paints the glyphs of its symbol at its own column, from the glyph cache. The cache key is the symbol and its style; the colour comes at paint time. Thus a frame shapes no line and a symbol is shaped one time.

## From a key to the daemon

```
key down ─▶ TerminalTab::on_key_down ─▶ input::key_bytes(keystroke, modes)
              │ a named key or Ctrl/Alt: bytes            │ plain text: no bytes
              ▼                                           ▼
        TerminalTab::write ◀──────────── EntityInputHandler::replace_text_in_range (IME, text)
              │
              ▼
        TabInput::Bytes ──channel──▶ TerminalInput::send (connection thread) ─▶ daemon
```

- Each input goes through `TerminalTab::write`: keys, text, paste, mouse reports, focus reports.
- The modes of the program (application cursor, bracketed paste, mouse tracking and encoding, focus reporting, alternate scroll) come from the last snapshot.
- A new size waits 100 ms without a further change, then goes to the attach: the attach resizes its terminal and sends the size to the daemon on a second connection, as the CLI does.
- The attach ends when the tab drops its input sender: that is the detach. Closing the window ends the process; the sessions continue in the daemon.

## Tabs

The main area has one terminal tab for each open session. `Tabs` in the model holds the sequence and the active tab; `TerminalTabs` in `nomux-desktop` holds one `TerminalTab` for each of them, the tab bar and the state file. The `Workbench` routes the clicks and the key actions to them.

- A click on a Session row shows the session in the active tab. With the platform modifier (Cmd on macOS, Ctrl on Linux) the click opens the session in a new tab after the active one, as in a web browser. A session is in one tab at most: a click on a session that has a tab activates that tab.
- Closing a tab drops its `TerminalTab`, which detaches. The session continues. The tab at the right becomes active, else the one at the left.
- Only the active tab is visible. A hidden tab keeps its attach: the connection thread feeds its terminal, and the tab asks for no frame. Its `changed` flag stays set, thus its attach wakes the UI one time until the tab is shown again, and the first frame after the switch takes one snapshot. A hidden tab that was never shown attaches with the grid size of the active tab.
- The sidebar follows the active tab: a tab that becomes active selects its Project and its row. A click on a Project tile activates the first tab of a session of that Project; it closes no tab.

| Action | macOS | Linux |
|---|---|---|
| New session in a new tab, in the directory of the shown Project | Cmd-T | Ctrl-Shift-T |
| Close the active tab (the session continues) | Cmd-W | Ctrl-Shift-W |
| Next tab, previous tab | Cmd-Shift-], Cmd-Shift-[ | Ctrl-PageDown, Ctrl-PageUp |
| Move the active tab to the left, to the right | Cmd-Shift-Left, Cmd-Shift-Right | Ctrl-Shift-PageUp, Ctrl-Shift-PageDown |
| Tab 1 to 8, the last tab | Cmd-1 to Cmd-8, Cmd-9 | Alt-1 to Alt-8, Alt-9 |
| Open a session in a new tab | Cmd-click on its row | Ctrl-click on its row |
| Close a tab | the close mark, which takes the place of the state glyph below the pointer, or a middle click | the same |

On Linux, Alt-1 to Alt-9 go to the tabs, not to the program in the terminal, as in GNOME Terminal.

The client writes the open tabs after each change, on a background thread, and opens them again at the next start: the tabs of sessions that ended are left out. The file has one session identifier for each line, the active one with the prefix `* `:

- macOS: `~/Library/Application Support/sh.nomux.desktop/tabs`
- Linux: `$XDG_STATE_HOME/nomux-desktop/tabs`, by default `~/.local/state/nomux-desktop/tabs`

## The frame budget

The client does no work when nothing changes. A frame costs a wake-up of the UI thread, a layout and a paint, thus each rule below removes frames, not only paint time.

### Rules

- **No frame without a change.** A view renders again only when what it shows changes: the tiles of the rail, the header and the sequence of the sidebar rows, the values of one row, the session and the title of the main area, the screen of the terminal. No view calls `request_animation_frame`.
- **Output in batches.** The connection thread feeds the terminal and wakes the UI only when the `changed` flag goes from false to true. The tab takes one snapshot for each frame and clears the flag. Thus a flood of output makes one notify for each frame, not one for each read.
- **A clock runs only while something that animates is visible.** There are two clocks, and each one stops by itself:
  - The working glyph turns at a constant speed, one turn in `motion.indicator`. The clock of the `Workbench` wakes the window 15 times for each turn (`WORKING_STEPS_PER_TURN`), and only while a working glyph is in the rail or the sidebar, the window is visible and the user did not ask for reduced motion. A tick renders only the rows and the rail that show the glyph.
  - The cursor blinks only when the program asks for a blinking cursor (`DECSCUSR`), the tab has the focus, the window is visible and the user did not ask for reduced motion. One cycle is `motion.indicator`: half shown, half hidden. The blink ends with the cursor shown 10 cycles (`BLINK_CYCLES`) after the last change of the screen or of the focus.
- **Reduced motion.** GPUI does not read the setting of the system (`App::reduce_motion` changes only with `set_reduce_motion`). The client reads it one time at the start on a background thread (`defaults read com.apple.universalaccess reduceMotion` on macOS, `gsettings get org.gnome.desktop.interface enable-animations` on Linux), and `--reduce-motion` fixes it. The window has motion until the answer arrives. With reduced motion the working glyph is the static arc and the cursor does not blink.
- **The ages** of the sidebar are computed again one time each minute. Only a row whose age text changed renders again.
- **Hidden tabs** paint nothing and run no clock ("Tabs" above).
- **Caches.** The rail, each Session row, the tab bar and the active terminal tab are cached views (`AnyView::cached`): a frame paints them from the last frame while they were not notified and their bounds are the same. The sidebar and the `Workbench` are not cached views, because of two rules of GPUI 0.3.14: a notify marks each view above the notified one dirty (`Window::mark_view_dirty`), and a cached view that renders again renders each cached view in it again (`prepaint_view` in `view.rs` sets `refreshing`). Thus a change of one row renders the `Workbench` and the sidebar, which are small, and that row; the other rows, the rail and the terminal come from the cache.

### What each platform backend does when nothing changes

The rules above make a frame without a change unnecessary. Whether the UI thread still wakes depends on the frame source of the GPUI backend (`gpui-pre-linux`, `gpui-pre-macos` 0.3.14):

| Backend | Frame source | Wake-ups of an idle window |
|---|---|---|
| X11 | A timer at the refresh rate for each visible window: `update_refresh_loop` and `start_refresh_loop` in `src/linux/x11/client.rs` (lines 1969 and 2050). It starts when the window becomes visible and stops only when it is hidden or covered. Each tick calls the frame callback of GPUI, which draws nothing when no view is dirty (`window.rs`, `on_request_frame`). | One for each refresh. The client cannot stop the timer: GPUI has no option for it. The frames are not drawn. |
| Wayland | Frame callbacks of the compositor. After a frame without a draw the frame loop parks (`complete_frame` in `src/linux/wayland/window.rs`) and waits until a view is dirty. | None. |
| macOS 14 and later | A `CAMetalDisplayLink` that pauses after an update that did not draw and did not ask for a frame (`MetalFrames::end_frame` in `src/display_link.rs`). | None. |
| macOS 12 and 13 | The shared `CVDisplayLink` while the window is visible. | One for each refresh, as on X11. |

### Tests

- `nomux-sidebar/tests/row_paints.rs` counts the paints of each Session row for a sequence of events of the host: only the row whose values changed paints, a tick of the clock paints only the working row, and a frame without a change paints nothing.
- The smoke test checks that the window asks for no frame in 2 seconds without a change, after it showed the screen of a session.
- `nomux-desktop-model/tests/motion.rs` has the rules of the clocks.

## Crates

| Crate | Depends on | Responsibility |
|---|---|---|
| `nomux-desktop` | all crates below, `gpui`, `gpui_platform`, `nomux-client`, `nomux-term`, `tokio` | The binary and the window: `Workbench` (the root view), `HostConnection` and the attach, `TerminalTab`, the fonts, the measurement mode. |
| `nomux-desktop-model` | `nomux-client` (types only) | The state without GPUI: `HostState` (applies events, says what to load again), `Selection`, the content of the rail and of the sidebar, the label rule of a project tile, the state glyph and its priority. |
| `nomux-term-view` | `gpui`, `nomux-term` | The terminal element: `TerminalSnapshot`, `PaintPlan`, `GlyphCache`, `TerminalElement`, the input encoding, the selection. No I/O. |
| `nomux-sidebar` | `gpui`, `nomux-desktop-model`, `nomux-desktop-theme` | The views of the rail and of the sidebar, and the state glyphs. No state of their own. |
| `nomux-desktop-theme` | `gpui`, `nomux-theme` | The shipped nomux themes as GPUI values: colours, lengths, fonts, density. |

The split keeps the parts that change for different reasons apart, and it keeps the slow-to-compile GPUI out of the model: the model tests build without a window. The terminal element has no dependency on the connection, thus a later pane or a second tab uses it without change.

## The theme

The client uses `nomux Dark` and `nomux Light` from `nomux-theme` and follows the appearance of the system (`--appearance dark|light` fixes one). The density is comfortable. The fonts of the `font.ui` and `font.mono` tokens (Inter, JetBrains Mono) are in the binary.

The reference picture is the preview page of the themes (`nomux-theme preview`): the window follows its "Workspace" figure. The tab bar, the line `line.divider` between the sidebar and the pane, the terminal padding `space.3`, and the terminal row height come from it. The row height of the terminal is 1.5 × `font.code.size`, the value of the terminal of the preview page; `nomux-theme` 0.3.1 has no token for it, thus it is one function of the theme crate (`terminal_line_height`).

On macOS the window has a transparent title bar (`title_bar.rs`): the surfaces of the theme reach the top edge of the window. The window buttons sit at the top of the project rail, at the left edge of the rail tiles, centred on the line of the tab bar. The rail starts below that line, and the sidebar header text starts after the window buttons (they are 60 pt wide on macOS 26, wider than the rail). Where the line has no control (the top of the rail, the sidebar header, the tab bar after the last Tab) it is the title bar: a drag moves the window, and a double click does the action of the system setting (zoom, minimize or nothing). In full screen the window buttons are hidden, and the rail and the sidebar header go back to the positions of the specification. Linux keeps the title bar of the system.
