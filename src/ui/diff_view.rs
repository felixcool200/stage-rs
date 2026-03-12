use crate::app::{App, ConflictResolution, ConflictState, DiffState, DiffViewMode, Panel};
use crate::git::DiffLineKind;
use crate::syntax;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render_left(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" Index / HEAD ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let Some(ds) = &app.diff_state else {
        let placeholder = Paragraph::new("Select a file to view diff").block(block);
        frame.render_widget(placeholder, area);
        return;
    };

    let visible_height = area.height.saturating_sub(2) as usize;
    let is_focused = app.active_panel == Panel::DiffView;

    let blame_data = if app.show_blame { app.blame_data.as_deref() } else { None };

    let lines: Vec<Line> = ds
        .left_lines
        .iter()
        .enumerate()
        .skip(ds.scroll)
        .take(visible_height)
        .map(|(i, dl)| {
            let highlight = get_line_highlight(ds, i, is_focused);
            let line_num = format!("{:>4} ", i + 1);
            let (num_style, text_style) = line_styles(&dl.kind, &highlight);

            let mut spans = Vec::new();

            // Show blame annotation if enabled
            if let Some(blame) = blame_data {
                if let Some(bl) = blame.get(i) {
                    let annotation = format!("{} {:>8} ", bl.hash, truncate_str(&bl.author, 8));
                    spans.push(Span::styled(annotation, Style::default().fg(Color::DarkGray)));
                } else {
                    spans.push(Span::styled("                  ", Style::default()));
                }
            }

            // Show selection indicator in line mode
            if ds.view_mode == DiffViewMode::LineNav && dl.hunk_index.is_some() {
                let is_selected = ds.selected_lines.contains(&i);
                let is_cursor = i == ds.cursor_line;
                let marker = if is_selected && is_cursor {
                    Span::styled(
                        "[x]",
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    )
                } else if is_selected {
                    Span::styled(
                        "[x]",
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    )
                } else if is_cursor {
                    Span::styled(
                        "[ ]",
                        Style::default().fg(Color::Yellow),
                    )
                } else {
                    Span::styled("   ", Style::default())
                };
                spans.push(marker);
            }
            spans.push(Span::styled(line_num, num_style));
            push_highlighted_content(app, &dl.content, &dl.kind, text_style, &mut spans);
            Line::from(spans)
        })
        .collect();

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

pub fn render_right(app: &App, frame: &mut Frame, area: Rect) {
    // If in conflict resolver mode
    if let Some(cs) = &app.conflict_state {
        render_conflict(frame, cs, area);
        return;
    }

    // If in insert (edit) mode, render the textarea instead
    if let Some(edit) = &app.edit_state {
        let block = Block::default()
            .title(format!(" {} [Ctrl+S save, Esc normal] ", edit.file_path))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(&edit.textarea, inner);
        return;
    }

    render_right_diff(app, frame, area);
}

fn render_right_diff(app: &App, frame: &mut Frame, area: Rect) {
    let is_focused = app.active_panel == Panel::DiffView;
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let title = match &app.diff_state {
        Some(ds) => {
            let mode_info = match ds.view_mode {
                DiffViewMode::HunkNav if !ds.hunks.is_empty() => {
                    format!(" [hunk {}/{}]", ds.current_hunk + 1, ds.hunks.len())
                }
                DiffViewMode::LineNav => {
                    let sel = ds.selected_lines.len();
                    let total = ds.hunk_changed_rows.len();
                    format!(" [line mode: {sel}/{total} selected]")
                }
                _ => String::new(),
            };
            format!(" Working Tree: {}{} ", ds.file_path, mode_info)
        }
        None => " Working Tree ".into(),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let Some(ds) = &app.diff_state else {
        frame.render_widget(Paragraph::new("").block(block), area);
        return;
    };

    let visible_height = area.height.saturating_sub(2) as usize;

    let lines: Vec<Line> = ds
        .right_lines
        .iter()
        .enumerate()
        .skip(ds.scroll)
        .take(visible_height)
        .map(|(i, dl)| {
            let highlight = get_line_highlight(ds, i, is_focused);

            // Show hunk header at the start of the current hunk (only in hunk mode)
            if ds.view_mode == DiffViewMode::HunkNav
                && highlight == LineHighlight::CurrentHunk
                && ds.hunks.get(ds.current_hunk).map(|h| h.display_start) == Some(i)
            {
                return hunk_header_line(ds, i);
            }

            let line_num = format!("{:>4} ", i + 1);
            let (num_style, text_style) = line_styles(&dl.kind, &highlight);

            let mut spans = Vec::new();
            // Show selection indicator in line mode
            if ds.view_mode == DiffViewMode::LineNav && dl.hunk_index.is_some() {
                let is_selected = ds.selected_lines.contains(&i);
                let is_cursor = i == ds.cursor_line;
                let marker = if is_selected && is_cursor {
                    Span::styled(
                        "[x]",
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    )
                } else if is_selected {
                    Span::styled(
                        "[x]",
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    )
                } else if is_cursor {
                    Span::styled(
                        "[ ]",
                        Style::default().fg(Color::Yellow),
                    )
                } else {
                    Span::styled("   ", Style::default())
                };
                spans.push(marker);
            }
            spans.push(Span::styled(line_num, num_style));
            push_highlighted_content(app, &dl.content, &dl.kind, text_style, &mut spans);
            Line::from(spans)
        })
        .collect();

    // Split area: main diff content + 1-col overview bar on the right
    let [main_area, bar_area] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, main_area);

    render_change_overview(frame, ds, bar_area);
}

