use crate::tui::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render the status bar at the bottom
pub fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    // GPS status
    let gps_status = if let Some((lat, lon)) = app.gps_position {
        Span::styled(
            format!("GPS: {:.4}, {:.4}", lat, lon),
            Style::default().fg(Color::Green),
        )
    } else if app.gps_connected {
        Span::styled("GPS: Connecting...", Style::default().fg(Color::Yellow))
    } else {
        Span::styled("GPS: Disabled", Style::default().fg(Color::DarkGray))
    };

    // Channel status
    let channel_status = if let Some(ch) = app.current_channel {
        Span::styled(format!("Ch: {}", ch), Style::default().fg(Color::Cyan))
    } else {
        Span::styled("Ch: --", Style::default().fg(Color::DarkGray))
    };

    // Capture status
    let capture_status = if app.capture_active {
        Span::styled(
            "Capture: ACTIVE",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            "Capture: STOPPED",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )
    };

    // Uptime
    let uptime = format_duration(app.stats.capture_duration_secs);
    let uptime_status = Span::styled(
        format!("Uptime: {}", uptime),
        Style::default().fg(Color::DarkGray),
    );

    let status_line = Line::from(vec![
        Span::raw(" "),
        gps_status,
        Span::raw("  │  "),
        channel_status,
        Span::raw("  │  "),
        capture_status,
        Span::raw("  │  "),
        uptime_status,
    ]);

    let paragraph = Paragraph::new(status_line).block(block);

    frame.render_widget(paragraph, area);
}

fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
