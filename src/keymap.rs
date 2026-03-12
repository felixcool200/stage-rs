use crate::app::Message;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Context passed to the keymap resolver so it knows which mode we're in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputContext {
    FileList,
    DiffHunkNav,
    DiffLineNav,
}

/// Resolve a key event to a message given the current context.
pub fn resolve(ctx: InputContext, key: KeyEvent) -> Option<Message> {
    if key.modifiers == KeyModifiers::CONTROL {
        match key.code {
            KeyCode::Char('c') => return Some(Message::Quit),
            _ => {}
        }
    }

    // Shift+arrows: jump hunk-by-hunk in diff view
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        match key.code {
            KeyCode::Down => return Some(Message::NextHunk),
            KeyCode::Up => return Some(Message::PrevHunk),
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('q') => return Some(Message::Quit),
        KeyCode::Down => return Some(Message::MoveDown),
        KeyCode::Up => return Some(Message::MoveUp),
        KeyCode::Tab => return Some(Message::SwitchPanel),
        KeyCode::Char('r') => return Some(Message::Refresh),
        KeyCode::Char('c') => return Some(Message::OpenCommit),
        KeyCode::Char('C') => return Some(Message::OpenCommitAmend),
        KeyCode::Char('z') => return Some(Message::UndoLastCommit),
        KeyCode::Char('g') => return Some(Message::OpenGitLog),
        KeyCode::Char('y') => return Some(Message::YankToClipboard),
        KeyCode::Char('Z') => return Some(Message::StashSave),
        KeyCode::Char('W') => return Some(Message::OpenStashList),
        KeyCode::Char('b') => return Some(Message::OpenBranchList),
        KeyCode::Char('B') => return Some(Message::ToggleBlame),
        KeyCode::Char('R') => return Some(Message::OpenConflictResolver),
        KeyCode::Char('f') => return Some(Message::GitFetch),
        _ => {}
    }

    if ctx == InputContext::FileList {
        if key.code == KeyCode::Char('/') {
            return Some(Message::StartFilter);
        }
    }

    resolve_context(ctx, key)
}

fn resolve_context(ctx: InputContext, key: KeyEvent) -> Option<Message> {
    match ctx {
        InputContext::FileList => match key.code {
            KeyCode::Enter | KeyCode::Right => Some(Message::SelectFile),
            KeyCode::Char('s') => Some(Message::StageFile),
            KeyCode::Char('u') => Some(Message::UnstageFile),
            KeyCode::Char('d') => Some(Message::DiscardChanges),
            _ => None,
        },
        InputContext::DiffHunkNav => match key.code {
            KeyCode::Left => Some(Message::SwitchPanel),
            KeyCode::Enter | KeyCode::Right => Some(Message::EnterLineMode),
            KeyCode::Char('s') => Some(Message::StageHunk),
            KeyCode::Char('S') => Some(Message::StageFile),
            KeyCode::Char('u') => Some(Message::UnstageFile),
            KeyCode::Char('x') => Some(Message::SplitHunk),
            KeyCode::Char('e') => Some(Message::EnterEditMode),
            _ => None,
        },
        InputContext::DiffLineNav => match key.code {
            KeyCode::Char(' ') => Some(Message::ToggleLine),
            KeyCode::Char('a') => Some(Message::SelectAllLines),
            KeyCode::Char('s') => Some(Message::StageLines),
            KeyCode::Char('S') => Some(Message::StageFile),
            KeyCode::Esc | KeyCode::Left => Some(Message::ExitLineMode),
            _ => None,
        },
    }
}
