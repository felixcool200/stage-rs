use crate::app::{App, Overlay, WhichKeyEntry};
use crate::theme::Theme;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

pub fn render(app: &App, frame: &mut Frame) {
    let theme = &app.theme;

    // Render which-key popup if open
    if let Some(entries) = &app.which_key {
        render_which_key(frame, entries, theme);
        return;
    }

    match &app.overlay {
        Overlay::None => {}
        Overlay::Confirm { message, .. } => {
            render_confirm(frame, message, theme);
        }
        Overlay::CommitInput { input, amend } => {
            render_commit_input(frame, input, *amend, theme);
        }
        Overlay::GitLog {
            entries,
            selected,
            scroll,
        } => {
            render_git_log(frame, entries, *selected, *scroll, theme);
        }
        Overlay::StashList { entries, selected } => {
            render_stash_list(frame, entries, *selected, theme);
        }
        Overlay::BranchList { entries, selected, creating } => {
            render_branch_list(frame, entries, *selected, creating.as_deref(), theme);
        }
        Overlay::CommitDetail { hash, message, diff_lines, scroll, log_entries, log_selected } => {
            let refs = &log_entries[*log_selected].refs;
            render_commit_detail(frame, hash, message, diff_lines, *scroll, *log_selected, log_entries.len(), refs, theme);
        }
        Overlay::Rebase { entries, selected, .. } => {
            render_rebase(frame, entries, *selected, theme);
        }
        Overlay::DirtyCheckout { branch, has_conflicts } => {
            render_dirty_checkout(frame, branch, *has_conflicts, theme);
        }
    }
}

fn render_confirm(frame: &mut Frame, message: &str, theme: &Theme) {
    let area = centered_rect(50, 20, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Confirm ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.red))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            message,
            Style::default().fg(theme.fg),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [y/Enter] ", Style::default().fg(theme.green).add_modifier(Modifier::BOLD)),
            Span::styled("Yes  ", Style::default().fg(theme.fg_dim)),
            Span::styled(" [n/Esc] ", Style::default().fg(theme.red).add_modifier(Modifier::BOLD)),
            Span::styled("No", Style::default().fg(theme.fg_dim)),
        ]),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn render_dirty_checkout(frame: &mut Frame, branch: &str, has_conflicts: bool, theme: &Theme) {
    let area = centered_rect(50, 25, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Uncommitted Changes ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.yellow))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "You have uncommitted changes.".to_string(),
            Style::default().fg(theme.fg),
        )),
        Line::from(Span::styled(
            format!("Switch to {branch}?"),
            Style::default().fg(theme.yellow),
        )),
        Line::from(""),
    ];

    if has_conflicts {
        lines.push(Line::from(Span::styled(
            " Stash unavailable (unmerged files)",
            Style::default().fg(theme.fg_dim),
        )));
    } else {
        lines.push(Line::from(vec![
            Span::styled(" [s] ", Style::default().fg(theme.cyan).add_modifier(Modifier::BOLD)),
            Span::styled("Stash & switch", Style::default().fg(theme.fg_dim)),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled(" [d] ", Style::default().fg(theme.red).add_modifier(Modifier::BOLD)),
        Span::styled("Discard & switch", Style::default().fg(theme.fg_dim)),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" [Esc] ", Style::default().fg(theme.fg_dim).add_modifier(Modifier::BOLD)),
        Span::styled("Cancel", Style::default().fg(theme.fg_dim)),
    ]));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn render_commit_input(
    frame: &mut Frame,
    input: &crate::app::TextInput,
    amend: bool,
    theme: &Theme,
) {
    let area = centered_rect(60, 50, frame.area());
    frame.render_widget(Clear, area);

    let title = if amend {
        " Amend Commit (Ctrl+S to confirm, Esc to cancel) "
    } else {
        " Commit (Ctrl+S to confirm, Esc to cancel) "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.yellow))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split inner area: text input on top, fixed hint at bottom
    let chunks = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(2),
    ]).split(inner);
    let text_area = chunks[0];
    let hint_area = chunks[1];

    // Build styled lines for text input
    let mut lines: Vec<Line> = Vec::new();
    for line_text in input.lines.iter() {
        let display = if line_text.is_empty() { " " } else { line_text.as_str() };
        lines.push(Line::from(Span::styled(
            display,
            Style::default().fg(theme.fg),
        )));
    }

    // Fill remaining with ~ indicators
    let text_height = text_area.height as usize;
    while lines.len() < text_height {
        lines.push(Line::from(Span::styled("~", Style::default().fg(theme.fg_dim))));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, text_area);

    // Place real terminal cursor
    let cursor_x = text_area.x + input.cursor_col as u16;
    let cursor_y = text_area.y + input.cursor_row as u16;
    frame.set_cursor_position(ratatui::layout::Position { x: cursor_x, y: cursor_y });

    // Fixed hint at bottom
    let hint_lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            " Ctrl+S: commit  Esc: cancel  Enter: new line ",
            Style::default().fg(theme.fg_dim),
        )),
    ];
    frame.render_widget(Paragraph::new(hint_lines), hint_area);
}

