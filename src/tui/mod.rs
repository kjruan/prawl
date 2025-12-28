pub mod app;
pub mod event;
pub mod ui;
pub mod widgets;

use crate::channels::{find_monitor_interface, is_monitor_mode, set_monitor_mode, ChannelHopper};
use crate::config::Config;
use crate::database::{Database, ProbeCapture};
use crate::distance::estimate_distance;
use crate::gps::GpsClient;
use crate::ignore::IgnoreLists;
use crate::parser::parse_probe_request;
use anyhow::{Context, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::{warn, LevelFilter};
use pcap::Capture;
use ratatui::prelude::*;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

pub use app::{App, DeviceEntry, ProbeLogEntry, Stats};

/// Events sent from capture/gps tasks to the TUI
#[derive(Debug, Clone)]
pub enum TuiEvent {
    ProbeReceived(ProbeLogEntry),
    GpsUpdate(f64, f64),
    GpsDisconnected,
    ChannelChanged(u8),
    StatsUpdate(Stats),
    CaptureStarted,
    CaptureStopped,
    Error(String),
}

/// Setup terminal for TUI mode
fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore terminal to normal mode
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

/// Run the TUI application
pub async fn run_tui(mut config: Config, set_monitor: bool) -> Result<()> {
    // Try to auto-detect monitor interface if configured one isn't in monitor mode
    let _interface = if set_monitor {
        set_monitor_mode(&config.capture.interface)?;
        config.capture.interface.clone()
    } else if is_monitor_mode(&config.capture.interface)? {
        config.capture.interface.clone()
    } else if let Some(found) = find_monitor_interface()? {
        eprintln!("Auto-detected monitor interface: {}", found);
        config.capture.interface = found.clone();
        found
    } else {
        eprintln!(
            "Interface {} is not in monitor mode and no monitor interface found.",
            config.capture.interface
        );
        eprintln!("Use --set-monitor or run 'prowl scan' to find interfaces.");
        return Ok(());
    };

    // Disable logging to prevent interference with TUI display
    log::set_max_level(LevelFilter::Off);

    // Create event channel
    let (event_tx, event_rx) = mpsc::channel::<TuiEvent>(1000);

    // Open database
    let db = Database::open(&config.capture.database).context("Failed to open database")?;

    // Load ignore lists
    let ignore_lists =
        IgnoreLists::load(&config.ignore_lists.mac, &config.ignore_lists.ssid).unwrap_or_default();

    // Get initial stats
    let initial_stats = Stats {
        total_devices: db.count_devices().unwrap_or(0),
        total_probes: db.count_probes().unwrap_or(0),
        ..Default::default()
    };

    // Create running flag
    let running = Arc::new(AtomicBool::new(true));

    // Setup panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic);
    }));

    // Spawn capture task
    let capture_tx = event_tx.clone();
    let capture_running = running.clone();
    let capture_config = config.clone();
    let capture_db = Database::open(&config.capture.database)?;
    let capture_ignore = ignore_lists.clone();

    let capture_handle = tokio::spawn(async move {
        run_capture_loop(
            capture_config,
            capture_db,
            capture_ignore,
            capture_running,
            capture_tx,
        )
        .await
    });

    // Spawn GPS task if enabled
    if config.gps.enabled {
        let gps_tx = event_tx.clone();
        let gps_running = running.clone();
        let gps_host = config.gps.host.clone();
        let gps_port = config.gps.port;

        tokio::spawn(async move {
            let gps_client = GpsClient::new(gps_host, gps_port);
            let (pos_tx, mut pos_rx) = mpsc::channel(1);

            let gps_run = gps_running.clone();
            tokio::spawn(async move {
                let _ = gps_client.run(pos_tx, gps_run).await;
            });

            while gps_running.load(Ordering::SeqCst) {
                tokio::select! {
                    Some((lat, lon)) = pos_rx.recv() => {
                        let _ = gps_tx.send(TuiEvent::GpsUpdate(lat, lon)).await;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {
                        // Timeout, continue
                    }
                }
            }
        });
    }

    // Spawn stats refresh task
    let stats_tx = event_tx.clone();
    let stats_running = running.clone();
    let stats_db_path = config.capture.database.clone();
    let start_time = Instant::now();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        while stats_running.load(Ordering::SeqCst) {
            interval.tick().await;

            if let Ok(db) = Database::open(&stats_db_path) {
                let now = chrono::Utc::now().timestamp();
                let five_min_ago = now - 300;
                let fifteen_min_ago = now - 900;

                let stats = Stats {
                    total_devices: db.count_devices().unwrap_or(0),
                    total_probes: db.count_probes().unwrap_or(0),
                    devices_last_5min: db
                        .get_devices_in_time_range(five_min_ago, now)
                        .map(|d| d.len())
                        .unwrap_or(0),
                    devices_last_15min: db
                        .get_devices_in_time_range(fifteen_min_ago, now)
                        .map(|d| d.len())
                        .unwrap_or(0),
                    capture_duration_secs: start_time.elapsed().as_secs(),
                    ..Default::default()
                };

                let _ = stats_tx.send(TuiEvent::StatsUpdate(stats)).await;
            }
        }
    });

    // Create app
    let mut app = App::new(event_rx, initial_stats);

    // Setup terminal
    let mut terminal = setup_terminal()?;

    // Run event loop
    let tick_rate = Duration::from_millis(50); // 20 FPS for efficiency

    let result = run_event_loop(&mut terminal, &mut app, tick_rate, running.clone()).await;

    // Cleanup
    running.store(false, Ordering::SeqCst);
    capture_handle.abort();
    restore_terminal(&mut terminal)?;

    result
}

