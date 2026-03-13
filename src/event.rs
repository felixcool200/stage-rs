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
            Overlay::StashList { .. } => handle_stash_list(key.code),
            Overlay::BranchList { .. } => handle_branch_list(key.code),
            Overlay::CommitDetail { .. } => handle_commit_detail(key.code, key.modifiers),
            Overlay::Rebase { .. } => handle_rebase(key.code, key.modifiers),
            Overlay::DirtyCheckout { has_conflicts, .. } => handle_dirty_checkout(key.code, *has_conflicts),
            Overlay::None => unreachable!(),
        });
    }

    let ctx = match app.active_panel {
        Panel::FileList => InputContext::FileList,
        Panel::DiffView => {
            if app.conflict_state.is_some() {
                InputContext::ConflictNav
            } else {
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
        }
    };

    Ok(keymap::resolve(ctx, key))
}

/// Handle keys in the commit message input overlay.
/// Returns None for keys that modify the TextInput directly (handled in app.update via a
/// separate path — we call input methods from here for text editing keys).
fn handle_commit_input(modifiers: KeyModifiers, code: KeyCode) -> Option<Message> {
    match (modifiers, code) {
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Some(Message::CloseOverlay),
        // Ctrl+S or Ctrl+D to confirm
        (KeyModifiers::CONTROL, KeyCode::Char('s')) | (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
            Some(Message::ConfirmCommit)
        }
        (_, KeyCode::Esc) => Some(Message::CloseOverlay),
        _ => None, // Text editing keys handled separately
    }
}

fn handle_confirm(code: KeyCode) -> Option<Message> {
    match code {
        KeyCode::Char('y') | KeyCode::Enter => Some(Message::ConfirmAction),
        KeyCode::Char('n') | KeyCode::Esc | KeyCode::Char('q') => Some(Message::CloseOverlay),
        _ => None,
    }
}

fn handle_stash_list(code: KeyCode) -> Option<Message> {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => Some(Message::CloseOverlay),
        KeyCode::Down => Some(Message::MoveDown),
        KeyCode::Up => Some(Message::MoveUp),
        KeyCode::Char('p') | KeyCode::Enter => Some(Message::StashPop),
        KeyCode::Char('a') => Some(Message::StashApply),
        KeyCode::Char('d') => Some(Message::StashDrop),
        _ => None,
    }
}

fn handle_branch_list(code: KeyCode) -> Option<Message> {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => Some(Message::CloseOverlay),
        KeyCode::Down => Some(Message::MoveDown),
        KeyCode::Up => Some(Message::MoveUp),
        KeyCode::Enter => Some(Message::CheckoutBranch),
        KeyCode::Char('n') => Some(Message::StartCreateBranch),
        _ => None,
    }
}

fn handle_git_log(code: KeyCode) -> Option<Message> {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => Some(Message::CloseOverlay),
        KeyCode::Down => Some(Message::MoveDown),
        KeyCode::Up => Some(Message::MoveUp),
        KeyCode::Char('y') => Some(Message::YankToClipboard),
        KeyCode::Enter => Some(Message::ViewCommitDetail),
        KeyCode::Char('r') => Some(Message::StartRebase),
        _ => None,
    }
}

fn handle_rebase(code: KeyCode, modifiers: KeyModifiers) -> Option<Message> {
    match (modifiers, code) {
        (_, KeyCode::Esc) | (_, KeyCode::Char('q')) => Some(Message::CloseOverlay),
        (KeyModifiers::SHIFT, KeyCode::Down) => Some(Message::RebaseMoveDown),
        (KeyModifiers::SHIFT, KeyCode::Up) => Some(Message::RebaseMoveUp),
        (_, KeyCode::Down) => Some(Message::MoveDown),
        (_, KeyCode::Up) => Some(Message::MoveUp),
        (_, KeyCode::Char(' ') | KeyCode::Char('c')) => Some(Message::RebaseCycleAction),
        (_, KeyCode::Enter) => Some(Message::RebaseExecute),
        _ => None,
    }
}

fn handle_dirty_checkout(code: KeyCode, has_conflicts: bool) -> Option<Message> {
    match code {
        KeyCode::Char('s') if !has_conflicts => Some(Message::DirtyCheckoutStash),
        KeyCode::Char('d') => Some(Message::DirtyCheckoutDiscard),
        KeyCode::Esc | KeyCode::Char('c') | KeyCode::Char('q') => Some(Message::CloseOverlay),
        _ => None,
    }
}

fn handle_commit_detail(code: KeyCode, modifiers: KeyModifiers) -> Option<Message> {
    match (modifiers, code) {
        (_, KeyCode::Esc) | (_, KeyCode::Char('q')) => Some(Message::CloseOverlay),
        (KeyModifiers::SHIFT, KeyCode::Down) => Some(Message::NextCommitDetail),
        (KeyModifiers::SHIFT, KeyCode::Up) => Some(Message::PrevCommitDetail),
        (_, KeyCode::Down) => Some(Message::MoveDown),
        (_, KeyCode::Up) => Some(Message::MoveUp),
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