/// Render a 1-column-wide change overview bar showing where modifications are in the file.
/// Green = added, Red = removed, DarkGray = equal, Cyan = viewport indicator.
fn render_change_overview(frame: &mut Frame, ds: &DiffState, area: Rect) {
    let bar_height = area.height as usize;
    if bar_height == 0 {
        return;
    }

    let total_lines = ds.right_lines.len().max(1);
    let visible_height = bar_height; // each row of the bar maps to a portion of the file

    // Build the bar: map each bar row to a file region
    let mut bar_lines: Vec<Line> = Vec::with_capacity(bar_height);
    for row in 0..bar_height {
        let file_start = row * total_lines / bar_height;
        let file_end = ((row + 1) * total_lines / bar_height).max(file_start + 1);

        // Check what kinds of lines are in this region
        let mut has_added = false;
        let mut has_removed = false;
        for i in file_start..file_end.min(ds.right_lines.len()) {
            match ds.right_lines[i].kind {
                DiffLineKind::Added => has_added = true,
                DiffLineKind::Removed | DiffLineKind::Spacer => {
                    if ds.right_lines[i].hunk_index.is_some() {
                        has_removed = true;
                    }
                }
                _ => {}
            }
        }

        // Determine if this row overlaps the current viewport
        let viewport_start = ds.scroll;
        let viewport_end = ds.scroll + visible_height;
        let in_viewport = file_start < viewport_end && file_end > viewport_start;

        let (ch, color) = if has_added && has_removed {
            ("┃", Color::Yellow)
        } else if has_added {
            ("┃", Color::Green)
        } else if has_removed {
            ("┃", Color::Red)
        } else if in_viewport {
            ("│", Color::DarkGray)
        } else {
            (" ", Color::DarkGray)
        };

        // Brighten viewport indicator
        let bg = if in_viewport {
            Color::Rgb(30, 30, 40)
        } else {
            Color::Reset
        };

        bar_lines.push(Line::from(Span::styled(ch, Style::default().fg(color).bg(bg))));
    }

    let paragraph = Paragraph::new(bar_lines);
    frame.render_widget(paragraph, area);
}

#[derive(PartialEq, Eq)]
enum LineHighlight {
    None,
    CurrentHunk,
    CursorLine,
    SelectedLine,
}

fn get_line_highlight(
    ds: &crate::app::DiffState,
    display_row: usize,
    is_focused: bool,
) -> LineHighlight {
    if !is_focused {
        return LineHighlight::None;
    }

    match ds.view_mode {
        DiffViewMode::LineNav => {
            if ds.selected_lines.contains(&display_row) {
                LineHighlight::SelectedLine
            } else if display_row == ds.cursor_line {
                LineHighlight::CursorLine
            } else {
                LineHighlight::None
            }
        }
        DiffViewMode::HunkNav => {
            if let Some(hunk) = ds.hunks.get(ds.current_hunk) {
                if display_row >= hunk.display_start
                    && display_row < hunk.display_end
                    && ds.left_lines[display_row].hunk_index.is_some()
                {
                    LineHighlight::CurrentHunk
                } else {
                    LineHighlight::None
                }
            } else {
                LineHighlight::None
            }
        }
    }
}

