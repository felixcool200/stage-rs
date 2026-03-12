use crate::app::{App, DiffViewMode, Panel};
use crate::git::DiffLineKind;
use ratatui::layout::Rect;
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
            // Show selection indicator in line mode
            if ds.view_mode == DiffViewMode::LineNav && dl.hunk_index.is_some() {
                let marker = if ds.selected_lines.contains(&i) {
                    Span::styled(
                        "[x]",
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    )
                } else if i == ds.cursor_line {
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
            spans.push(Span::styled(&dl.content, text_style));
            Line::from(spans)
        })
        .collect();

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

pub fn render_right(app: &App, frame: &mut Frame, area: Rect) {
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
                let marker = if ds.selected_lines.contains(&i) {
                    Span::styled(
                        "[x]",
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                    )
                } else if i == ds.cursor_line {
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
            spans.push(Span::styled(&dl.content, text_style));
            Line::from(spans)
        })
        .collect();

    let paragraph = Paragraph::new(lines).block(block);
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
