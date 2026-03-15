mod diff_view;
mod file_panel;
mod overview_bar;
mod popup;
mod status_bar;

use crate::app::App;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::widgets::{Block, Clear};
use ratatui::Frame;

pub fn render(app: &App, frame: &mut Frame) {
    // Clear entire frame to avoid stale overlay artifacts
    frame.render_widget(Clear, frame.area());
    // Fill background with theme color
    frame.render_widget(
        Block::default().style(Style::default().bg(app.theme.bg)),
        frame.area(),
    );
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let [file_area, diff_area] =
        Layout::horizontal([Constraint::Length(30), Constraint::Fill(1)]).areas(body);

    status_bar::render_header(app, frame, header);
    file_panel::render(app, frame, file_area);

    if app.conflict_state.is_some() {
        // Conflict resolver gets the full diff area
        diff_view::render_right(app, frame, diff_area);
    } else {
        let [diff_left, diff_right] =
            Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).areas(diff_area);
        diff_view::render_left(app, frame, diff_left);
        diff_view::render_right(app, frame, diff_right);
    }

    status_bar::render_footer(app, frame, footer);

    // Render overlay on top
    popup::render(app, frame);
}
