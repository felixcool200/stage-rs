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

    // Global navigation
    match key.code {
        KeyCode::Char('q') => return Some(Message::Quit),
        KeyCode::Down => return Some(Message::MoveDown),
        KeyCode::Up => return Some(Message::MoveUp),
        KeyCode::Tab => return Some(Message::SwitchPanel),
        KeyCode::Char(' ') => return Some(Message::OpenWhichKey),
        _ => {}
    }

    // Context-specific navigation
    match ctx {
        InputContext::FileList => match key.code {
            KeyCode::Enter | KeyCode::Right => Some(Message::SwitchPanel),
            KeyCode::Char('/') => Some(Message::StartFilter),
            _ => None,
        },
        InputContext::DiffHunkNav => match key.code {
            KeyCode::Left => Some(Message::SwitchPanel),
            KeyCode::Enter | KeyCode::Right => Some(Message::EnterLineMode),
            _ => None,
        },
        InputContext::DiffLineNav => match key.code {
            KeyCode::Enter | KeyCode::Right => Some(Message::ToggleLine),
            KeyCode::Esc | KeyCode::Left => Some(Message::ExitLineMode),
            _ => None,
        },
    }
}
