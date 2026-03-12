use crate::app::{App, Panel};
use crate::git::DiffLineKind;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
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

    let visible_height = area.height.saturating_sub(2) as usize; // minus borders
    let lines: Vec<Line> = ds
        .left_lines
        .iter()
        .enumerate()
        .skip(ds.scroll)
        .take(visible_height)
        .map(|(i, dl)| {
            let line_num = format!("{:>4} ", i + 1);
            let (num_style, text_style) = match dl.kind {
                DiffLineKind::Equal => (
                    Style::default().fg(Color::DarkGray),
                    Style::default().fg(Color::White),
                ),
                DiffLineKind::Removed => (
                    Style::default().fg(Color::DarkGray),
                    Style::default().fg(Color::Red),
                ),
                DiffLineKind::Spacer => (
                    Style::default().fg(Color::DarkGray),
                    Style::default().fg(Color::DarkGray),
                ),
                DiffLineKind::Added => (
                    Style::default().fg(Color::DarkGray),
                    Style::default().fg(Color::Green),
                ),
            };
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
        Some(ds) => format!(" Working Tree: {} ", ds.file_path),
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
            let line_num = format!("{:>4} ", i + 1);
            let (num_style, text_style) = match dl.kind {
                DiffLineKind::Equal => (
                    Style::default().fg(Color::DarkGray),
                    Style::default().fg(Color::White),
                ),
                DiffLineKind::Added => (
                    Style::default().fg(Color::DarkGray),
                    Style::default().fg(Color::Green),
                ),
                DiffLineKind::Spacer => (
                    Style::default().fg(Color::DarkGray),
                    Style::default().fg(Color::DarkGray),
                ),
                DiffLineKind::Removed => (
                    Style::default().fg(Color::DarkGray),
                    Style::default().fg(Color::Red),
                ),
            };
            Line::from(vec![
                Span::styled(line_num, num_style),
                Span::styled(&dl.content, text_style),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
