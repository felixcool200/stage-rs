mod app;
mod event;
mod git;
mod keymap;
mod syntax;
mod ui;

use app::{App, Overlay};
use color_eyre::Result;
use std::time::Duration;

fn main() -> Result<()> {
    color_eyre::install()?;

    let mut path = ".".to_string();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                println!("gitview-rs - TUI git client with side-by-side diff view");
                println!();
                println!("USAGE: gitview-rs [PATH]");
                println!();
                println!("ARGS:");
                println!("  [PATH]  Path to git repository (default: current directory)");
                println!();
                println!("OPTIONS:");
                println!("  -h, --help  Show this help");
                return Ok(());
            }
            other => {
                path = other.to_string();
            }
        }
        i += 1;
    }

    let mut terminal = ratatui::init();
    let mut app = App::new(&path)?;
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::render(app, frame))?;

        let branch_creating = matches!(app.overlay, Overlay::BranchList { creating: Some(_), .. });
        if app.conflict_state.is_some() {
            if let Some(msg) = poll_conflict_mode(app)? {
                app.update(msg)?;
            }
        } else if app.edit_state.is_some() {
            if let Some(msg) = poll_edit_mode(app)? {
                app.update(msg)?;
            }
        } else if matches!(app.overlay, Overlay::CommitInput { .. }) {
            if let Some(msg) = poll_with_text_input(app)? {
                app.update(msg)?;
            }
        } else if branch_creating {
            if let Some(msg) = poll_branch_create(app)? {
                app.update(msg)?;
            }
        } else if app.file_filter.is_some() {
            if let Some(msg) = poll_with_filter(app)? {
                app.update(msg)?;
            }
        } else if let Some(msg) = event::poll_event(app)? {
            app.update(msg)?;
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn poll_conflict_mode(app: &mut App) -> Result<Option<app::Message>> {
    if !crossterm::event::poll(Duration::from_millis(250))? {
        return Ok(None);
    }
    let crossterm::event::Event::Key(key) = crossterm::event::read()? else {
        return Ok(None);
    };
    if key.kind != crossterm::event::KeyEventKind::Press {
        return Ok(None);
    }
    use crossterm::event::{KeyCode, KeyModifiers};
    Ok(match (key.modifiers, key.code) {
        (_, KeyCode::Esc) => {
            app.conflict_state = None;
            app.status_message = Some("Conflict resolver closed".into());
            None
        }
        (_, KeyCode::Char('o')) => Some(app::Message::ConflictPickOurs),
        (_, KeyCode::Char('t')) => Some(app::Message::ConflictPickTheirs),
        (_, KeyCode::Char('b')) => Some(app::Message::ConflictPickBoth),
        (_, KeyCode::Down) => Some(app::Message::ConflictNextSection),
        (_, KeyCode::Up) => Some(app::Message::ConflictPrevSection),
        (_, KeyCode::Char('s')) => Some(app::Message::ConflictSave),
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Some(app::Message::Quit),
        _ => None,
    })
}

fn poll_edit_mode(app: &mut App) -> Result<Option<app::Message>> {
    if !crossterm::event::poll(Duration::from_millis(250))? {
        return Ok(None);
    }
    let event = crossterm::event::read()?;

    if let crossterm::event::Event::Key(key) = &event {
        if key.kind != crossterm::event::KeyEventKind::Press {
            return Ok(None);
        }
        use crossterm::event::{KeyCode, KeyModifiers};
        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => {
                return Ok(Some(app::Message::SaveEdit));
            }
            (_, KeyCode::Esc) => {
                return Ok(Some(app::Message::ExitEditMode));
            }
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                return Ok(Some(app::Message::Quit));
            }
            _ => {}
        }
    }

    // Forward event to textarea
    if let Some(edit) = &mut app.edit_state {
        edit.textarea.input(event);
    }
    Ok(None)
}

fn poll_branch_create(app: &mut App) -> Result<Option<app::Message>> {
    if !crossterm::event::poll(Duration::from_millis(250))? {
        return Ok(None);
    }
    let crossterm::event::Event::Key(key) = crossterm::event::read()? else {
        return Ok(None);
    };
    if key.kind != crossterm::event::KeyEventKind::Press {
        return Ok(None);
    }
    use crossterm::event::{KeyCode, KeyModifiers};
    match (key.modifiers, key.code) {
        (_, KeyCode::Esc) => {
            if let Overlay::BranchList { ref mut creating, .. } = app.overlay {
                *creating = None;
            }
        }
        (_, KeyCode::Enter) => {
            return Ok(Some(app::Message::ConfirmCreateBranch));
        }
        (_, KeyCode::Backspace) => {
            if let Overlay::BranchList { creating: Some(ref mut name), .. } = app.overlay {
                name.pop();
            }
        }
        (_, KeyCode::Char(ch)) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            if let Overlay::BranchList { creating: Some(ref mut name), .. } = app.overlay {
                name.push(ch);
            }
        }
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            return Ok(Some(app::Message::Quit));
        }
        _ => {}
    }
    Ok(None)
}

fn poll_with_filter(app: &mut App) -> Result<Option<app::Message>> {
    if !crossterm::event::poll(Duration::from_millis(250))? {
        return Ok(None);
    }

    let crossterm::event::Event::Key(key) = crossterm::event::read()? else {
        return Ok(None);
    };

    if key.kind != crossterm::event::KeyEventKind::Press {
        return Ok(None);
    }

    use crossterm::event::{KeyCode, KeyModifiers};
    match (key.modifiers, key.code) {
        (_, KeyCode::Esc) => {
            return Ok(Some(app::Message::ClearFilter));
        }
        (_, KeyCode::Enter) => {
            // Confirm filter and go back to normal mode — keep filter text active
            // Select first matching entry
            if let Some((idx, _)) = app.filtered_entries().first() {
                app.selected_index = *idx;
            }
            app.file_filter = None;
            return Ok(None);
        }
        (_, KeyCode::Backspace) => {
            if let Some(ref mut f) = app.file_filter {
                f.pop();
            }
        }
        (_, KeyCode::Char(ch)) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            if let Some(ref mut f) = app.file_filter {
                f.push(ch);
            }
        }
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            return Ok(Some(app::Message::Quit));
        }
        _ => {}
    }
    // Update selected index to first matching entry
    if let Some((idx, _)) = app.filtered_entries().first() {
        app.selected_index = *idx;
    }
    Ok(None)
}

fn poll_with_text_input(app: &mut App) -> Result<Option<app::Message>> {
    if !crossterm::event::poll(Duration::from_millis(250))? {
        return Ok(None);
    }

    let crossterm::event::Event::Key(key) = crossterm::event::read()? else {
        return Ok(None);
    };

    if key.kind != crossterm::event::KeyEventKind::Press {
        return Ok(None);
    }

    // First try the overlay handler for control keys
    if let Some(msg) = event::poll_event_overlay_only(key) {
        return Ok(Some(msg));
    }

    // Otherwise, apply as text input
    if let Overlay::CommitInput { ref mut input, .. } = app.overlay {
        event::apply_text_input_key(input, key.modifiers, key.code);
    }

    Ok(None)
}
