use crate::distance::{estimate_distance_smart, DistanceConfidence};
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

    // Center the popup - make it larger for capabilities
    let popup_width = 70.min(area.width.saturating_sub(4));
    let popup_height = 35.min(area.height.saturating_sub(4));
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

    // Get averaged signal if available
    let avg_rssi = device.rssi_tracker.weighted_average();
    let sample_count = device.rssi_tracker.sample_count();

    let signal_str = avg_rssi
        .or(device.last_signal)
        .map(|s| {
            if sample_count > 1 {
                format!("{} dBm (avg of {})", s, sample_count)
            } else {
                format!("{} dBm", s)
            }
        })
        .unwrap_or_else(|| "N/A".to_string());

    // Calculate distance with uncertainty using smart estimation
    let distance_estimate = avg_rssi.and_then(|rssi| {
        estimate_distance_smart(
            rssi,
            device.wifi_generation.as_deref(),
            3.0, // path loss exponent
            sample_count,
            None, // no calibration yet
        )
    });

    let distance_str = if let Some(est) = distance_estimate {
        let confidence_indicator = match est.confidence {
            DistanceConfidence::Low => "?",
            DistanceConfidence::Medium => "",
            DistanceConfidence::High => "",
        };
        format!("~{:.0}m ({:.0}-{:.0}m){}", est.center, est.min, est.max, confidence_indicator)
    } else {
        "N/A".to_string()
    };

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

    let mut content = vec![
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
    ];

    // Add capabilities section if available
    if let Some(caps) = &device.capabilities {
        content.push(Line::from(""));
        content.push(Line::from(Span::styled(
            "═══ WiFi Capabilities ═══",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )));

        // WiFi Generation
        if !caps.wifi_generation.is_empty() {
            content.push(Line::from(vec![
                Span::styled("Generation: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    &caps.wifi_generation,
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        // Max Rate
        if let Some(rate) = caps.max_rate_mbps {
            content.push(Line::from(vec![
                Span::styled("Max Rate: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{:.1} Mbps", rate)),
            ]));
        }

        // Supported Rates
        if !caps.supported_rates_mbps.is_empty() {
            let rates_str: Vec<String> = caps.supported_rates_mbps.iter()
                .map(|r| format!("{:.0}", r))
                .collect();
            content.push(Line::from(vec![
                Span::styled("Rates: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{} Mbps", rates_str.join(", "))),
            ]));
        }

        // HT Capabilities (802.11n)
        if let Some(ht) = &caps.ht_caps {
            content.push(Line::from(""));
            content.push(Line::from(Span::styled(
                "── HT (802.11n) ──",
                Style::default().fg(Color::Blue),
            )));
            content.push(Line::from(vec![
                Span::styled("40MHz: ", Style::default().fg(Color::Yellow)),
                Span::raw(if ht.channel_width_40mhz { "Yes" } else { "No" }),
                Span::raw("  "),
                Span::styled("Short GI: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("20:{} 40:{}",
                    if ht.short_gi_20 { "Y" } else { "N" },
                    if ht.short_gi_40 { "Y" } else { "N" }
                )),
            ]));
            content.push(Line::from(vec![
                Span::styled("TX STBC: ", Style::default().fg(Color::Yellow)),
                Span::raw(if ht.tx_stbc { "Yes" } else { "No" }),
                Span::raw("  "),
                Span::styled("RX STBC: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{}", ht.rx_stbc)),
            ]));
        }

        // VHT Capabilities (802.11ac)
        if let Some(vht) = &caps.vht_caps {
            content.push(Line::from(""));
            content.push(Line::from(Span::styled(
                "── VHT (802.11ac) ──",
                Style::default().fg(Color::Blue),
            )));
            let width_str = match vht.supported_channel_width {
                0 => "80 MHz",
                1 => "160 MHz",
                2 => "160 MHz + 80+80 MHz",
                _ => "Unknown",
            };
            content.push(Line::from(vec![
                Span::styled("Max Width: ", Style::default().fg(Color::Yellow)),
                Span::raw(width_str),
            ]));
            content.push(Line::from(vec![
                Span::styled("Beamforming: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("SU:{} MU:{}",
                    if vht.su_beamformer { "Y" } else { "N" },
                    if vht.mu_beamformer { "Y" } else { "N" }
                )),
            ]));
        }

        // RSN Security Info
        if let Some(rsn) = &caps.rsn_info {
            content.push(Line::from(""));
            content.push(Line::from(Span::styled(
                "── Security (RSN) ──",
                Style::default().fg(Color::Blue),
            )));
            content.push(Line::from(vec![
                Span::styled("Auth: ", Style::default().fg(Color::Yellow)),
                Span::raw(rsn.akm_suites.join(", ")),
            ]));
            content.push(Line::from(vec![
                Span::styled("Cipher: ", Style::default().fg(Color::Yellow)),
                Span::raw(rsn.pairwise_ciphers.join(", ")),
            ]));
            let mfp_str = match (rsn.mfp_required, rsn.mfp_capable) {
                (true, _) => "Required",
                (false, true) => "Capable",
                (false, false) => "No",
            };
            content.push(Line::from(vec![
                Span::styled("MFP: ", Style::default().fg(Color::Yellow)),
                Span::raw(mfp_str),
            ]));
        }

        // WPS Info
        if let Some(wps) = &caps.wps_info {
            content.push(Line::from(""));
            content.push(Line::from(Span::styled(
                "── WPS Device Info ──",
                Style::default().fg(Color::Blue),
            )));
            if !wps.device_name.is_empty() {
                content.push(Line::from(vec![
                    Span::styled("Name: ", Style::default().fg(Color::Yellow)),
                    Span::styled(&wps.device_name, Style::default().fg(Color::Green)),
                ]));
            }
            if !wps.manufacturer.is_empty() {
                content.push(Line::from(vec![
                    Span::styled("Manufacturer: ", Style::default().fg(Color::Yellow)),
                    Span::raw(&wps.manufacturer),
                ]));
            }
            if !wps.model.is_empty() {
                content.push(Line::from(vec![
                    Span::styled("Model: ", Style::default().fg(Color::Yellow)),
                    Span::raw(&wps.model),
                ]));
            }
            if !wps.model_number.is_empty() {
                content.push(Line::from(vec![
                    Span::styled("Model #: ", Style::default().fg(Color::Yellow)),
                    Span::raw(&wps.model_number),
                ]));
            }
        }

        // Vendor IEs
        if !caps.vendor_ies.is_empty() {
            content.push(Line::from(""));
            content.push(Line::from(Span::styled(
                "── Vendor IEs ──",
                Style::default().fg(Color::Blue),
            )));
            for vie in caps.vendor_ies.iter().take(5) {
                let vendor_name = vie.vendor_name.as_deref().unwrap_or("Unknown");
                content.push(Line::from(vec![
                    Span::styled(vendor_name, Style::default().fg(Color::Yellow)),
                    Span::raw(format!(" ({}) - {} bytes", vie.oui, vie.data_len)),
                ]));
            }
            if caps.vendor_ies.len() > 5 {
                content.push(Line::from(Span::styled(
                    format!("... and {} more", caps.vendor_ies.len() - 5),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
    }

    content.push(Line::from(""));
    content.push(Line::from(Span::styled(
        "Press ESC to close",
        Style::default().fg(Color::DarkGray),
    )));

    let popup = Paragraph::new(content).block(
        Block::default()
            .title(" Device Details ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(popup, popup_area);
}
