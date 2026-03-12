use crate::app::{App, DiffViewMode, Message, Panel};
use crate::keymap::{self, InputContext};
use color_eyre::Result;
use crossterm::event::{self, Event};
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
