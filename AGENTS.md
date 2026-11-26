# AGENTS.md

Rules for coding agents and for people who work in this repository. `CLAUDE.md` and `QWEN.md` refer to this file.

## This repository

`nomux-sh/desktop` is the desktop client of nomux: a GPUI application that shows the Projects, the sessions and the Agent runs of the daemon of the local host, and a terminal attached to a session. [docs/architecture.md](docs/architecture.md) describes the threads and the data flow.

| Path | Responsibility |
|---|---|
| `crates/nomux-desktop/` | The application: the binary, the window root (`Workbench`), the connection thread to the daemon, the attach, the terminal tab, the fonts and the measurement mode. |
| `crates/nomux-desktop/tests/smoke.rs` | The smoke test: a real daemon in a temporary HOME, the window, a headless screenshot. |
| `crates/nomux-desktop-model/` | The state of the client without GPUI: the workspace tree, the sessions, the Agent runs, the selection, the events, the content of the rail and of the sidebar. |
| `crates/nomux-term-view/` | The GPUI terminal element: the snapshot of `nomux-term`, the paint plan, the glyph cache, the selection and the encoding of the input. No I/O. |
| `crates/nomux-sidebar/` | The views of the project rail and of the sidebar, from the model and the theme tokens. |
| `crates/nomux-desktop-theme/` | The shipped nomux themes of `nomux-theme` as GPUI values. |
| `assets/fonts/` | The fonts of the `font.ui` and `font.mono` tokens (Inter, JetBrains Mono) and their licenses. |
| `tools/dev-container/` | The container for a computer without the system packages of GPUI. |
| `tools/measure.sh` | Runs the measurement mode (`--measure`) on Linux. |
| `Cargo.toml`, `deny.toml`, `rust-toolchain.toml` | Workspace members and shared versions, license and source policy, pinned toolchain. |
| `cliff.toml`, `CHANGELOG.md` | Changelog configuration and the changelog. |
| `.github/workflows/` | `ci.yml` (checks, the smoke test, the macOS build), `pull-request.yml` (sign-off, commit messages). |

## Rules for this repository

