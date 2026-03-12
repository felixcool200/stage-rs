use crate::app::{App, Panel};
use crate::git::FileStatus;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let is_focused = app.active_panel == Panel::FileList;
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .title(" Source Control ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let mut items: Vec<ListItem> = Vec::new();
    let mut list_index_to_entry: Vec<Option<usize>> = Vec::new();
    let mut current_section: Option<&str> = None;

    for (i, entry) in app.file_entries.iter().enumerate() {
        let section = entry.status.section_name();
        if current_section != Some(section) {
            current_section = Some(section);
            // Add section header
            let count = app
                .file_entries
                .iter()
                .filter(|e| e.status.section_name() == section)
                .count();
            items.push(ListItem::new(Line::from(vec![Span::styled(
                format!(" {section} ({count})"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )])));
            list_index_to_entry.push(None);
        }

        let status_color = match &entry.status {
            FileStatus::Staged(_) => Color::Green,
            FileStatus::Unstaged(_) => Color::Yellow,
            FileStatus::Conflict => Color::Red,
            FileStatus::Untracked => Color::Gray,
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

        let mut spans = vec![
            Span::raw("  "),
            Span::styled(format!("{label} "), Style::default().fg(status_color)),
            Span::styled(filename, Style::default().fg(Color::White)),
            Span::styled(dir, Style::default().fg(Color::DarkGray)),
        ];
        if entry.insertions > 0 || entry.deletions > 0 {
            spans.push(Span::styled(" ", Style::default()));
            if entry.insertions > 0 {
                spans.push(Span::styled(
                    format!("+{}", entry.insertions),
                    Style::default().fg(Color::Green),
                ));
            }
            if entry.deletions > 0 {
                if entry.insertions > 0 {
                    spans.push(Span::styled(" ", Style::default()));
                }
                spans.push(Span::styled(
                    format!("-{}", entry.deletions),
                    Style::default().fg(Color::Red),
                ));
            }
        }
        items.push(ListItem::new(Line::from(spans)));
        list_index_to_entry.push(Some(i));
    }

    // Map app.selected_index to the list display index
    let display_index = list_index_to_entry
        .iter()
        .position(|e| *e == Some(app.selected_index));

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(display_index);
    frame.render_stateful_widget(list, area, &mut state);
}
