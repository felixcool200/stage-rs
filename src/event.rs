use crate::app::{App, DiffViewMode, Message, Overlay, Panel};
use crate::keymap::{self, InputContext};
use crate::text_input::TextInput;
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

    let remaining =
        Duration::from_secs(AUTO_REFRESH_SECS).saturating_sub(app.last_refresh.elapsed());
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
            Overlay::Confirm { .. } => keymap::resolve(InputContext::Confirm, key),
            Overlay::CommitInput { .. } => keymap::resolve(InputContext::CommitInput, key),
            Overlay::GitLog { .. } => keymap::resolve(InputContext::GitLog, key),
            Overlay::StashList { .. } => keymap::resolve(InputContext::StashList, key),
            Overlay::BranchList { .. } => keymap::resolve(InputContext::BranchList, key),
            Overlay::CommitDetail { .. } => keymap::resolve(InputContext::CommitDetail, key),
            Overlay::Rebase { .. } => keymap::resolve(InputContext::Rebase, key),
            Overlay::DirtyCheckout { has_conflicts, .. } => {
                keymap::resolve_dirty_checkout(key, *has_conflicts)
            }
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

/// Check if a key event should produce a Message for the overlay.
/// Used by the main loop to separate control keys from text editing keys.
pub fn poll_event_overlay_only(key: crossterm::event::KeyEvent) -> Option<Message> {
    keymap::resolve(InputContext::CommitInput, key)
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
