use crate::app::{App, DiffViewMode, Message, Overlay, Panel, TextInput};
use crate::keymap::{self, InputContext};
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::time::Duration;

const AUTO_REFRESH_SECS: u64 = 2;

pub fn poll_event(app: &App) -> Result<Option<Message>> {
    // Don't auto-refresh while overlay is open
    if !app.overlay.is_active()
        && app.last_refresh.elapsed() >= Duration::from_secs(AUTO_REFRESH_SECS)
    {
        return Ok(Some(Message::AutoRefresh));
    }

    let remaining = Duration::from_secs(AUTO_REFRESH_SECS)
        .saturating_sub(app.last_refresh.elapsed());
    let timeout = remaining.min(Duration::from_millis(250));

    if !event::poll(timeout)? {
        return Ok(None);
    }

    let Event::Key(key) = event::read()? else {
        return Ok(None);
    };

    if key.kind != crossterm::event::KeyEventKind::Press {
        return Ok(None);
    }

    // Route to overlay handlers when active
    if app.overlay.is_active() {
        return Ok(match &app.overlay {
            Overlay::Confirm { .. } => handle_confirm(key.code),
            Overlay::CommitInput { .. } => handle_commit_input(key.modifiers, key.code),
            Overlay::GitLog { .. } => handle_git_log(key.code),
            Overlay::None => unreachable!(),
        });
    }

    let ctx = match app.active_panel {
        Panel::FileList => InputContext::FileList,
        Panel::DiffView => {
            let in_line_mode = app
                .diff_state
                .as_ref()
                .map(|ds| ds.view_mode == DiffViewMode::LineNav)
                .unwrap_or(false);
            if in_line_mode {
                InputContext::DiffLineNav
            } else {
                InputContext::DiffHunkNav
            }
        }
    };

    Ok(keymap::resolve(app.keymap, ctx, key))
}

/// Handle keys in the commit message input overlay.
/// Returns None for keys that modify the TextInput directly (handled in app.update via a
/// separate path — we call input methods from here for text editing keys).
fn handle_commit_input(modifiers: KeyModifiers, code: KeyCode) -> Option<Message> {
    match (modifiers, code) {
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Some(Message::CloseOverlay),
        // Ctrl+Enter or Ctrl+D to confirm
        (KeyModifiers::CONTROL, KeyCode::Enter) | (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
            Some(Message::ConfirmCommit)
        }
        (_, KeyCode::Esc) => Some(Message::CloseOverlay),
        _ => None, // Text editing keys handled separately
    }
}

fn handle_confirm(code: KeyCode) -> Option<Message> {
    match code {
        KeyCode::Char('y') | KeyCode::Enter => Some(Message::ConfirmAction),
        KeyCode::Char('n') | KeyCode::Esc => Some(Message::CloseOverlay),
        _ => None,
    }
}

fn handle_git_log(code: KeyCode) -> Option<Message> {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => Some(Message::CloseOverlay),
        KeyCode::Char('j') | KeyCode::Down => Some(Message::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::MoveUp),
        KeyCode::Char('y') => Some(Message::YankToClipboard),
        _ => None,
    }
}

/// Check if a key event should produce a Message for the overlay.
/// Used by the main loop to separate control keys from text editing keys.
pub fn poll_event_overlay_only(key: crossterm::event::KeyEvent) -> Option<Message> {
    handle_commit_input(key.modifiers, key.code)
}

/// Process raw key events for the commit input text area.
/// Called directly from the main loop when the commit overlay is active
/// and the key was not consumed by handle_commit_input.
pub fn apply_text_input_key(input: &mut TextInput, modifiers: KeyModifiers, code: KeyCode) {
    match (modifiers, code) {
        (_, KeyCode::Char(ch)) if modifiers.is_empty() || modifiers == KeyModifiers::SHIFT => {
            input.insert_char(ch);
        }
        (_, KeyCode::Backspace) => input.backspace(),
        (_, KeyCode::Enter) => input.insert_newline(),
        (_, KeyCode::Left) => input.move_left(),
        (_, KeyCode::Right) => input.move_right(),
        (_, KeyCode::Up) => input.move_up(),
        (_, KeyCode::Down) => input.move_down(),
        (_, KeyCode::Home) => input.move_home(),
        (_, KeyCode::End) => input.move_end(),
        _ => {}
    }
}
