use crate::oui::{is_randomized_mac, vendor_short};
use crate::tui::app::{App, DeviceSortField};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
    Frame,
};

/// Render the device table panel
pub fn render_device_table(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let border_color = if focused { Color::Cyan } else { Color::DarkGray };

    // Build title with sort indicator
    let sort_indicator = match app.sort_field {
        DeviceSortField::Mac => "MAC",
        DeviceSortField::LastSeen => "Last Seen",
        DeviceSortField::ProbeCount => "Probes",
        DeviceSortField::Signal => "Signal",
    };
    let sort_arrow = if app.sort_ascending { "▲" } else { "▼" };
    let title = format!(" Devices [Sort: {} {}] [s]ort [r]everse ", sort_indicator, sort_arrow);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    // Table header
    let header_cells = ["MAC Address", "Vendor", "Last Seen", "Probes", "Signal", "Distance", "SSIDs"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1);

    // Table rows
    let rows: Vec<Row> = app
        .devices
        .iter()
        .enumerate()
        .map(|(idx, device)| {
            let last_seen = chrono::DateTime::from_timestamp(device.last_seen, 0)
                .map(|dt| dt.format("%H:%M:%S").to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            let signal_str = device
                .last_signal
                .map(|s| format!("{}dBm", s))
                .unwrap_or_else(|| "N/A".to_string());

            let distance_str = device
                .last_distance
                .map(|d| format!("{:.1}m", d))
                .unwrap_or_else(|| "N/A".to_string());

            let ssids_str = if device.ssids.is_empty() {
                "<broadcast>".to_string()
            } else if device.ssids.len() == 1 {
                truncate_str(&device.ssids[0], 15)
            } else {
                format!("{} (+{})", truncate_str(&device.ssids[0], 10), device.ssids.len() - 1)
            };

            // Color code signal
            let signal_color = device.last_signal.map(|s| {
                if s >= -50 {
                    Color::Green
                } else if s >= -70 {
                    Color::Yellow
                } else {
                    Color::Red
                }
            }).unwrap_or(Color::DarkGray);

            // Color code distance (closer = more concerning)
            let distance_color = device.last_distance.map(|d| {
                if d < 3.0 {
                    Color::Red
                } else if d < 10.0 {
                    Color::Yellow
                } else {
                    Color::Green
                }
            }).unwrap_or(Color::DarkGray);

            // Get vendor info
            let vendor = vendor_short(&device.mac);
            let is_random = is_randomized_mac(&device.mac);
            let vendor_color = if is_random {
                Color::Magenta  // Randomized MACs in magenta
            } else if vendor == "UNK" {
                Color::DarkGray
            } else {
                Color::Green
            };

            let cells = vec![
                Cell::from(device.mac.clone()),
                Cell::from(vendor).style(Style::default().fg(vendor_color)),
                Cell::from(last_seen),
                Cell::from(device.probe_count.to_string()),
                Cell::from(signal_str).style(Style::default().fg(signal_color)),
                Cell::from(distance_str).style(Style::default().fg(distance_color)),
                Cell::from(ssids_str).style(Style::default().fg(Color::Cyan)),
            ];

            let style = if idx == app.selected_device && focused {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(cells).style(style).height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(17),  // MAC
        Constraint::Length(6),   // Vendor
        Constraint::Length(10),  // Last Seen
        Constraint::Length(7),   // Probes
        Constraint::Length(8),   // Signal
        Constraint::Length(9),   // Distance
        Constraint::Min(10),     // SSIDs (flexible)
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
        );

    // Create table state for selection
    let mut state = TableState::default();
    if !app.devices.is_empty() {
        state.select(Some(app.selected_device));
    }

    frame.render_stateful_widget(table, area, &mut state);

    // Show device count
    let count_str = format!(" {} devices ", app.devices.len());
    let count_len = count_str.len() as u16;
    let count_x = area.x + area.width.saturating_sub(count_len + 2);
    let count_y = area.y;

    if count_x > area.x {
        frame.render_widget(
            ratatui::widgets::Paragraph::new(count_str)
                .style(Style::default().fg(Color::DarkGray)),
            Rect::new(count_x, count_y, count_len, 1),
        );
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
