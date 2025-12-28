use crate::oui::{is_randomized_mac, vendor_short};
use crate::tui::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

/// Render the probe log panel
pub fn render_probe_log(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let border_color = if focused { Color::Cyan } else { Color::DarkGray };

    let block = Block::default()
        .title(" Probe Log (Live) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    // Get visible entries (most recent first, but display oldest at top)
    let inner_height = area.height.saturating_sub(2) as usize;
    let total_entries = app.probe_log.len();

    // Calculate which entries to show
    let start_idx = app.log_scroll;
    let entries: Vec<&_> = app
        .probe_log
        .iter()
        .rev()
        .skip(start_idx)
        .take(inner_height)
        .collect();

    let items: Vec<ListItem> = entries
        .iter()
        .rev()
        .map(|entry| {
            let timestamp = chrono::DateTime::from_timestamp(entry.timestamp, 0)
                .map(|dt| dt.format("%H:%M:%S").to_string())
                .unwrap_or_else(|| "??:??:??".to_string());

            let ssid_display = if entry.ssid.is_empty() {
                "<broadcast>"
            } else {
                &entry.ssid
            };

            let signal_str = entry
                .signal_dbm
                .map(|s| format!("{:>4}dBm", s))
                .unwrap_or_else(|| "    N/A".to_string());

            let distance_str = entry
                .distance_m
                .map(|d| format!("{:>5.1}m", d))
                .unwrap_or_else(|| "    N/A".to_string());

            // Color code signal strength
            let signal_color = entry.signal_dbm.map(|s| {
                if s >= -50 {
                    Color::Green
                } else if s >= -70 {
                    Color::Yellow
                } else {
                    Color::Red
                }
            }).unwrap_or(Color::DarkGray);

            // Color code distance
            let distance_color = entry.distance_m.map(|d| {
                if d < 3.0 {
                    Color::Red
                } else if d < 10.0 {
                    Color::Yellow
                } else {
                    Color::Green
                }
            }).unwrap_or(Color::DarkGray);

            // Get vendor info
            let vendor = vendor_short(&entry.mac);
            let is_random = is_randomized_mac(&entry.mac);
            let vendor_color = if is_random {
                Color::Magenta  // Randomized MACs in magenta
            } else if vendor == "UNK" {
                Color::DarkGray
            } else {
                Color::Green
            };

            let spans = vec![
                Span::styled(
                    format!("[{}] ", timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{:<17} ", entry.mac),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!("{:<4} ", vendor),
                    Style::default().fg(vendor_color),
                ),
                Span::styled(signal_str, Style::default().fg(signal_color)),
                Span::raw(" "),
                Span::styled(distance_str, Style::default().fg(distance_color)),
                Span::raw("  "),
                Span::styled(
                    truncate_str(ssid_display, 16),
                    Style::default().fg(Color::Cyan),
                ),
            ];

            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items).block(block);

    frame.render_widget(list, area);

    // Show scroll indicator if there are more entries
    if total_entries > inner_height {
        let scroll_info = format!(
            " {}/{} ",
            total_entries.saturating_sub(start_idx).min(total_entries),
            total_entries
        );
        let scroll_len = scroll_info.len() as u16;
        let scroll_x = area.x + area.width.saturating_sub(scroll_len + 2);
        let scroll_y = area.y;

        if scroll_x > area.x && scroll_y < area.y + area.height {
            frame.render_widget(
                ratatui::widgets::Paragraph::new(scroll_info)
                    .style(Style::default().fg(Color::DarkGray)),
                Rect::new(scroll_x, scroll_y, scroll_len, 1),
            );
        }
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
