use crate::app::{App, ConflictResolution, ConflictState, DiffState, DiffViewMode, Panel};
use crate::git::DiffLineKind;
use crate::keymap::{self, InputContext};
use crate::syntax;
use crate::theme::Theme;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render_left(app: &App, frame: &mut Frame, area: Rect) {
    let theme = &app.theme;
    let block = Block::default()
        .title(" Index / HEAD ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.fg_dim))
        .style(Style::default().bg(theme.bg));

    let Some(ds) = &app.diff_state else {
        let text = if app.large_file_skipped.is_some() {
            ""
        } else {
            "Select a file to view diff"
        };
        let placeholder = Paragraph::new(text).block(block);
        frame.render_widget(placeholder, area);
        return;
    };

    let visible_height = area.height.saturating_sub(2) as usize;
    let is_focused = app.active_panel == Panel::DiffView;

    let blame_data = if app.show_blame {
        app.blame_data.as_deref()
    } else {
        None
    };

    // Build a map from display line index to old-file line index for blame
    let blame_map: Vec<Option<usize>> = if blame_data.is_some() {
        let mut file_line = 0usize;
        ds.left_lines
            .iter()
            .map(|dl| {
                match dl.kind {
                    DiffLineKind::Equal | DiffLineKind::Removed => {
                        let idx = file_line;
                        file_line += 1;
                        Some(idx)
                    }
                    _ => None, // Spacer lines have no blame
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    let lines: Vec<Line> = ds
        .left_lines
        .iter()
        .enumerate()
        .skip(ds.scroll)
        .take(visible_height)
        .map(|(i, dl)| {
            let highlight = get_line_highlight(ds, i, is_focused);
            let line_num = format!("{:>4} ", i + 1);
            let (num_style, text_style) = line_styles(&dl.kind, &highlight, theme);

            let mut spans = Vec::new();

            // Show blame annotation if enabled
            if let Some(blame) = blame_data {
                let blame_line = blame_map
                    .get(i)
                    .copied()
                    .flatten()
                    .and_then(|fi| blame.get(fi));
                if let Some(bl) = blame_line {
                    let annotation = format!("{} {:>8} ", bl.hash, truncate_str(&bl.author, 8));
                    spans.push(Span::styled(annotation, Style::default().fg(theme.fg_dim)));
                } else {
                    spans.push(Span::styled("                  ", Style::default()));
                }
            }

            // Show selection indicator in line mode
            if ds.view_mode == DiffViewMode::LineNav && dl.hunk_index.is_some() {
                spans.push(line_mode_marker(ds, i, theme));
            }
            spans.push(Span::styled(line_num, num_style));
            let visible = skip_chars(&dl.content, ds.h_scroll);
            push_highlighted_content(app, visible, &dl.kind, text_style, &mut spans);
            Line::from(spans)
        })
        .collect();

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

pub fn render_right(app: &App, frame: &mut Frame, area: Rect) {
    // If in conflict resolver mode
    if let Some(cs) = &app.conflict_state {
        let focused = app.active_panel == crate::app::Panel::DiffView;
        render_conflict(frame, cs, focused, area, &app.theme);
        return;
    }

    render_right_diff(app, frame, area);
}

fn render_right_diff(app: &App, frame: &mut Frame, area: Rect) {
    let theme = &app.theme;
    let is_focused = app.active_panel == Panel::DiffView;
    let border_color = if is_focused { theme.cyan } else { theme.fg_dim };

    let title = match &app.diff_state {
        Some(ds) => {
            let panel_label = if ds.is_staged {
                "Index"
            } else {
                "Working Tree"
            };
            let mode_info = match ds.view_mode {
                DiffViewMode::HunkNav if !ds.hunks.is_empty() => {
                    format!(" [hunk {}/{}]", ds.current_hunk + 1, ds.hunks.len())
                }
                DiffViewMode::LineNav => {
                    let sel = ds.selected_lines.len();
                    let total = ds.hunk_changed_rows.len();
                    let verb = if ds.is_staged {
                        "to unstage"
                    } else {
                        "selected"
                    };
                    format!(
                        " [{sel}/{total} {verb} | hunk {}/{}]",
                        ds.current_hunk + 1,
                        ds.hunks.len()
                    )
                }
                _ => String::new(),
            };
            format!(" {panel_label}: {}{} ", ds.file_path, mode_info)
        }
        None => " Working Tree ".into(),
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(theme.bg));

    // Show large file warning instead of diff
    if let Some((ref path, size, _)) = app.large_file_skipped {
        let size_str = format_file_size(size);
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  {path}"),
                Style::default().fg(theme.fg),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("  File is too large to auto-diff ({size_str})"),
                Style::default().fg(theme.yellow),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Press Enter to load anyway",
                Style::default().fg(theme.fg_dim),
            )),
        ];
        frame.render_widget(Paragraph::new(lines).block(block), area);
        return;
    }

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

            let line_num = format!("{:>4} ", i + 1);
            let (num_style, text_style) = line_styles(&dl.kind, &highlight, theme);

            let mut spans = Vec::new();
            // Show selection indicator in line mode
            if ds.view_mode == DiffViewMode::LineNav && dl.hunk_index.is_some() {
                spans.push(line_mode_marker(ds, i, theme));
            }
            spans.push(Span::styled(line_num, num_style));
            let visible = skip_chars(&dl.content, ds.h_scroll);
            push_highlighted_content(app, visible, &dl.kind, text_style, &mut spans);
            Line::from(spans)
        })
        .collect();

    // Split area: main diff content + 1-col overview bar on the right
    let [main_area, bar_area] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, main_area);

    super::overview_bar::render(
        frame,
        &ds.right_lines,
        ds.scroll,
        bar_area.height as usize,
        bar_area,
        theme,
    );
}

