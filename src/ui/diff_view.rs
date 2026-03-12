use crate::app::{App, Panel};
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
        let placeholder =
            Paragraph::new("Select a file to view diff").block(block);
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
            let is_current_hunk = is_focused
                && dl.hunk_index.is_some()
                && dl.hunk_index == ds.hunks.get(ds.current_hunk).and_then(|h| {
                    // Check if this display line is in the current hunk
                    if i >= h.display_start && i < h.display_end {
                        ds.left_lines[i].hunk_index
                    } else {
                        None
                    }
                });

            let line_num = format!("{:>4} ", i + 1);
            let (num_style, text_style) = line_styles(dl.kind.clone(), is_current_hunk);
            Line::from(vec![
                Span::styled(line_num, num_style),
                Span::styled(&dl.content, text_style),
            ])
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
            let hunk_info = if ds.hunks.is_empty() {
                String::new()
            } else {
                format!(" [hunk {}/{}]", ds.current_hunk + 1, ds.hunks.len())
            };
            format!(" Working Tree: {}{} ", ds.file_path, hunk_info)
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
            let is_current_hunk = is_focused
                && dl.hunk_index.is_some()
                && ds
                    .hunks
                    .get(ds.current_hunk)
                    .map(|h| i >= h.display_start && i < h.display_end)
                    .unwrap_or(false);

            // Show hunk header line at the start of the current hunk
            if is_current_hunk && ds.hunks.get(ds.current_hunk).map(|h| h.display_start) == Some(i)
            {
                return hunk_header_line(ds, i);
            }

            let line_num = format!("{:>4} ", i + 1);
            let (num_style, text_style) = line_styles(dl.kind.clone(), is_current_hunk);
            Line::from(vec![
                Span::styled(line_num, num_style),
                Span::styled(&dl.content, text_style),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn hunk_header_line(ds: &crate::app::DiffState, display_row: usize) -> Line<'static> {
    let hunk = &ds.hunks[ds.current_hunk];
    let dl = &ds.right_lines[display_row];
    let line_num = format!("{:>4} ", display_row + 1);

    let (num_style, text_style) = line_styles(dl.kind.clone(), true);

    // Show the hunk header + content on the first line
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

fn line_styles(kind: DiffLineKind, is_current_hunk: bool) -> (Style, Style) {
    let bg = if is_current_hunk {
        Color::Rgb(30, 30, 50)
    } else {
        Color::Reset
    };

    match kind {
        DiffLineKind::Equal => (
            Style::default().fg(Color::DarkGray).bg(bg),
            Style::default().fg(Color::White).bg(bg),
        ),
        DiffLineKind::Removed => (
            Style::default().fg(Color::DarkGray).bg(bg),
            Style::default().fg(Color::Red).bg(if is_current_hunk {
                Color::Rgb(60, 20, 20)
            } else {
                Color::Reset
            }),
        ),
        DiffLineKind::Added => (
            Style::default().fg(Color::DarkGray).bg(bg),
            Style::default().fg(Color::Green).bg(if is_current_hunk {
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
