mod app;
mod event;
mod git;
mod keymap;
mod ui;

use app::{App, Overlay};
use color_eyre::Result;
use keymap::KeymapName;
use std::time::Duration;

fn main() -> Result<()> {
    color_eyre::install()?;

    let mut path = ".".to_string();
    let mut keymap = KeymapName::Vim;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--keymap" | "-k" => {
                i += 1;
                if let Some(name) = args.get(i) {
                    keymap = KeymapName::from_str(name).unwrap_or_else(|| {
                        eprintln!("Unknown keymap '{}', using vim. Options: vim, helix", name);
                        KeymapName::Vim
                    });
                }
            }
            "--help" | "-h" => {
                println!("gitview-rs - TUI git client with side-by-side diff view");
                println!();
                println!("USAGE: gitview-rs [OPTIONS] [PATH]");
                println!();
                println!("ARGS:");
                println!("  [PATH]  Path to git repository (default: current directory)");
                println!();
                println!("OPTIONS:");
                println!("  -k, --keymap <NAME>  Keymap to use: vim (default), helix");
                println!("  -h, --help           Show this help");
                println!();
                println!("RUNTIME:");
                println!("  Ctrl+K  Cycle between keymaps");
                return Ok(());
            }
            other => {
                path = other.to_string();
            }
        }
        i += 1;
    }

    let mut terminal = ratatui::init();
    let mut app = App::new(&path, keymap)?;
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::render(app, frame))?;

        if matches!(app.overlay, Overlay::CommitInput { .. }) {
            if let Some(msg) = poll_with_text_input(app)? {
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