#[derive(PartialEq, Eq)]
enum LineHighlight {
    None,
    CurrentHunk,
    CursorLine,
    SelectedStage,
    SelectedUnstage,
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
                if ds.is_staged {
                    LineHighlight::SelectedUnstage
                } else {
                    LineHighlight::SelectedStage
                }
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

fn render_conflict(
    frame: &mut Frame,
    cs: &ConflictState,
    focused: bool,
    area: Rect,
    theme: &Theme,
) {
    let section = &cs.sections[cs.current_section];

    let (res_label, res_color) = match section.resolution {
        ConflictResolution::Unresolved => ("UNRESOLVED", theme.red),
        ConflictResolution::Ours => (&*cs.left_name, theme.cyan),
        ConflictResolution::Theirs => (&*cs.right_name, theme.magenta),
        ConflictResolution::Both => ("BOTH", theme.green),
    };

    // Top status bar
    let top_line = Line::from(vec![
        Span::styled(
            format!(" {} ", cs.file_path),
            Style::default().fg(theme.fg).bg(theme.fg_dim),
        ),
        Span::styled(
            format!(" {}/{} ", cs.current_section + 1, cs.sections.len()),
            Style::default().fg(theme.yellow),
        ),
        Span::styled(
            format!(" {res_label} "),
            Style::default()
                .fg(theme.black)
                .bg(res_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", Style::default()),
    ]);
    let hint = keymap::hint_line(InputContext::ConflictNav, theme);
    let top_line = {
        let mut spans = top_line.spans;
        spans.extend(hint.spans);
        Line::from(spans)
    };
    let status_area = Rect { height: 1, ..area };
    frame.render_widget(Paragraph::new(top_line), status_area);

    // Side-by-side panels below status
    let panels_area = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    let halves = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(panels_area);

    let left_selected = matches!(
        section.resolution,
        ConflictResolution::Ours | ConflictResolution::Both
    );
    let right_selected = matches!(
        section.resolution,
        ConflictResolution::Theirs | ConflictResolution::Both
    );

    // Colors depend on focus: bright when focused, dim when viewing from file list
    let (left_fg, left_border, left_bg) = if left_selected {
        (theme.cyan, theme.cyan, theme.conflict_ours_bg)
    } else if focused {
        (
            theme.conflict_dim_fg,
            theme.conflict_dim_border,
            theme.conflict_dim_bg,
        )
    } else {
        (theme.fg_dim, theme.fg_dim, theme.bg)
    };

    let (right_fg, right_border, right_bg) = if right_selected {
        (
            theme.conflict_theirs_accent,
            theme.conflict_theirs_accent,
            theme.conflict_theirs_bg,
        )
    } else if focused {
        (
            theme.conflict_dim_fg,
            theme.conflict_dim_border,
            theme.conflict_dim_bg,
        )
    } else {
        (theme.fg_dim, theme.fg_dim, theme.bg)
    };

    let left_title_fg = if focused || left_selected {
        theme.cyan
    } else {
        theme.fg_dim
    };
    let right_title_fg = if focused || right_selected {
        theme.magenta
    } else {
        theme.fg_dim
    };

    // Left panel (ours / left branch)
    let left_block = Block::default()
        .title(Span::styled(
            format!(" {} ", cs.left_name),
            Style::default()
                .fg(left_title_fg)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(left_border))
        .style(Style::default().bg(left_bg));

    let left_inner = left_block.inner(halves[0]);
    frame.render_widget(left_block, halves[0]);

    // Gather context lines: before = previous suffix (or prefix), after = current suffix
    let context_before: &[String] = if cs.current_section == 0 {
        &cs.prefix
    } else {
        &cs.sections[cs.current_section - 1].suffix
    };
    let context_after: &[String] = &section.suffix;

    let context_style = Style::default().fg(theme.fg_dim);
    let marker_style = Style::default().fg(theme.fg_dim);

    let mut left_lines: Vec<Line> = Vec::new();
    for l in context_before {
        left_lines.push(Line::from(Span::styled(format!("  {l}"), context_style)));
    }
    for l in &section.ours {
        left_lines.push(Line::from(vec![
            Span::styled("~ ", marker_style),
            Span::styled(l.to_string(), Style::default().fg(left_fg).bg(left_bg)),
        ]));
    }
    for l in context_after {
        left_lines.push(Line::from(Span::styled(format!("  {l}"), context_style)));
    }

    // Scroll so the conflict lines are visible (skip context_before if it's too long)
    let panel_height = left_inner.height as usize;
    let conflict_start = context_before.len();
    let scroll = if conflict_start > panel_height / 3 {
        conflict_start.saturating_sub(panel_height / 3)
    } else {
        0
    };

    frame.render_widget(
        Paragraph::new(left_lines).scroll((scroll as u16, 0)),
        left_inner,
    );

    // Right panel (theirs / right branch)
    let right_block = Block::default()
        .title(Span::styled(
            format!(" {} ", cs.right_name),
            Style::default()
                .fg(right_title_fg)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(right_border))
        .style(Style::default().bg(right_bg));

    let right_inner = right_block.inner(halves[1]);
    frame.render_widget(right_block, halves[1]);

    let mut right_lines: Vec<Line> = Vec::new();
    for l in context_before {
        right_lines.push(Line::from(Span::styled(format!("  {l}"), context_style)));
    }
    for l in &section.theirs {
        right_lines.push(Line::from(vec![
            Span::styled("~ ", marker_style),
            Span::styled(l.to_string(), Style::default().fg(right_fg).bg(right_bg)),
        ]));
    }
    for l in context_after {
        right_lines.push(Line::from(Span::styled(format!("  {l}"), context_style)));
    }

    frame.render_widget(
        Paragraph::new(right_lines).scroll((scroll as u16, 0)),
        right_inner,
    );
}

/// Push syntax-highlighted spans for a line's content, falling back to a single styled span.
/// Return the substring after skipping `n` characters (char-aware).
fn skip_chars(s: &str, n: usize) -> &str {
    if n == 0 {
        return s;
    }
    let byte_offset = s.char_indices().nth(n).map(|(i, _)| i).unwrap_or(s.len());
    &s[byte_offset..]
}

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
            let bg = fallback_style.bg.unwrap_or(app.theme.bg);
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

fn line_styles(kind: &DiffLineKind, highlight: &LineHighlight, theme: &Theme) -> (Style, Style) {
    let (bg, change_bg_boost) = match highlight {
        LineHighlight::CursorLine => (theme.diff_cursor_bg, true),
        LineHighlight::SelectedStage => (theme.diff_selected_stage_bg, true),
        LineHighlight::SelectedUnstage => (theme.diff_selected_unstage_bg, true),
        LineHighlight::CurrentHunk => (theme.diff_hunk_bg, true),
        LineHighlight::None => (theme.bg, false),
    };

    match kind {
        DiffLineKind::Equal => (
            Style::default().fg(theme.fg_dim).bg(bg),
            Style::default().fg(theme.fg).bg(bg),
        ),
        DiffLineKind::Removed => (
            Style::default().fg(theme.fg_dim).bg(bg),
            Style::default().fg(theme.red).bg(if change_bg_boost {
                theme.diff_removed_bg_bright
            } else {
                theme.diff_removed_bg
            }),
        ),
        DiffLineKind::Added => {
            let is_unstage = *highlight == LineHighlight::SelectedUnstage;
            (
                Style::default().fg(theme.fg_dim).bg(bg),
                Style::default()
                    .fg(if is_unstage { theme.red } else { theme.green })
                    .bg(if is_unstage {
                        if change_bg_boost {
                            theme.diff_removed_bg_bright
                        } else {
                            theme.diff_removed_bg
                        }
                    } else if change_bg_boost {
                        theme.diff_added_bg_bright
                    } else {
                        theme.diff_added_bg
                    }),
            )
        }
        DiffLineKind::Spacer => (
            Style::default().fg(theme.fg_dim).bg(bg),
            Style::default().fg(theme.fg_dim).bg(bg),
        ),
    }
}

/// Produce the 2-char selection marker for line-by-line mode.
fn line_mode_marker(ds: &DiffState, row: usize, theme: &Theme) -> Span<'static> {
    let is_selected = ds.selected_lines.contains(&row);
    let is_cursor = row == ds.cursor_line;
    let (mark, mark_color) = if ds.is_staged {
        ("-", theme.red)
    } else {
        ("+", theme.green)
    };
    match (is_cursor, is_selected) {
        (true, true) => Span::styled(
            ">>",
            Style::default()
                .fg(theme.orange)
                .add_modifier(Modifier::BOLD),
        ),
        (true, false) => Span::styled(
            "> ",
            Style::default()
                .fg(theme.orange)
                .add_modifier(Modifier::BOLD),
        ),
        (false, true) => Span::styled(
            format!(" {mark}"),
            Style::default().fg(mark_color).add_modifier(Modifier::BOLD),
        ),
        (false, false) => Span::styled("  ", Style::default()),
    }
}

fn format_file_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    }
}