async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    tick_rate: Duration,
    running: Arc<AtomicBool>,
) -> Result<()> {
    let mut last_tick = Instant::now();

    loop {
        // Draw UI
        terminal.draw(|f| ui::draw(f, app))?;

        // Calculate timeout
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());

        // Poll for events
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = crossterm::event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => {
                            app.running = false;
                        }
                        KeyCode::Char('?') => {
                            app.show_help = !app.show_help;
                        }
                        KeyCode::Tab | KeyCode::Right => {
                            app.next_panel();
                        }
                        KeyCode::BackTab | KeyCode::Left => {
                            app.prev_panel();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.scroll_down();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.scroll_up();
                        }
                        KeyCode::Char('s') => {
                            app.cycle_sort();
                        }
                        KeyCode::Char('r') => {
                            app.reverse_sort();
                        }
                        KeyCode::Enter => {
                            app.select_device();
                        }
                        KeyCode::Esc => {
                            if app.show_help {
                                app.show_help = false;
                            } else if app.detail_view.is_some() {
                                app.detail_view = None;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Process TUI events from background tasks
        while let Ok(tui_event) = app.event_rx.try_recv() {
            app.handle_event(tui_event);
        }

        // Tick
        if last_tick.elapsed() >= tick_rate {
            app.tick();
            last_tick = Instant::now();
        }

        if !app.running {
            running.store(false, Ordering::SeqCst);
            break;
        }
    }

    Ok(())
}

/// Capture loop that sends events to TUI
async fn run_capture_loop(
    config: Config,
    db: Database,
    ignore_lists: IgnoreLists,
    running: Arc<AtomicBool>,
    event_tx: mpsc::Sender<TuiEvent>,
) -> Result<()> {
    let interface = &config.capture.interface;

    // Open capture handle
    let mut cap = Capture::from_device(interface.as_str())
        .context("Failed to open capture device")?
        .promisc(true)
        .snaplen(65535)
        .timeout(100)
        .open()
        .context("Failed to activate capture")?;

    // Set BPF filter
    if let Err(e) = cap.filter("type mgt subtype probe-req", true) {
        warn!("Failed to set BPF filter: {}", e);
    }

    // Start channel hopper
    let hopper = ChannelHopper::new(
        interface.clone(),
        config.capture.channels.clone(),
        config.capture.hop_interval_ms,
    );
    let hopper_running = running.clone();
    let hopper_channels = hopper.channels().to_vec();
    let hopper_interval = hopper.hop_interval_ms();
    tokio::spawn(async move {
        let _ = hopper.run(hopper_running.clone()).await;
    });

    // Send channel change events
    let channel_tx = event_tx.clone();
    let channel_running = running.clone();
    tokio::spawn(async move {
        let mut idx = 0;
        while channel_running.load(Ordering::SeqCst) {
            if !hopper_channels.is_empty() {
                let ch = hopper_channels[idx % hopper_channels.len()];
                let _ = channel_tx.send(TuiEvent::ChannelChanged(ch)).await;
                idx += 1;
            }
            tokio::time::sleep(Duration::from_millis(hopper_interval)).await;
        }
    });

    let _ = event_tx.send(TuiEvent::CaptureStarted).await;

    while running.load(Ordering::SeqCst) {
        match cap.next_packet() {
            Ok(packet) => {
                // Extract signal from radiotap
                let signal_dbm = extract_signal_dbm(packet.data);

                if let Some(probe) = parse_probe_request(packet.data, signal_dbm) {
                    // Check ignore lists
                    if ignore_lists.should_ignore_mac(&probe.source_mac) {
                        continue;
                    }
                    if !probe.ssid.is_empty() && ignore_lists.should_ignore_ssid(&probe.ssid) {
                        continue;
                    }

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64;

                    // Calculate distance
                    let distance_m = if config.distance.enabled {
                        probe.signal_dbm.and_then(|rssi| {
                            estimate_distance(
                                rssi,
                                config.distance.tx_power_dbm,
                                config.distance.path_loss_exponent,
                            )
                        })
                    } else {
                        None
                    };

                    // Insert into database
                    let capture = ProbeCapture {
                        mac: probe.source_mac.clone(),
                        ssid: probe.ssid.clone(),
                        timestamp: now,
                        lat: None,
                        lon: None,
                        signal_dbm: probe.signal_dbm,
                        channel: None,
                        distance_m,
                    };

                    let _ = db.insert_probe(&capture);

                    // Send to TUI
                    let log_entry = ProbeLogEntry {
                        timestamp: now,
                        mac: probe.source_mac,
                        ssid: probe.ssid,
                        signal_dbm: probe.signal_dbm,
                        distance_m,
                        channel: None,
                    };

                    let _ = event_tx.send(TuiEvent::ProbeReceived(log_entry)).await;
                }
            }
            Err(pcap::Error::TimeoutExpired) => {
                // Normal timeout, use async yield
                tokio::task::yield_now().await;
            }
            Err(e) => {
                if running.load(Ordering::SeqCst) {
                    let _ = event_tx
                        .send(TuiEvent::Error(format!("Capture error: {}", e)))
                        .await;
                }
                break;
            }
        }
    }

    let _ = event_tx.send(TuiEvent::CaptureStopped).await;
    Ok(())
}

fn extract_signal_dbm(data: &[u8]) -> Option<i32> {
    if data.len() < 8 || data[0] != 0 {
        return None;
    }

    let radiotap_len = u16::from_le_bytes([data[2], data[3]]) as usize;
    if radiotap_len > data.len() || radiotap_len < 8 {
        return None;
    }

    let present = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    if present & (1 << 5) == 0 {
        return None;
    }

    let mut offset = 8;

    if present & (1 << 0) != 0 {
        offset = (offset + 7) & !7;
        offset += 8;
    }

    if present & (1 << 1) != 0 {
        offset += 1;
    }

    if present & (1 << 2) != 0 {
        offset += 1;
    }

    if present & (1 << 3) != 0 {
        offset = (offset + 1) & !1;
        offset += 4;
    }

    if present & (1 << 4) != 0 {
        offset += 2;
    }

    if offset < radiotap_len {
        let signal = data[offset] as i8;
        return Some(signal as i32);
    }

    None
}
