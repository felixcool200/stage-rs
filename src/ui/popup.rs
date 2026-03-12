use crate::app::{App, Overlay};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

pub fn render(app: &App, frame: &mut Frame) {
    match &app.overlay {
        Overlay::None => {}
        Overlay::Confirm { message, .. } => {
            render_confirm(frame, message);
        }
        Overlay::CommitInput { input, amend } => {
            render_commit_input(frame, input, *amend);
        }
        Overlay::GitLog {
            entries,
            selected,
            scroll,
        } => {
            render_git_log(frame, entries, *selected, *scroll);
        }
    }
}

fn render_confirm(frame: &mut Frame, message: &str) {
    let area = centered_rect(50, 20, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Confirm ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            message,
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [y/Enter] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("Yes  ", Style::default().fg(Color::DarkGray)),
            Span::styled(" [n/Esc] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled("No", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn render_commit_input(
    frame: &mut Frame,
    input: &crate::app::TextInput,
    amend: bool,
) {
    let area = centered_rect(60, 50, frame.area());
    frame.render_widget(Clear, area);

    let title = if amend {
        " Amend Commit (Ctrl+Enter to confirm, Esc to cancel) "
    } else {
        " Commit (Ctrl+Enter to confirm, Esc to cancel) "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build styled lines with cursor
    let mut lines: Vec<Line> = Vec::new();
    for (row, line_text) in input.lines.iter().enumerate() {
        if row == input.cursor_row {
            // Insert a cursor indicator
            let col = input.cursor_col;
            let before: String = line_text.chars().take(col).collect();
            let cursor_char: String = line_text.chars().skip(col).take(1).collect();
            let after: String = line_text.chars().skip(col + 1).collect();

            let cursor_display = if cursor_char.is_empty() {
                " ".to_string()
            } else {
                cursor_char
            };

            lines.push(Line::from(vec![
                Span::styled(before, Style::default().fg(Color::White)),
                Span::styled(
                    cursor_display,
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White),
                ),
                Span::styled(after, Style::default().fg(Color::White)),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                line_text.as_str(),
                Style::default().fg(Color::White),
            )));
        }
    }

    // Show hint below the input
    let input_height = inner.height.saturating_sub(2) as usize;
    while lines.len() < input_height {
        lines.push(Line::from(Span::styled("~", Style::default().fg(Color::DarkGray))));
    }

    // Add hint at the bottom
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            " Ctrl+Enter: commit  Esc: cancel  Enter: new line ",
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn render_git_log(
    frame: &mut Frame,
    entries: &[crate::git::LogEntry],
    selected: usize,
    _scroll: usize,
) {
    let area = centered_rect(80, 70, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" Git Log ({} commits) [q/Esc to close] ", entries.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let items: Vec<ListItem> = entries
        .iter()
        .map(|e| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", e.hash),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{} ", e.date),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{} ", e.author),
                    Style::default().fg(Color::Green),
                ),
                Span::styled(&e.message, Style::default().fg(Color::White)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(list, area, &mut state);
}

/// Create a centered rectangle with the given percentage of width and height.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let [_, v_center, _] = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .areas(area);

    let [_, h_center, _] = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .areas(v_center);

    h_center
}
