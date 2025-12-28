use crate::oui::{infer_device_type, is_randomized_mac, lookup_vendor};
use crate::tui::app::{ActivePanel, App};
use crate::tui::widgets::{
    device_table::render_device_table, help_overlay::render_help, probe_log::render_probe_log,
    stats_panel::render_stats, status_bar::render_status_bar,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Main draw function for the TUI
pub fn draw(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Main layout: Header, Content, Status Bar
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(10),    // Content
            Constraint::Length(3),  // Status bar
        ])
        .split(size);

    // Draw header
    draw_header(frame, main_chunks[0]);

    // Content layout: Top section (log + stats) and Bottom section (device table)
    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40), // Top: Log + Stats
            Constraint::Percentage(60), // Bottom: Device table
        ])
        .split(main_chunks[1]);

    // Top section: Probe log (70%) + Stats (30%)
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70), // Probe log
            Constraint::Percentage(30), // Stats
        ])
        .split(content_chunks[0]);

    // Draw probe log
    let log_focused = app.active_panel == ActivePanel::ProbeLog;
    render_probe_log(frame, top_chunks[0], app, log_focused);

    // Draw stats panel
    render_stats(frame, top_chunks[1], app);

    // Draw device table
    let table_focused = app.active_panel == ActivePanel::DeviceTable;
    render_device_table(frame, content_chunks[1], app, table_focused);

    // Draw status bar
    render_status_bar(frame, main_chunks[2], app);

    // Draw help overlay if active
    if app.show_help {
        render_help(frame, size);
    }

    // Draw device detail view if active
    if let Some(idx) = app.detail_view {
        if idx < app.devices.len() {
            draw_device_detail(frame, size, app, idx);
        }
    }
}

fn draw_header(frame: &mut Frame, area: Rect) {
    let title = vec![
        Span::styled(
            " PROWL ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            "Wi-Fi Probe Analyzer",
            Style::default().fg(Color::White),
        ),
        Span::raw("  "),
        Span::styled(
            "[?] Help  [q] Quit",
            Style::default().fg(Color::DarkGray),
        ),
    ];

    let header = Paragraph::new(Line::from(title))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

    frame.render_widget(header, area);
}

fn draw_device_detail(frame: &mut Frame, area: Rect, app: &App, idx: usize) {
    let device = &app.devices[idx];

    // Center the popup
    let popup_width = 60.min(area.width.saturating_sub(4));
    let popup_height = 18.min(area.height.saturating_sub(4));
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    // Format device info
    let first_seen = chrono::DateTime::from_timestamp(device.first_seen, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let last_seen = chrono::DateTime::from_timestamp(device.last_seen, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let signal_str = device
        .last_signal
        .map(|s| format!("{} dBm", s))
        .unwrap_or_else(|| "N/A".to_string());

    let distance_str = device
        .last_distance
        .map(|d| format!("{:.1}m", d))
        .unwrap_or_else(|| "N/A".to_string());

    let ssids_str = if device.ssids.is_empty() {
        "<broadcast only>".to_string()
    } else {
        device.ssids.join(", ")
    };

    // Vendor/device identification
    let vendor = lookup_vendor(&device.mac);
    let vendor_str = vendor.unwrap_or("Unknown");
    let device_type = infer_device_type(&device.mac, vendor);
    let is_random = is_randomized_mac(&device.mac);
    let mac_type = if is_random { "Randomized" } else { "Real" };

    let content = vec![
        Line::from(vec![
            Span::styled("MAC: ", Style::default().fg(Color::Yellow)),
            Span::raw(&device.mac),
            Span::styled(
                format!(" ({})", mac_type),
                Style::default().fg(if is_random { Color::Magenta } else { Color::Green }),
            ),
        ]),
        Line::from(vec![
            Span::styled("Vendor: ", Style::default().fg(Color::Yellow)),
            Span::raw(vendor_str),
        ]),
        Line::from(vec![
            Span::styled("Type: ", Style::default().fg(Color::Yellow)),
            Span::raw(device_type),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("First Seen: ", Style::default().fg(Color::Yellow)),
            Span::raw(first_seen),
        ]),
        Line::from(vec![
            Span::styled("Last Seen:  ", Style::default().fg(Color::Yellow)),
            Span::raw(last_seen),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Probes: ", Style::default().fg(Color::Yellow)),
            Span::raw(device.probe_count.to_string()),
        ]),
        Line::from(vec![
            Span::styled("Signal: ", Style::default().fg(Color::Yellow)),
            Span::raw(signal_str),
        ]),
        Line::from(vec![
            Span::styled("Distance: ", Style::default().fg(Color::Yellow)),
            Span::raw(distance_str),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("SSIDs: ", Style::default().fg(Color::Yellow)),
            Span::raw(ssids_str),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press ESC to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let popup = Paragraph::new(content).block(
        Block::default()
            .title(" Device Details ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(popup, popup_area);
}
