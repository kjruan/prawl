use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Render the help overlay
pub fn render_help(frame: &mut Frame, area: Rect) {
    // Center the help popup
    let popup_width = 50.min(area.width.saturating_sub(4));
    let popup_height = 18.min(area.height.saturating_sub(4));
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Tab / ← →    ", Style::default().fg(Color::Yellow)),
            Span::raw("Switch panels"),
        ]),
        Line::from(vec![
            Span::styled("  ↑ ↓ / j k    ", Style::default().fg(Color::Yellow)),
            Span::raw("Scroll / Select"),
        ]),
        Line::from(vec![
            Span::styled("  Enter        ", Style::default().fg(Color::Yellow)),
            Span::raw("View device details"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  s            ", Style::default().fg(Color::Yellow)),
            Span::raw("Cycle sort field"),
        ]),
        Line::from(vec![
            Span::styled("  r            ", Style::default().fg(Color::Yellow)),
            Span::raw("Reverse sort order"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ?            ", Style::default().fg(Color::Yellow)),
            Span::raw("Toggle this help"),
        ]),
        Line::from(vec![
            Span::styled("  Esc          ", Style::default().fg(Color::Yellow)),
            Span::raw("Close popup/overlay"),
        ]),
        Line::from(vec![
            Span::styled("  q            ", Style::default().fg(Color::Yellow)),
            Span::raw("Quit application"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press ? or Esc to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let help_popup = Paragraph::new(help_text).block(
        Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(help_popup, popup_area);
}
