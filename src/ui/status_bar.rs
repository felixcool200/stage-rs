use crate::app::{App, DiffViewMode, Panel};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render_header(app: &App, frame: &mut Frame, area: Rect) {
    let branch = &app.branch_name;
    let file_count = app.file_entries.len();

    let (ahead, behind) = app.ahead_behind;
    let mut ab_parts = Vec::new();
    if ahead > 0 {
        ab_parts.push(format!("↑{ahead}"));
    }
    if behind > 0 {
        ab_parts.push(format!("↓{behind}"));
    }
    let ab_str = if ab_parts.is_empty() {
        String::new()
    } else {
        format!(" {}", ab_parts.join(" "))
    };

    let line = Line::from(vec![
        Span::styled(
            format!("  {branch}{ab_str} "),
            Style::default()
                .fg(app.theme.black)
                .bg(app.theme.cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {file_count} changes "),
            Style::default().fg(app.theme.fg).bg(app.theme.fg_dim),
        ),
        Span::styled(
            "  Space: commands  q: quit ",
            Style::default().fg(app.theme.fg_dim),
        ),
    ]);

    frame.render_widget(Paragraph::new(line).style(Style::default().bg(app.theme.bg)), area);
}

pub fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let in_line_mode = app
        .diff_state
        .as_ref()
        .map(|ds| ds.view_mode == DiffViewMode::LineNav)
        .unwrap_or(false);

    let (mode_label, mode_bg) = match (app.active_panel, in_line_mode) {
        (Panel::FileList, _) => ("FILES", app.theme.blue),
        (Panel::DiffView, false) => ("NORMAL", app.theme.blue),
        (Panel::DiffView, true) => ("SELECT", app.theme.magenta),
    };

    let mut spans = vec![
        Span::styled(
            format!(" {mode_label} "),
            Style::default()
                .fg(app.theme.black)
                .bg(mode_bg)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    if let Some(ds) = &app.diff_state {
        spans.push(Span::styled(
            format!(" {} ", ds.file_path),
            Style::default().fg(app.theme.fg),
        ));
        match ds.view_mode {
            DiffViewMode::HunkNav if !ds.hunks.is_empty() => {
                spans.push(Span::styled(
                    format!("hunk {}/{} ", ds.current_hunk + 1, ds.hunks.len()),
                    Style::default().fg(app.theme.cyan),
                ));
            }
            DiffViewMode::LineNav => {
                let sel = ds.selected_lines.len();
                let total = ds.hunk_changed_rows.len();
                let verb = if ds.is_staged { "to unstage" } else { "selected" };
                spans.push(Span::styled(
                    format!("{sel}/{total} lines {verb} "),
                    Style::default().fg(app.theme.magenta),
                ));
            }
            _ => {}
        }
    }

    if let Some(msg) = &app.status_message {
        spans.push(Span::styled(
            format!(" | {msg}"),
            Style::default().fg(app.theme.yellow),
        ));
    }

    let nav_hint = match (app.active_panel, in_line_mode) {
        (Panel::FileList, _) => " | ↑/↓:navigate Enter:select Tab:diff Space:commands ",
        (Panel::DiffView, false) => {
            " | ↑/↓:scroll Shift+↑/↓:hunks Enter:lines Tab:files Space:commands "
        }
        (Panel::DiffView, true) => {
            " | ↑/↓:lines Enter:toggle Esc:back Space:commands "
        }
    };
    spans.push(Span::styled(
        nav_hint,
        Style::default().fg(app.theme.fg_dim),
    ));

    frame.render_widget(Paragraph::new(Line::from(spans)).style(Style::default().bg(app.theme.bg)), area);
}