fn render_git_log(
    frame: &mut Frame,
    entries: &[crate::git::LogEntry],
    selected: usize,
    _scroll: usize,
    theme: &Theme,
) {
    let area = centered_rect(80, 70, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" Git Log ({} commits) ", entries.len()))
        .title_bottom(Line::from(vec![
            Span::styled(" y", Style::default().fg(theme.yellow)),
            Span::styled(":yank hash  ", Style::default().fg(theme.fg_dim)),
            Span::styled("Enter", Style::default().fg(theme.yellow)),
            Span::styled(":view  ", Style::default().fg(theme.fg_dim)),
            Span::styled("r", Style::default().fg(theme.yellow)),
            Span::styled(":rebase  ", Style::default().fg(theme.fg_dim)),
            Span::styled("q/Esc", Style::default().fg(theme.yellow)),
            Span::styled(":close ", Style::default().fg(theme.fg_dim)),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.cyan))
        .style(Style::default().bg(theme.bg));

    let items: Vec<ListItem> = entries
        .iter()
        .map(|e| {
            let mut spans = vec![
                Span::styled(
                    format!("{} ", e.hash),
                    Style::default()
                        .fg(theme.yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{} ", e.date),
                    Style::default().fg(theme.fg_dim),
                ),
                Span::styled(
                    format!("{} ", e.author),
                    Style::default().fg(theme.green),
                ),
            ];
            for r in &e.refs {
                spans.push(Span::styled(
                    format!("({r}) "),
                    Style::default().fg(theme.cyan).add_modifier(Modifier::BOLD),
                ));
            }
            spans.push(Span::styled(&e.message, Style::default().fg(theme.fg)));
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(theme.fg_dim)
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
    theme: &Theme,
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
        .border_style(Style::default().fg(theme.green))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(name) = creating {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Branch name:",
                Style::default().fg(theme.fg),
            )),
            Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(name, Style::default().fg(theme.cyan)),
                Span::styled("_", Style::default().fg(theme.fg).add_modifier(Modifier::SLOW_BLINK)),
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
                theme.green
            } else if e.is_remote {
                theme.red
            } else {
                theme.fg
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(theme.green)),
                Span::styled(&e.name, Style::default().fg(name_color)),
            ]))
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(theme.fg_dim)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(list, inner, &mut state);
}

#[allow(clippy::too_many_arguments)]
fn render_commit_detail(
    frame: &mut Frame,
    hash: &str,
    message: &str,
    diff_lines: &[String],
    scroll: usize,
    log_index: usize,
    log_total: usize,
    refs: &[String],
    theme: &Theme,
) {
    let area = centered_rect(85, 80, frame.area());
    frame.render_widget(Clear, area);

    let refs_str = if refs.is_empty() {
        String::new()
    } else {
        format!(" ({})", refs.join(", "))
    };

    let block = Block::default()
        .title(format!(" [{}/{}] {hash}{refs_str} - {message} ", log_index + 1, log_total))
        .title_bottom(Line::from(vec![
            Span::styled(" Shift+↑/↓", Style::default().fg(theme.yellow)),
            Span::styled(":prev/next  ", Style::default().fg(theme.fg_dim)),
            Span::styled("↑/↓", Style::default().fg(theme.yellow)),
            Span::styled(":scroll  ", Style::default().fg(theme.fg_dim)),
            Span::styled("q/Esc", Style::default().fg(theme.yellow)),
            Span::styled(":close ", Style::default().fg(theme.fg_dim)),
        ]))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.yellow))
        .style(Style::default().bg(theme.bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_height = inner.height as usize;
    let lines: Vec<Line> = diff_lines
        .iter()
        .skip(scroll)
        .take(visible_height)
        .map(|line| {
            let color = if line.starts_with('+') {
                theme.green
            } else if line.starts_with('-') {
                theme.red
            } else if line.starts_with("@@") {
                theme.cyan
            } else {
                theme.fg
            };
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
    theme: &Theme,
) {
    let area = centered_rect(75, 60, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Interactive Rebase [Space]cycle [Shift+↑/↓]reorder [Enter]execute [q]cancel ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.magenta))
        .style(Style::default().bg(theme.bg));

    let items: Vec<ListItem> = entries
        .iter()
        .map(|e| {
            let action_color = match e.action {
                crate::app::RebaseAction::Pick => theme.green,
                crate::app::RebaseAction::Squash => theme.yellow,
                crate::app::RebaseAction::Drop => theme.red,
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:>7} ", e.action.label()),
                    Style::default().fg(action_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{} ", e.hash),
                    Style::default().fg(theme.yellow),
                ),
                Span::styled(&e.message, Style::default().fg(theme.fg)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(theme.fg_dim)
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
    theme: &Theme,
) {
    let area = centered_rect(70, 50, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(
            " Stashes ({}) [p]op [a]pply [d]rop [q]close ",
            entries.len()
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.magenta))
        .style(Style::default().bg(theme.bg));

    let items: Vec<ListItem> = entries
        .iter()
        .map(|e| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("stash@{{{}}} ", e.index),
                    Style::default()
                        .fg(theme.yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&e.message, Style::default().fg(theme.fg)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(theme.fg_dim)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(Some(selected));
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_which_key(frame: &mut Frame, entries: &[WhichKeyEntry], theme: &Theme) {
    let cols = 3;
    let rows = entries.len().div_ceil(cols);
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
        .border_style(Style::default().fg(theme.fg_dim))
        .style(Style::default().bg(theme.bg));
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
                        .fg(theme.cyan)
                        .add_modifier(Modifier::BOLD),
                ));
                let label = format!(" {}", entry.label);
                let pad = col_width.saturating_sub(2 + entry.label.len());
                spans.push(Span::styled(
                    label,
                    Style::default().fg(theme.fg),
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
