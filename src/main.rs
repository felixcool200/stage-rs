mod app;
mod clipboard;
mod conflict;
mod event;
mod git;
mod keymap;
mod syntax;
mod text_input;
mod theme;
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
                println!("stage-rs - TUI git client with side-by-side diff view");
                println!();
                println!("USAGE: stage-rs [PATH]");
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

    // Validate path before initializing the terminal
    let canon = std::path::Path::new(&path);
    if !canon.exists() {
        eprintln!("error: path does not exist: {path}");
        std::process::exit(1);
    }
    if !canon.is_dir() {
        eprintln!("error: not a directory: {path}");
        std::process::exit(1);
    }

    let mut app = match App::new(&path) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("error: not a git repository (or any parent): {path}\n{e}");
            std::process::exit(1);
        }
    };

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        // Update viewport height for scroll centering
        let term_height = terminal.size()?.height as usize;
        if let Some(ds) = &mut app.diff_state {
            // body height = term_height - header(1) - footer(1), each diff panel has 2 border rows
            ds.viewport_height = term_height.saturating_sub(4);
        }
        if let Overlay::CommitDetail {
            ref mut viewport_height,
            ..
        } = app.overlay
        {
            // popup is 80% of terminal height, minus 2 border rows
            *viewport_height = (term_height * 80 / 100).saturating_sub(2);
        }
        terminal.draw(|frame| ui::render(app, frame))?;

        let branch_creating = matches!(
            app.overlay,
            Overlay::BranchList {
                creating: Some(_),
                ..
            }
        );
        if app.pending_editor.is_some() {
            spawn_editor(terminal, app)?;
        } else if app.pending_terminal_cmd.is_some() {
            run_terminal_cmd(terminal, app)?;
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
        } else if app.which_key.is_some() {
            if let Some(msg) = poll_which_key(app)? {
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

fn run_with_suspended_tui(
    terminal: &mut ratatui::DefaultTerminal,
    command: &mut std::process::Command,
) -> Result<std::process::ExitStatus> {
    ratatui::restore();
    let status = command
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();
    *terminal = ratatui::init();
    status.map_err(Into::into)
}

fn spawn_editor(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    let req = app.pending_editor.take().unwrap();
    let workdir = app.repo.workdir().to_path_buf();
    let full_path = workdir.join(&req.file_path);

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".into());

    let mut cmd = std::process::Command::new("sh");
    cmd.arg("-c").arg(format!(
        "{} +{} '{}'",
        editor,
        req.line_number,
        full_path.display()
    ));

    match run_with_suspended_tui(terminal, &mut cmd) {
        Ok(s) if s.success() => {
            app.status_message = Some(format!("Editor closed: {}", req.file_path));
        }
        Ok(s) => {
            app.status_message = Some(format!(
                "Editor exited with code {}",
                s.code().unwrap_or(-1)
            ));
        }
        Err(e) => {
            app.status_message = Some(format!("Failed to launch '{}': {}", editor, e));
        }
    }

    app.update(app::Message::Refresh)?;
    Ok(())
}

fn run_terminal_cmd(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    let cmd = app.pending_terminal_cmd.take().unwrap();

    let mut command = std::process::Command::new(&cmd.program);
    command.args(&cmd.args).current_dir(&cmd.workdir);

    match run_with_suspended_tui(terminal, &mut command) {
        Ok(s) if s.success() => {
            app.status_message = Some(cmd.success_msg);
        }
        Ok(_) => {
            app.status_message = Some(format!("{} failed", cmd.program));
        }
        Err(e) => {
            app.status_message = Some(format!("Failed to run {}: {e}", cmd.program));
        }
    }

    app.update(app::Message::Refresh)?;
    Ok(())
}

fn poll_key() -> Result<Option<crossterm::event::KeyEvent>> {
    if !crossterm::event::poll(Duration::from_millis(250))? {
        return Ok(None);
    }
    let crossterm::event::Event::Key(key) = crossterm::event::read()? else {
        return Ok(None);
    };
    if key.kind != crossterm::event::KeyEventKind::Press {
        return Ok(None);
    }
    Ok(Some(key))
}

fn poll_which_key(app: &mut App) -> Result<Option<app::Message>> {
    let Some(key) = poll_key()? else {
        return Ok(None);
    };

    let entries = app.which_key.take().unwrap();

    if let crossterm::event::KeyCode::Char(ch) = key.code {
        if let Some(entry) = entries.iter().find(|e| e.key == ch) {
            return Ok(Some(entry.message.clone()));
        }
    }

    // Any non-matching key (including Esc) closes the popup
    Ok(None)
}

fn poll_branch_create(app: &mut App) -> Result<Option<app::Message>> {
    let Some(key) = poll_key()? else {
        return Ok(None);
    };

    use crossterm::event::{KeyCode, KeyModifiers};
    match (key.modifiers, key.code) {
        (_, KeyCode::Esc) => {
            if let Overlay::BranchList {
                ref mut creating, ..
            } = app.overlay
            {
                *creating = None;
            }
        }
        (_, KeyCode::Enter) => {
            return Ok(Some(app::Message::ConfirmCreateBranch));
        }
        (_, KeyCode::Backspace) => {
            if let Overlay::BranchList {
                creating: Some(ref mut name),
                ..
            } = app.overlay
            {
                name.pop();
            }
        }
        (_, KeyCode::Char(ch))
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
        {
            if let Overlay::BranchList {
                creating: Some(ref mut name),
                ..
            } = app.overlay
            {
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
    let Some(key) = poll_key()? else {
        return Ok(None);
    };

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
                app.header_selected = false;
            }
            app.file_filter = None;
            return Ok(None);
        }
        (_, KeyCode::Backspace) => {
            if let Some(ref mut f) = app.file_filter {
                f.pop();
            }
        }
        (_, KeyCode::Char(ch))
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
        {
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
        app.header_selected = false;
    }
    Ok(None)
}

fn poll_with_text_input(app: &mut App) -> Result<Option<app::Message>> {
    let Some(key) = poll_key()? else {
        return Ok(None);
    };

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
