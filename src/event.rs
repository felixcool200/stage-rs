use crate::app::{App, AppMode, Message, Panel};
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

pub fn poll_event(app: &App) -> Result<Option<Message>> {
    if !event::poll(Duration::from_millis(250))? {
        return Ok(None);
    }

    let Event::Key(key) = event::read()? else {
        return Ok(None);
    };

    // Ignore key release events (crossterm on some platforms sends both press and release)
    if key.kind != crossterm::event::KeyEventKind::Press {
        return Ok(None);
    }

    Ok(handle_normal_mode(app, key))
}

fn handle_normal_mode(app: &App, key: KeyEvent) -> Option<Message> {
    // Global keybindings
    match (key.modifiers, key.code) {
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => return Some(Message::Quit),
        _ => {}
    }

    match key.code {
        KeyCode::Char('q') => Some(Message::Quit),
        KeyCode::Char('j') | KeyCode::Down => Some(Message::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::MoveUp),
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
            if app.active_panel == Panel::FileList {
                Some(Message::SelectFile)
            } else {
                None
            }
        }
        KeyCode::Tab => Some(Message::SwitchPanel),
        KeyCode::Char('s') => Some(Message::StageFile),
        KeyCode::Char('u') => Some(Message::UnstageFile),
        KeyCode::Char('d') => Some(Message::DiscardChanges),
        KeyCode::Char('r') => Some(Message::Refresh),
        KeyCode::Char('h') | KeyCode::Left => {
            if app.active_panel == Panel::DiffView {
                Some(Message::SwitchPanel)
            } else {
                None
            }
        }
        _ => None,
    }
}
