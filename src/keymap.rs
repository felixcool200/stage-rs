use crate::app::Message;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeymapName {
    Vim,
    Helix,
}

impl KeymapName {
    pub fn label(&self) -> &'static str {
        match self {
            KeymapName::Vim => "vim",
            KeymapName::Helix => "helix",
        }
    }

    pub fn cycle(&self) -> Self {
        match self {
            KeymapName::Vim => KeymapName::Helix,
            KeymapName::Helix => KeymapName::Vim,
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "vim" => Some(KeymapName::Vim),
            "helix" => Some(KeymapName::Helix),
            _ => None,
        }
    }
}

/// Context passed to the keymap resolver so it knows which mode we're in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputContext {
    FileList,
    DiffHunkNav,
    DiffLineNav,
}

/// Resolve a key event to a message given the active keymap and context.
pub fn resolve(keymap: KeymapName, ctx: InputContext, key: KeyEvent) -> Option<Message> {
    // Global bindings (shared by all keymaps)
    if key.modifiers == KeyModifiers::CONTROL {
        match key.code {
            KeyCode::Char('c') => return Some(Message::Quit),
            KeyCode::Char('k') => return Some(Message::CycleKeymap),
            _ => {}
        }
    }

    // Shared navigation (both keymaps use h/j/k/l and arrows)
    match key.code {
        KeyCode::Char('q') => return Some(Message::Quit),
        KeyCode::Char('j') | KeyCode::Down => return Some(Message::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => return Some(Message::MoveUp),
        KeyCode::Tab => return Some(Message::SwitchPanel),
        KeyCode::Char('r') => return Some(Message::Refresh),
        // Commit / log (shared across keymaps and contexts)
        KeyCode::Char('c') => return Some(Message::OpenCommit),
        KeyCode::Char('C') => return Some(Message::OpenCommitAmend),
        KeyCode::Char('z') => return Some(Message::UndoLastCommit),
        KeyCode::Char('g') => return Some(Message::OpenGitLog),
        KeyCode::Char('y') => return Some(Message::YankToClipboard),
        _ => {}
    }

    match keymap {
        KeymapName::Vim => resolve_vim(ctx, key),
        KeymapName::Helix => resolve_helix(ctx, key),
    }
}

// ── Vim ──────────────────────────────────────────────────────────────────────

fn resolve_vim(ctx: InputContext, key: KeyEvent) -> Option<Message> {
    match ctx {
        InputContext::FileList => match key.code {
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => Some(Message::SelectFile),
            KeyCode::Char('s') => Some(Message::StageFile),
            KeyCode::Char('u') => Some(Message::UnstageFile),
            KeyCode::Char('d') => Some(Message::DiscardChanges),
            _ => None,
        },
        InputContext::DiffHunkNav => match key.code {
            KeyCode::Char('h') | KeyCode::Left => Some(Message::SwitchPanel),
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => Some(Message::EnterLineMode),
            KeyCode::Char('s') => Some(Message::StageHunk),
            KeyCode::Char('S') => Some(Message::StageFile),
            KeyCode::Char('u') => Some(Message::UnstageFile),
            _ => None,
        },
        InputContext::DiffLineNav => match key.code {
            KeyCode::Char(' ') => Some(Message::ToggleLine),
            KeyCode::Char('a') => Some(Message::SelectAllLines),
            KeyCode::Char('s') => Some(Message::StageLines),
            KeyCode::Char('S') => Some(Message::StageFile),
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => Some(Message::ExitLineMode),
            _ => None,
        },
    }
}

// ── Helix ────────────────────────────────────────────────────────────────────
//
// Key philosophy differences from Vim:
//   - `v`  enters select (line) mode  → our EnterLineMode
//   - `x`  selects current line       → our ToggleLine
//   - `X`  extends selection           → our SelectAllLines
//   - `Esc` is the universal "go back"
//   - `l`/`Right` don't drill down (they're pure movement in Helix)
//   - `Enter` confirms selections

fn resolve_helix(ctx: InputContext, key: KeyEvent) -> Option<Message> {
    match ctx {
        InputContext::FileList => match key.code {
            KeyCode::Enter => Some(Message::SelectFile),
            KeyCode::Char('l') | KeyCode::Right => Some(Message::SelectFile),
            KeyCode::Char('s') => Some(Message::StageFile),
            KeyCode::Char('u') => Some(Message::UnstageFile),
            KeyCode::Char('d') => Some(Message::DiscardChanges),
            _ => None,
        },
        InputContext::DiffHunkNav => match key.code {
            KeyCode::Char('h') | KeyCode::Left | KeyCode::Esc => Some(Message::SwitchPanel),
            KeyCode::Char('v') | KeyCode::Enter => Some(Message::EnterLineMode),
            KeyCode::Char('s') => Some(Message::StageHunk),
            KeyCode::Char('S') => Some(Message::StageFile),
            KeyCode::Char('u') => Some(Message::UnstageFile),
            _ => None,
        },
        InputContext::DiffLineNav => match key.code {
            KeyCode::Char('x') => Some(Message::ToggleLine),
            KeyCode::Char('X') => Some(Message::SelectAllLines),
            KeyCode::Char('s') => Some(Message::StageLines),
            KeyCode::Char('S') => Some(Message::StageFile),
            KeyCode::Esc => Some(Message::ExitLineMode),
            KeyCode::Char('h') | KeyCode::Left => Some(Message::ExitLineMode),
            _ => None,
        },
    }
}
