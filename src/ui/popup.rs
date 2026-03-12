use crate::app::{App, Overlay, WhichKeyEntry};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

pub fn render(app: &App, frame: &mut Frame) {
    // Render which-key popup if open
    if let Some(entries) = &app.which_key {
        render_which_key(frame, entries);
        return;
    }

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
        Overlay::StashList { entries, selected } => {
            render_stash_list(frame, entries, *selected);
        }
        Overlay::BranchList { entries, selected, creating } => {
            render_branch_list(frame, entries, *selected, creating.as_deref());
        }
        Overlay::CommitDetail { hash, message, diff_lines, scroll } => {
            render_commit_detail(frame, hash, message, diff_lines, *scroll);
        }
        Overlay::Rebase { entries, selected, .. } => {
            render_rebase(frame, entries, *selected);
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

fn render_branch_list(
    frame: &mut Frame,
    entries: &[crate::git::BranchEntry],
    selected: usize,
    creating: Option<&str>,
) {
    let area = centered_rect(60, 60, frame.area());
    frame.render_widget(Clear, area);

    let title = if creating.is_some() {
        " New Branch (Enter to create, Esc to cancel) "
    } else {
        " Branches [Enter]checkout [n]ew [q]close "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(name) = creating {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Branch name:",
                Style::default().fg(Color::White),
            )),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(name, Style::default().fg(Color::Cyan)),
                Span::styled("_", Style::default().fg(Color::White).add_modifier(Modifier::SLOW_BLINK)),
            ]),
        ];
        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
        return;
    }

    let items: Vec<ListItem> = entries
        .iter()
        .map(|e| {
            let marker = if e.is_current { "* " } else { "  " };
            let name_color = if e.is_current {
                Color::Green
            } else if e.is_remote {
                Color::Red
            } else {
                Color::White
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Green)),
                Span::styled(&e.name, Style::default().fg(name_color)),
            ]))
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(list, inner, &mut state);
}

fn render_commit_detail(
    frame: &mut Frame,
    hash: &str,
    message: &str,
    diff_lines: &[String],
    scroll: usize,
) {
    let area = centered_rect(85, 80, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" {hash} - {message} [q/Esc to close, ↑/↓ to scroll] "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_height = inner.height as usize;
    let lines: Vec<Line> = diff_lines
        .iter()
        .skip(scroll)
        .take(visible_height)
        .map(|line| {
            let (color, prefix_style) = if line.starts_with('+') {
                (Color::Green, Color::Green)
            } else if line.starts_with('-') {
                (Color::Red, Color::Red)
            } else if line.starts_with("@@") {
                (Color::Cyan, Color::Cyan)
            } else {
                (Color::White, Color::DarkGray)
            };
            let _ = prefix_style;
            Line::from(Span::styled(line.as_str(), Style::default().fg(color)))
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

fn render_rebase(
    frame: &mut Frame,
    entries: &[crate::app::RebaseEntry],
    selected: usize,
) {
    let area = centered_rect(75, 60, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Interactive Rebase [Space]cycle [Shift+↑/↓]reorder [Enter]execute [q]cancel ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let items: Vec<ListItem> = entries
        .iter()
        .map(|e| {
            let action_color = match e.action {
                crate::app::RebaseAction::Pick => Color::Green,
                crate::app::RebaseAction::Squash => Color::Yellow,
                crate::app::RebaseAction::Drop => Color::Red,
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:>7} ", e.action.label()),
                    Style::default().fg(action_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{} ", e.hash),
                    Style::default().fg(Color::Yellow),
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

fn render_stash_list(
    frame: &mut Frame,
    entries: &[crate::git::StashEntry],
    selected: usize,
) {
    let area = centered_rect(70, 50, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(
            " Stashes ({}) [p]op [a]pply [d]rop [q]close ",
            entries.len()
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let items: Vec<ListItem> = entries
        .iter()
        .map(|e| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("stash@{{{}}} ", e.index),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
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

fn render_which_key(frame: &mut Frame, entries: &[WhichKeyEntry]) {
    let cols = 3;
    let rows = (entries.len() + cols - 1) / cols;
    let popup_height = rows as u16 + 2; // +2 for borders
    let area = frame.area();

    // Position at the bottom of the screen
    let popup_area = Rect {
        x: area.x,
        y: area.height.saturating_sub(popup_height + 1), // +1 for footer
        width: area.width,
        height: popup_height,
    };

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let col_width = inner.width as usize / cols;

    let mut lines: Vec<Line> = Vec::new();
    for row in 0..rows {
        let mut spans = Vec::new();
        for col in 0..cols {
            let idx = row + col * rows;
            if let Some(entry) = entries.get(idx) {
                spans.push(Span::styled(
                    format!(" {}", entry.key),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
                let label = format!(" {}", entry.label);
                let pad = col_width.saturating_sub(2 + entry.label.len());
                spans.push(Span::styled(
                    label,
                    Style::default().fg(Color::White),
                ));
                spans.push(Span::raw(" ".repeat(pad)));
            }
        }
        lines.push(Line::from(spans));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
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
