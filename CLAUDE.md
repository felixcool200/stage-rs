# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build                        # Debug build
cargo run                          # Build and run
cargo run -- /path/to/repo -k vim  # Run against specific repo with keymap
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

The main loop in `main.rs` drives this cycle. When a modal overlay is active (commit input, git log, confirm dialog), key routing bypasses the normal keymap and goes through overlay-specific handlers in `event.rs`.

### Key modules

- **`app.rs`** — Central state (`App` struct), `Message` enum, `Overlay` enum for modal popups, `DiffState` for diff navigation, `TextInput` for commit message editing. This is the largest file and the heart of the app.
- **`git/`** — All git2 interactions behind `GitRepo` wrapper. `diff.rs` contains the diff computation engine using the `similar` crate plus hunk/line-level staging logic (reconstructs file content by selectively applying changes, writes as blob to index). All unit tests live here.
- **`keymap.rs`** — Table-driven key resolution with `InputContext` (FileList, DiffHunkNav, DiffLineNav) determining which bindings are active. Vim and Helix keymaps share navigation but differ in selection semantics.
- **`ui/`** — Pure rendering. Two-panel layout: file list (30 cols fixed) + side-by-side diff (50/50 split). Overlays render on top via `popup.rs`.

### Partial staging flow

Hunk and line staging don't use git's patch machinery. Instead, `apply_hunk()` / `apply_lines()` in `git/diff.rs` walk the diff output and reconstruct a new file by cherry-picking which changes to include, then `stage_content()` writes the result as a blob directly to the git index.

## Dependencies

- **ratatui 0.29** + **crossterm 0.28** — TUI framework and terminal backend
- **git2 0.19** — libgit2 bindings (all git operations)
- **similar 2** — Text diffing engine (with `text` feature)
- **color-eyre 0.6** — Error handling
- `instability` pinned to 0.3.7 in Cargo.lock for rustc 1.87 compatibility

## Special patterns to know

- `poll_with_text_input()` in `main.rs` exists because commit message editing needs raw character input, while normal mode routes through the keymap resolver. The main loop switches between these two paths based on whether `Overlay::CommitInput` is active.
- Auto-refresh (2 sec timer) is disabled while any overlay is open.
- `Overlay::Confirm` gates destructive operations (undo last commit, amend) behind a y/n dialog before proceeding.
