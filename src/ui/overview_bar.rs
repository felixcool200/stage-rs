use crate::git::{DiffLine, DiffLineKind};
use crate::theme::Theme;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Render a 1-column-wide change overview bar showing where modifications are in the file.
/// Green = added, Red = removed, Yellow = both, DarkGray = viewport indicator.
pub fn render(
    frame: &mut Frame,
    lines: &[DiffLine],
    scroll: usize,
    visible_height: usize,
    area: Rect,
    theme: &Theme,
) {
    let bar_height = area.height as usize;
    if bar_height == 0 {
        return;
    }

    let total_lines = lines.len().max(1);
    let mut bar_lines: Vec<Line> = Vec::with_capacity(bar_height);

    for row in 0..bar_height {
        let file_start = row * total_lines / bar_height;
        let file_end = ((row + 1) * total_lines / bar_height).max(file_start + 1);

        let mut has_added = false;
        let mut has_removed = false;
        for dl in lines
            .iter()
            .take(file_end.min(lines.len()))
            .skip(file_start)
        {
            match dl.kind {
                DiffLineKind::Added => has_added = true,
                DiffLineKind::Removed | DiffLineKind::Spacer => {
                    if dl.hunk_index.is_some() {
                        has_removed = true;
                    }
                }
                _ => {}
            }
        }

        let in_viewport = file_start < scroll + visible_height && file_end > scroll;

        let (ch, color) = if has_added && has_removed {
            ("┃", theme.yellow)
        } else if has_added {
            ("┃", theme.green)
        } else if has_removed {
            ("┃", theme.red)
        } else if in_viewport {
            ("│", theme.fg_dim)
        } else {
            (" ", theme.fg_dim)
        };

        let bg = if in_viewport {
            theme.diff_viewport_bg
        } else {
            theme.bg
        };

        bar_lines.push(Line::from(Span::styled(
            ch,
            Style::default().fg(color).bg(bg),
        )));
    }

    frame.render_widget(Paragraph::new(bar_lines), area);
}
