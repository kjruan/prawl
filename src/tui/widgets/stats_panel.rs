use crate::tui::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render the statistics panel
pub fn render_stats(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Statistics ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let duration_str = format_duration(app.stats.capture_duration_secs);

    let lines = vec![
        Line::from(vec![
            Span::styled("Devices:  ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{:>6}", app.stats.total_devices),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Probes:   ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{:>6}", app.stats.total_probes),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("Rate:     ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{:>5.1}/min", app.stats.probes_per_minute),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Last 5m:  ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{:>6}", app.stats.devices_last_5min),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("Last 15m: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{:>6}", app.stats.devices_last_15min),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Uptime:   ", Style::default().fg(Color::Yellow)),
            Span::styled(duration_str, Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(block);

    frame.render_widget(paragraph, area);
}

fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
