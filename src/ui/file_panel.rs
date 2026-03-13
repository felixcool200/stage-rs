use crate::app::{App, Panel};
use crate::git::FileStatus;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let is_focused = app.active_panel == Panel::FileList;
    let border_color = if is_focused {
        app.theme.cyan
    } else {
        app.theme.fg_dim
    };

    let filtering = app.file_filter.is_some();
    let title = if filtering {
        " Source Control (filtering) "
    } else {
        " Source Control "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(app.theme.bg));

    // Reserve a line at the bottom for filter input when filtering
    let (list_area, filter_area) = if filtering {
        let inner = block.inner(area);
        let [la, fa] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(inner);
        (la, Some(fa))
    } else {
        (block.inner(area), None)
    };

    frame.render_widget(block, area);

    let filtered = app.filtered_entries();
    let filtered_indices: std::collections::HashSet<usize> = filtered.iter().map(|(i, _)| *i).collect();

    let mut items: Vec<ListItem> = Vec::new();
    // None = header or section label (not a file), Some(i) = file entry index
    let mut list_index_to_entry: Vec<Option<usize>> = Vec::new();
    let mut current_section: Option<&str> = None;

    // "Repository" header entry at position 0
    let header_style = if app.header_selected && is_focused {
        Style::default()
            .fg(app.theme.cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(app.theme.fg)
    };
    items.push(ListItem::new(Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled("Repository", header_style),
    ])));
    list_index_to_entry.push(None); // header, not a file

    for (i, entry) in app.file_entries.iter().enumerate() {
        if filtering && !filtered_indices.contains(&i) {
            continue;
        }

        let section = entry.status.section_name();
        if current_section != Some(section) {
            current_section = Some(section);
            let count = if filtering {
                filtered.iter().filter(|(_, e)| e.status.section_name() == section).count()
            } else {
                app.file_entries.iter().filter(|e| e.status.section_name() == section).count()
            };
            items.push(ListItem::new(Line::from(vec![Span::styled(
                format!(" {section} ({count})"),
                Style::default()
                    .fg(app.theme.yellow)
                    .add_modifier(Modifier::BOLD),
            )])));
            list_index_to_entry.push(None);
        }

        let status_color = match &entry.status {
            FileStatus::Staged(_) => app.theme.green,
            FileStatus::Unstaged(_) => app.theme.yellow,
            FileStatus::Conflict => app.theme.red,
            FileStatus::Untracked => app.theme.gray,
        };

        let label = entry.status.short_label();
        let filename = entry
            .path
            .rsplit('/')
            .next()
            .unwrap_or(&entry.path);
        let dir = if entry.path.contains('/') {
            let parent = &entry.path[..entry.path.len() - filename.len()];
            format!(" {parent}")
        } else {
            String::new()
        };

        let first_line = Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{label} "), Style::default().fg(status_color)),
            Span::styled(filename, Style::default().fg(app.theme.fg)),
        ]);

        let mut second_spans: Vec<Span> = Vec::new();
        if !dir.is_empty() {
            second_spans.push(Span::styled(format!("    {}", dir.trim()), Style::default().fg(app.theme.fg_dim)));
        }
        if entry.insertions > 0 || entry.deletions > 0 {
            if !second_spans.is_empty() {
                second_spans.push(Span::styled(" ", Style::default()));
            } else {
                second_spans.push(Span::styled("     ", Style::default()));
            }
            if entry.insertions > 0 {
                second_spans.push(Span::styled(
                    format!("+{}", entry.insertions),
                    Style::default().fg(app.theme.green),
                ));
            }
            if entry.deletions > 0 {
                if entry.insertions > 0 {
                    second_spans.push(Span::styled(" ", Style::default()));
                }
                second_spans.push(Span::styled(
                    format!("-{}", entry.deletions),
                    Style::default().fg(app.theme.red),
                ));
            }
        }

        if second_spans.is_empty() {
            items.push(ListItem::new(first_line));
        } else {
            items.push(ListItem::new(vec![first_line, Line::from(second_spans)]));
        }
        list_index_to_entry.push(Some(i));
    }

    // Map selection to the list display index
    let display_index = if app.header_selected {
        Some(0) // Header is always at position 0
    } else {
        list_index_to_entry
            .iter()
            .position(|e| *e == Some(app.selected_index))
    };

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(app.theme.fg_dim)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(display_index);
    frame.render_stateful_widget(list, list_area, &mut state);

    // Render filter input
    if let (Some(fa), Some(ref filter_text)) = (filter_area, &app.file_filter) {
        let line = Line::from(vec![
            Span::styled("/", Style::default().fg(app.theme.cyan)),
            Span::styled(filter_text.as_str(), Style::default().fg(app.theme.fg)),
            Span::styled("_", Style::default().fg(app.theme.fg).add_modifier(Modifier::SLOW_BLINK)),
        ]);
        frame.render_widget(Paragraph::new(line), fa);
    }
}