- The client has no colour, length, font or duration of its own. Each one is a token of `nomux-theme`. The geometry of the rail and the sidebar is `docs/sidebar.md` of the [`themes`](https://github.com/nomux-sh/themes) repository.
- `nomux-desktop-model` has no GPUI and no I/O. Put state, rules and sequences there and test them there.
- `nomux-term-view` does no I/O. The terminal is fed on the connection thread, never on the UI thread. The UI takes one snapshot for each frame.
- Each input of a terminal tab goes through `TerminalTab::write`.
- The window root (`Workbench`) routes updates and clicks. It holds no terminal logic and no model rules.
- GPUI is the `gpui-pre` line with exact pins. Bump `gpui-pre` and `gpui-pre-platform` together, with the smoke test and a run on a Mac. `deny.toml` bans the other GPUI lines.
- Do not copy code from the GPL crates of Zed (`terminal`, `terminal_view`, `ui`, `rope`, `theme` and the others) and do not read them for an implementation.
- `nomux-client`, `nomux-core` and `nomux-term` come from the `nomux` repository by git with an exact `rev`. Move the revision of the three together, and build the nomux binaries of the same revision for the smoke test.
- A missing feature of the daemon, the client crates or the protocol goes to its repository. Do not work around it in the client.
- No `unsafe` code.
- Pin each action in a workflow to a full commit SHA with a version comment.

## Checks

Run these commands from the repository root before a push. CI runs the same commands. On a computer without the system packages of GPUI, run each `cargo` command in the development container: `tools/dev-container/run.sh <command>`.

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked
cargo test --workspace --doc --locked
PATH="<nomux binaries>:$PATH" cargo nextest run -p nomux-desktop --test smoke --run-ignored only
cargo deny check
cargo machete
actionlint
```

If you changed `AGENTS.md`, also run this command with a clone of the [`.github`](https://github.com/nomux-sh/.github) repository next to this repository:

```sh
sh ../.github/standards/sync.sh . && git diff --exit-code
```

<!-- nomux:common:start -->
## Organization rules

These rules apply to each repository of the nomux-sh organization. The source of this block is `standards/AGENTS.common.md` in the [`.github`](https://github.com/nomux-sh/.github) repository. Do not edit the block in other repositories. CI compares it with the source.

### Before you change something

- Read the code around the change. Use the names, the structure and the style that the repository already has.
- Use the terms of the [glossary](https://github.com/nomux-sh/docs/blob/main/src/content/docs/reference/glossary.md). One concept has one name in code, CLI, UI and documentation.
- An architecture change needs an ADR in the [`docs`](https://github.com/nomux-sh/docs) repository before the implementation.
- Check versions, APIs and file paths before you use them. Do not guess.

### Scope

- Make only the change that the task asks for. Do not add options, abstractions or helpers that the task does not need.
- Do not add a placeholder that looks like an implementation. If work is deferred, say it in the pull request.
- Validate input at system boundaries. Trust internal code.

### Code

- Keep it simple (KISS). Build only what the task needs now (YAGNI). Have one source for each piece of logic (DRY).
- Write modern, readable code. Do not write clever code that the reader must decode.
- One type, one module and one file have one responsibility. Do not make a type that knows or does everything. Split it before it grows.
- Keep the file tree tidy. A new file goes where its neighbours have the same responsibility. Do not make catch-all directories or files such as `utils`, `common`, `helpers` or `misc`.
- Give each file, type and function a name that says what it is or does. Do not use abbreviations that the glossary does not have.
- Keep functions short and at one level of abstraction. Do not nest deeply. Return early.
- Do not duplicate code. If the same logic is necessary in a second place, move it to one place and call it.
- Comments: none by default. Write one line only for a reason that the code cannot show.
- Do not swallow errors. Do not add retries without a known cause.
- The performance targets of the project come before features. A change that breaks a target is not merged.
- Do not copy code from GPL, AGPL or Commons Clause projects.

### Tests

- Complete the code first. Write the tests last, and run them when the work is complete.
- Write the tests that matter: edge cases, behaviour that can break later, and the proof that the current behaviour is correct.
- Do not write a test only to have a test. Do not test what is obvious or what the compiler already guarantees.
- A test must fail when the behaviour that it guards is removed or moved.
- Do not mock the thing that the test examines.

### CI

- CI must be fast. Reviews and CI are the only things that slow a merge.
- Cache dependencies and build output. Run independent jobs in parallel. Cancel a run when a newer commit arrives.
- Put slow work (long fuzz runs, benchmarks, full matrix builds) in scheduled runs, not in pull request runs.
- When you add a CI step, measure the time that it adds and say it in the pull request.

### Text

- Write code, comments, documentation, commit messages and pull requests in English.
- Use short sentences, the active voice and one word for one concept. Put the condition before the instruction.
- Do not use these words: delve, robust, seamless, seamlessly, leverage, utilize, harness, pivotal, intricate, furthermore, moreover, notably, cutting-edge, best-in-class.

### Commits

- One subject line: `type: description`. No scope. 72 characters at most.
- Types: `build`, `chore`, `ci`, `docs`, `feat`, `fix`, `perf`, `refactor`, `revert`, `style`, `test`. Add `!` before the colon for a breaking change.
- The description says what changed, in plain English. Do not name a phase, a milestone or a ticket.
- Sign off each commit with `git commit -s`. `Signed-off-by` is the only trailer.
- Do not add `Co-authored-by`, a tool name or a "generated with" line to a commit or to a pull request.
- Enable the commit hook one time in each clone: `git config core.hooksPath .githooks`.

### Pull requests

- One logical change in each pull request. The branch name is short and names the change.
- The title has the format of a commit subject, 70 characters at most.
- The description says what changed and why, then shows the commands that you ran and their real result.
- Each commit in the pull request obeys the commit rules. The maintainers merge with a rebase, thus each commit goes to `main` unchanged.
- Do not merge when a check fails. Do not disable a check to make it pass.

### Never commit

- Secrets, tokens, keys or passwords.
- Paths, host names or account names of a local machine.
- Generated build output, editor files or local configuration.

### Before you push

1. Run the checks that the "Checks" section of this file lists.
2. Read your diff from top to bottom.
3. Make sure that the changelog and the documentation agree with the change.
<!-- nomux:common:end -->
