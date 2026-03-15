# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build                        # Debug build
cargo run                          # Build and run
cargo run -- /path/to/repo         # Run against specific repo
cargo test                         # Run all tests (7 tests in src/git/diff.rs)
cargo test test_apply_hunk         # Run tests matching name pattern
cargo clippy                       # Lint
cargo fmt                          # Format
```

## Architecture

**TEA (The Elm Architecture) pattern** — the entire app follows a unidirectional data flow:

1. **Event** → `event.rs` polls crossterm key events, resolves them via `keymap.rs` into a `Message`
2. **Update** → `app.rs` `App::update(Message)` handles all state transitions in a single match
3. **View** → `ui/` modules render the current `App` state as ratatui widgets (pure functions, no mutation)

The main loop in `main.rs` drives this cycle and has multiple input paths:
- Normal mode: keymap resolution via `event.rs`
- Commit input: raw text input to `TextInput`
- Edit mode: suspends TUI and launches `$EDITOR`
- Filter mode: character input for file search
- Branch creation: character input for branch name
- Conflict resolver: dedicated key handlers

When a modal overlay is active, key routing bypasses the normal keymap and goes through overlay-specific handlers in `event.rs`.

### Key modules

- **`app.rs`** — Central state (`App` struct), `Message` enum, `Overlay` enum for modal popups, `DiffState` for diff navigation, `TextInput` for commit message editing, `ConflictState` for merge resolution. This is the largest file and the heart of the app.
- **`git/`** — All git2 interactions behind `GitRepo` wrapper. `diff.rs` contains the diff computation engine using the `similar` crate plus hunk/line-level staging logic. `operations.rs` has stage/unstage/commit/stash/branch/remote operations. `log.rs` has commit log, blame, and commit diff. All unit tests live in `diff.rs`.
- **`keymap.rs`** — Table-driven key resolution with `InputContext` (FileList, DiffHunkNav, DiffLineNav) determining which bindings are active. Vim and Helix keymaps share navigation but differ in selection semantics.
- **`ui/`** — Pure rendering. Two-panel layout: file list (30 cols fixed) + side-by-side diff (50/50 split). Overlays render on top via `popup.rs`. `diff_view.rs` handles conflict resolver and blame annotations.
- **`theme.rs`** — Color theme system (default, Dracula). `Theme` struct defines all palette colors, diff backgrounds, and conflict colors.
- **`syntax.rs`** — Syntax highlighting via syntect. `Highlighter` wraps syntect and produces ratatui `Span`s with per-line coloring.

### Partial staging flow

Hunk and line staging don't use git's patch machinery. Instead, `apply_hunk()` / `apply_lines()` in `git/diff.rs` walk the diff output and reconstruct a new file by cherry-picking which changes to include, then `stage_content()` writes the result as a blob directly to the git index.

### Remote operations

Push, pull, and fetch shell out to `git` CLI rather than using git2's remote API, to leverage existing SSH keys and credential helpers. The TUI is suspended during these commands so SSH can prompt for passphrases.

### Clipboard

Yank operations pipe text to system clipboard tools (`wl-copy`, `xclip`, or `xsel`) — no clipboard library dependency.

## Dependencies

- **ratatui 0.29** + **crossterm 0.28** — TUI framework and terminal backend
- **git2 0.19** — libgit2 bindings (all git operations)
- **similar 2** — Text diffing engine (with `text` feature)
- **color-eyre 0.6** — Error handling
- **syntect 5** — Syntax highlighting (default syntaxes/themes, regex-onig backend)
- `instability` pinned to 0.3.7 in Cargo.lock for rustc 1.87 compatibility

## Special patterns to know

- `main.rs` has multiple `poll_*` functions for different input modes (text input, filter, which-key, branch creation). The main loop dispatches to the right one based on current app state.
- Auto-refresh (2 sec timer) is disabled while any overlay is open.
- `Overlay::Confirm` gates destructive operations (undo, amend, discard) behind a y/n dialog.
- `Overlay` has many variants: Confirm, CommitInput, GitLog, StashList, BranchList, CommitDetail, Rebase, DirtyCheckout. Each has its own key handler in `event.rs` and renderer in `popup.rs`.
- Interactive rebase uses a temp script as `GIT_SEQUENCE_EDITOR` to drive `git rebase -i`.
