# stage-rs

A fast TUI git client with side-by-side diffs, built in Rust.

![MIT License](https://img.shields.io/badge/license-MIT-blue.svg)

## Features

- **Side-by-side diff view** with syntax highlighting
- **Hunk and line-level staging** — stage exactly what you want
- **Merge conflict resolver** — side-by-side view with pick ours/theirs/both
- **Interactive rebase** from the git log
- **Branch management** — switch, create, delete branches with dirty-worktree handling
- **Git blame** annotations inline with diffs
- **Stash** support (save, pop, apply, drop, list)
- **Inline editing** — open `$EDITOR` at the right line
- **Clipboard yank** — copy file names, hunks, or selected lines
- **File search/filter** with `/`
- **Context-sensitive actions** via Space (which-key style popup)
- **Auto-refresh** every 2 seconds

## Install

```
cargo install --path .
```

## Usage

```
stage-rs              # current directory
stage-rs /path/to/repo
```

### Navigation

| Key | Action |
|-----|--------|
| `↑`/`↓` | Navigate files / hunks / lines |
| `→`/`Enter` | Drill in: file list → diff → line mode |
| `←` | Back out: line mode → diff → file list |
| `Space` | Open actions menu |
| `/` | Filter file list |
| `q` | Quit (or back out of line mode / overlays) |
| `Ctrl+C` | Quit |

## License

MIT
