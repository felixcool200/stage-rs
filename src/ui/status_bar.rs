use crate::app::{App, Panel};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render_header(app: &App, frame: &mut Frame, area: Rect) {
    let branch = &app.branch_name;
    let file_count = app.file_entries.len();

    let keybinds = match app.active_panel {
        Panel::FileList => "  [s]tage file [u]nstage [d]iscard [r]efresh [q]uit ",
        Panel::DiffView => "  [s]tage hunk [S]tage file [u]nstage [r]efresh [q]uit ",
    };

    let line = Line::from(vec![
        Span::styled(
            format!("  {branch} "),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {file_count} changes "),
            Style::default().fg(Color::White).bg(Color::DarkGray),
        ),
        Span::styled(keybinds, Style::default().fg(Color::DarkGray)),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}

pub fn render_footer(app: &App, frame: &mut Frame, area: Rect) {
    let panel_name = match app.active_panel {
        Panel::FileList => "Files",
        Panel::DiffView => "Diff",
    };
    let mut spans = vec![Span::styled(
        format!(" {panel_name} "),
        Style::default().fg(Color::Black).bg(Color::Blue),
    )];

    if let Some(ds) = &app.diff_state {
        spans.push(Span::styled(
            format!(" {} ", ds.file_path),
            Style::default().fg(Color::White),
        ));
        if !ds.hunks.is_empty() {
            spans.push(Span::styled(
                format!("hunk {}/{} ", ds.current_hunk + 1, ds.hunks.len()),
                Style::default().fg(Color::Cyan),
            ));
        }
    }

    if let Some(msg) = &app.status_message {
        spans.push(Span::styled(
            format!(" | {msg}"),
            Style::default().fg(Color::Yellow),
        ));
    }

    let nav_hint = match app.active_panel {
        Panel::FileList => " | j/k:navigate Enter:select Tab:diff ",
        Panel::DiffView => " | j/k:prev/next hunk Tab:files ",
    };
    spans.push(Span::styled(
        nav_hint,
        Style::default().fg(Color::DarkGray),
    ));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
