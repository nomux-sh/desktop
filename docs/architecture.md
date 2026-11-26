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