fn hunk_header_line(ds: &crate::app::DiffState, display_row: usize) -> Line<'static> {
    let hunk = &ds.hunks[ds.current_hunk];
    let dl = &ds.right_lines[display_row];
    let line_num = format!("{:>4} ", display_row + 1);

    let (num_style, text_style) = line_styles(&dl.kind, &LineHighlight::CurrentHunk);

    Line::from(vec![
        Span::styled(
            format!("{} ", hunk.header),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(line_num, num_style),
        Span::styled(dl.content.clone(), text_style),
    ])
}

fn render_conflict(frame: &mut Frame, cs: &ConflictState, area: Rect) {
    let block = Block::default()
        .title(format!(
            " Conflict: {} [{}/{}] o=ours t=theirs b=both s=save ",
            cs.file_path,
            cs.current_section + 1,
            cs.sections.len()
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let section = &cs.sections[cs.current_section];
    let resolution_label = match section.resolution {
        ConflictResolution::Unresolved => "UNRESOLVED",
        ConflictResolution::Ours => "OURS",
        ConflictResolution::Theirs => "THEIRS",
        ConflictResolution::Both => "BOTH",
    };
    let res_color = match section.resolution {
        ConflictResolution::Unresolved => Color::Red,
        ConflictResolution::Ours => Color::Cyan,
        ConflictResolution::Theirs => Color::Magenta,
        ConflictResolution::Both => Color::Green,
    };

    let mut lines = vec![
        Line::from(Span::styled(
            format!("  Resolution: {resolution_label}"),
            Style::default().fg(res_color).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  <<<<<<< OURS",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
    ];
    for l in &section.ours {
        lines.push(Line::from(Span::styled(
            format!("  {l}"),
            Style::default().fg(Color::Cyan),
        )));
    }
    lines.push(Line::from(Span::styled(
        "  =======",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        "  >>>>>>> THEIRS",
        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
    )));
    for l in &section.theirs {
        lines.push(Line::from(Span::styled(
            format!("  {l}"),
            Style::default().fg(Color::Magenta),
        )));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Push syntax-highlighted spans for a line's content, falling back to a single styled span.
fn push_highlighted_content<'a>(
    app: &App,
    content: &'a str,
    kind: &DiffLineKind,
    fallback_style: Style,
    spans: &mut Vec<Span<'a>>,
) {
    // Don't highlight spacer lines or empty content
    if *kind == DiffLineKind::Spacer || content.is_empty() {
        spans.push(Span::styled(content, fallback_style));
        return;
    }

    // Try syntax highlighting based on file extension
    if let Some(ds) = &app.diff_state {
        if let Some(ext) = syntax::file_extension(&ds.file_path) {
            let bg = fallback_style.bg.unwrap_or(Color::Reset);
            if let Some(highlighted) = app.highlighter.highlight_line(content, ext, bg) {
                spans.extend(highlighted);
                return;
            }
        }
    }

    spans.push(Span::styled(content, fallback_style));
}

fn truncate_str(s: &str, max_len: usize) -> String {
    let chars: Vec<char> = s.chars().take(max_len).collect();
    let truncated: String = chars.iter().collect();
    if chars.len() < max_len {
        format!("{truncated}{}", " ".repeat(max_len - chars.len()))
    } else {
        truncated
    }
}

fn line_styles(kind: &DiffLineKind, highlight: &LineHighlight) -> (Style, Style) {
    let (bg, change_bg_boost) = match highlight {
        LineHighlight::CursorLine => (Color::Rgb(50, 50, 20), true),
        LineHighlight::SelectedLine => (Color::Rgb(20, 50, 20), true),
        LineHighlight::CurrentHunk => (Color::Rgb(30, 30, 50), true),
        LineHighlight::None => (Color::Reset, false),
    };

    match kind {
        DiffLineKind::Equal => (
            Style::default().fg(Color::DarkGray).bg(bg),
            Style::default().fg(Color::White).bg(bg),
        ),
        DiffLineKind::Removed => (
            Style::default().fg(Color::DarkGray).bg(bg),
            Style::default().fg(Color::Red).bg(if change_bg_boost {
                Color::Rgb(60, 20, 20)
            } else {
                Color::Reset
            }),
        ),
        DiffLineKind::Added => (
            Style::default().fg(Color::DarkGray).bg(bg),
            Style::default().fg(Color::Green).bg(if change_bg_boost {
                Color::Rgb(20, 60, 20)
            } else {
                Color::Reset
            }),
        ),
        DiffLineKind::Spacer => (
            Style::default().fg(Color::DarkGray).bg(bg),
            Style::default().fg(Color::DarkGray).bg(bg),
        ),
    }
}
