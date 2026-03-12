use crate::app::{App, DiffViewMode, Message, Panel};
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

const AUTO_REFRESH_SECS: u64 = 2;

pub fn poll_event(app: &App) -> Result<Option<Message>> {
    if app.last_refresh.elapsed() >= Duration::from_secs(AUTO_REFRESH_SECS) {
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

    if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
        return Ok(Some(Message::Quit));
    }

    match app.active_panel {
        Panel::FileList => Ok(handle_file_list(key)),
        Panel::DiffView => {
            let in_line_mode = app
                .diff_state
                .as_ref()
                .map(|ds| ds.view_mode == DiffViewMode::LineNav)
                .unwrap_or(false);
            if in_line_mode {
                Ok(handle_line_mode(key))
            } else {
                Ok(handle_diff_view(key))
            }
        }
    }
}

fn handle_file_list(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char('q') => Some(Message::Quit),
        KeyCode::Char('j') | KeyCode::Down => Some(Message::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::MoveUp),
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => Some(Message::SelectFile),
        KeyCode::Tab => Some(Message::SwitchPanel),
        KeyCode::Char('s') => Some(Message::StageFile),
        KeyCode::Char('u') => Some(Message::UnstageFile),
        KeyCode::Char('d') => Some(Message::DiscardChanges),
        KeyCode::Char('r') => Some(Message::Refresh),
        _ => None,
    }
}

fn handle_diff_view(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char('q') => Some(Message::Quit),
        KeyCode::Char('j') | KeyCode::Down => Some(Message::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::MoveUp),
        KeyCode::Tab | KeyCode::Char('h') | KeyCode::Left => Some(Message::SwitchPanel),
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => Some(Message::EnterLineMode),
        KeyCode::Char('s') => Some(Message::StageHunk),
        KeyCode::Char('S') => Some(Message::StageFile),
        KeyCode::Char('u') => Some(Message::UnstageFile),
        KeyCode::Char('r') => Some(Message::Refresh),
        _ => None,
    }
}

fn handle_line_mode(key: KeyEvent) -> Option<Message> {
    match key.code {
        KeyCode::Char('q') => Some(Message::Quit),
        KeyCode::Char('j') | KeyCode::Down => Some(Message::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::MoveUp),
        KeyCode::Char(' ') => Some(Message::ToggleLine),
        KeyCode::Char('a') => Some(Message::SelectAllLines),
        KeyCode::Char('s') => Some(Message::StageLines),
        KeyCode::Char('S') => Some(Message::StageFile),
        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => Some(Message::ExitLineMode),
        KeyCode::Char('r') => Some(Message::Refresh),
        _ => None,
    }
}
