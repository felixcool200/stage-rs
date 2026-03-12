use crate::app::{App, DiffViewMode, Panel};
use crate::keymap::KeymapName;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render_header(app: &App, frame: &mut Frame, area: Rect) {
    let branch = &app.branch_name;
    let file_count = app.file_entries.len();

    let in_line_mode = app
        .diff_state
        .as_ref()
        .map(|ds| ds.view_mode == DiffViewMode::LineNav)
        .unwrap_or(false);

    let keybinds = match (app.active_panel, in_line_mode, app.keymap) {
        (Panel::FileList, _, _) => {
            "  [s]tage [u]nstage [d]iscard [c]ommit [C]amend [z]undo [g]log [q]uit "
        }
        (Panel::DiffView, false, _) => {
            match app.keymap {
                KeymapName::Vim => "  [s]tage hunk [S]file Enter:lines [c]ommit [g]log [q]uit ",
                KeymapName::Helix => "  [s]tage hunk [S]file [v]:lines [c]ommit [g]log [q]uit ",
            }
        }
        (Panel::DiffView, true, KeymapName::Vim) => {
            "  Space:toggle [a]ll [s]tage [S]file Esc:back [q]uit "
        }
        (Panel::DiffView, true, KeymapName::Helix) => {
            "  [x]:toggle [X]:all [s]tage [S]file Esc:back [q]uit "
        }
    };

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
    let in_line_mode = app
        .diff_state
        .as_ref()
        .map(|ds| ds.view_mode == DiffViewMode::LineNav)
        .unwrap_or(false);

    let panel_name = match (app.active_panel, in_line_mode) {
        (Panel::FileList, _) => "Files",
        (Panel::DiffView, false) => "Diff",
        (Panel::DiffView, true) => "Lines",
    };
    let mut spans = vec![
        Span::styled(
            format!(" {panel_name} "),
            Style::default().fg(Color::Black).bg(Color::Blue),
        ),
        Span::styled(
            format!(" {} ", app.keymap.label()),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta),
        ),
    ];

    if let Some(ds) = &app.diff_state {
        spans.push(Span::styled(
            format!(" {} ", ds.file_path),
            Style::default().fg(Color::White),
        ));
        match ds.view_mode {
            DiffViewMode::HunkNav if !ds.hunks.is_empty() => {
                spans.push(Span::styled(
                    format!("hunk {}/{} ", ds.current_hunk + 1, ds.hunks.len()),
                    Style::default().fg(Color::Cyan),
                ));
            }
            DiffViewMode::LineNav => {
                let sel = ds.selected_lines.len();
                let total = ds.hunk_changed_rows.len();
                spans.push(Span::styled(
                    format!("{sel}/{total} lines selected "),
                    Style::default().fg(Color::Magenta),
                ));
            }
            _ => {}
        }
    }

    if let Some(msg) = &app.status_message {
        spans.push(Span::styled(
            format!(" | {msg}"),
            Style::default().fg(Color::Yellow),
        ));
    }

    let nav_hint = match (app.active_panel, in_line_mode, app.keymap) {
        (Panel::FileList, _, _) => " | j/k:navigate Enter:select Tab:diff Ctrl+K:keymap ",
        (Panel::DiffView, false, KeymapName::Vim) => {
            " | j/k:scroll J/K:hunks Enter:lines Tab:files Ctrl+K:keymap "
        }
        (Panel::DiffView, false, KeymapName::Helix) => {
            " | j/k:scroll J/K:hunks v:lines Tab:files Ctrl+K:keymap "
        }
        (Panel::DiffView, true, KeymapName::Vim) => {
            " | j/k:lines Space:toggle s:stage Esc:back "
        }
        (Panel::DiffView, true, KeymapName::Helix) => {
            " | j/k:lines x:toggle s:stage Esc:back "
        }
    };
    spans.push(Span::styled(
        nav_hint,
        Style::default().fg(Color::DarkGray),
    ));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
