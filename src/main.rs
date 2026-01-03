use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use log::{error, info, warn, LevelFilter};
use pcap::Capture;
use prowl::analysis::SurveillanceAnalyzer;
use prowl::capture::CaptureEngine;
use prowl::channels::{
    find_monitor_interface, is_monitor_mode, list_wireless_interfaces, set_monitor_mode,
};
use prowl::config::Config;
use prowl::database::Database;
use prowl::distance::calibrate_tx_power;
use prowl::ignore::{create_default_ignore_lists, IgnoreLists};
use prowl::report::ReportGenerator;
use prowl::tui;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(name = "prowl")]
#[command(author = "spikehead")]
#[command(version = "0.1.0")]
#[command(about = "Wi-Fi probe request analyzer for surveillance detection")]
struct Cli {
    /// Config file path
    #[arg(short, long, default_value = "config.json")]
    config: PathBuf,

    /// Wi-Fi interface (overrides config)
    #[arg(short, long)]
    interface: Option<String>,

    /// Database file (overrides config)
    #[arg(short, long)]
    database: Option<PathBuf>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start capturing probe requests
    Capture {
        /// Set interface to monitor mode before capture
        #[arg(long)]
        set_monitor: bool,
    },

    /// Analyze captured data for surveillance patterns
    Analyze {
        /// Number of hours to analyze
        #[arg(long, default_value = "2")]
        last_hours: u32,

        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Generate reports from database
    Report {
        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Report type
        #[arg(long, default_value = "devices")]
        report_type: String,
    },

    /// List captured devices and probes
    List {
        /// Show only devices from last N hours
        #[arg(long)]
        last_hours: Option<u32>,

        /// Show detailed probe information
        #[arg(long)]
        detailed: bool,
    },

    /// Show database statistics
    Stats,

    /// Initialize configuration and ignore lists
    Init,

    /// Direct SQLite database access
    Db {
        #[command(subcommand)]
        action: DbCommands,
    },

    /// Start interactive TUI dashboard with live capture
    Tui {
        /// Set interface to monitor mode before capture
        #[arg(long)]
        set_monitor: bool,
    },

    /// Scan for wireless interfaces
    Scan,

    /// Calibrate distance estimation by capturing at known distance
    Calibrate {
        /// Known distance in meters to the probe source
        #[arg(long)]
        distance: f64,

        /// Duration to capture samples (seconds)
        #[arg(short = 't', long, default_value = "30")]
        duration: u64,

        /// Set interface to monitor mode before capture
        #[arg(long)]
        set_monitor: bool,
    },
}

#[derive(Subcommand)]
enum DbCommands {
    /// Execute a SQL query
    Query {
        /// SQL query to execute
        sql: String,
    },

    /// Show database schema
    Schema,

    /// Export data to CSV
    Export {
        /// Table to export (devices or probes)
        #[arg(default_value = "probes")]
        table: String,

        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Import from another SQLite database
    Import {
        /// Source database file
        source: PathBuf,
    },

    /// Vacuum/optimize the database
    Vacuum,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    env_logger::Builder::new()
        .filter_level(log_level)
        .format_timestamp_secs()
        .init();

    // Handle init command before loading config
    if matches!(cli.command, Commands::Init) {
        return handle_init();
    }

    // Load configuration
    let mut config = if cli.config.exists() {
        Config::load(&cli.config).context("Failed to load config")?
    } else {
        info!("Config file not found, using defaults");
        Config::default()
    };

    // Override config with CLI args
    if let Some(interface) = cli.interface {
        config.capture.interface = interface;
    }
    if let Some(database) = cli.database {
        config.capture.database = database.to_string_lossy().to_string();
    }

    // Execute command
    match cli.command {
        Commands::Capture { set_monitor } => handle_capture(config, set_monitor).await,
        Commands::Analyze { last_hours, output } => handle_analyze(config, last_hours, output),
        Commands::Report {
            output,
            report_type,
        } => handle_report(config, output, report_type),
        Commands::List {
            last_hours,
            detailed,
        } => handle_list(config, last_hours, detailed),
        Commands::Stats => handle_stats(config),
        Commands::Init => unreachable!(),
        Commands::Db { action } => handle_db(config, action),
        Commands::Tui { set_monitor } => tui::run_tui(config, set_monitor).await,
        Commands::Scan => handle_scan(),
        Commands::Calibrate {
            distance,
            duration,
            set_monitor,
        } => handle_calibrate(config, distance, duration, set_monitor).await,
    }
}

fn handle_scan() -> Result<()> {
    println!("Scanning for wireless interfaces...\n");

    let interfaces = list_wireless_interfaces()?;

    if interfaces.is_empty() {
        println!("No wireless interfaces found.");
        println!("\nMake sure you have a wireless adapter connected.");
        return Ok(());
    }

    let mut found_monitor = false;

    for (iface, mode) in &interfaces {
        if mode == "monitor" {
            println!("\x1b[32m[MONITOR]\x1b[0m {}", iface);
            found_monitor = true;
        } else {
            println!("\x1b[33m[{}]\x1b[0m {}", mode, iface);
        }
    }

    println!();

    if found_monitor {
        if let Ok(Some(iface)) = find_monitor_interface() {
            println!("\x1b[32mMonitor interface found: {}\x1b[0m", iface);
            println!("\nStart capturing with:");
            println!("  sudo prowl capture");
            println!("  sudo prowl tui");
        }
    } else {
        println!("\x1b[33mNo monitor mode interfaces found.\x1b[0m");
        println!("\nTo enable monitor mode:");
        println!("  sudo prowl capture --set-monitor");
        println!("  sudo prowl tui --set-monitor");
    }

    Ok(())
}

async fn handle_calibrate(
    mut config: Config,
    known_distance: f64, // in meters
    duration_secs: u64,
    set_monitor: bool,
) -> Result<()> {
    println!("=== Distance Calibration Mode ===");
    println!();
    println!("Instructions:");
    println!(
        "1. Position a device at exactly {:.1}m from this sensor",
        known_distance
    );
    println!("2. Make sure the device is actively sending probe requests");
    println!("   (e.g., with WiFi on but not connected, or scanning for networks)");
    println!(
        "3. Keep the device stationary for {} seconds",
        duration_secs
    );
    println!();

    // Set up interface
    let interface = if set_monitor {
        set_monitor_mode(&config.capture.interface)?;
        config.capture.interface.clone()
    } else if is_monitor_mode(&config.capture.interface)? {
        config.capture.interface.clone()
    } else if let Some(found) = find_monitor_interface()? {
        info!("Auto-detected monitor interface: {}", found);
        config.capture.interface = found.clone();
        found
    } else {
        error!(
            "Interface {} is not in monitor mode and no monitor interface found.",
            config.capture.interface
        );
        error!("Use --set-monitor to enable monitor mode.");
        return Ok(());
    };

    println!("Using interface: {}", interface);
    println!("Capturing for {} seconds...", duration_secs);
    println!();

    // Open capture
    let mut cap = Capture::from_device(interface.as_str())
        .context("Failed to open capture device")?
        .promisc(true)
        .snaplen(65535)
        .timeout(100)
        .open()
        .context("Failed to activate capture")?;

    if let Err(e) = cap.filter("type mgt subtype probe-req", true) {
        warn!("Failed to set BPF filter: {}", e);
    }

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        eprintln!("\nStopping calibration...");
        r.store(false, Ordering::SeqCst);
    })?;

    let start = Instant::now();
    let duration = Duration::from_secs(duration_secs);
    let mut rssi_samples: Vec<i32> = Vec::new();

    while running.load(Ordering::SeqCst) && start.elapsed() < duration {
        match cap.next_packet() {
            Ok(packet) => {
                if let Some(signal) = extract_signal_for_calibration(packet.data) {
                    rssi_samples.push(signal);
                    print!(
                        "\rSamples collected: {} (avg: {:.1} dBm)    ",
                        rssi_samples.len(),
                        rssi_samples.iter().sum::<i32>() as f64 / rssi_samples.len() as f64
                    );
                    std::io::Write::flush(&mut std::io::stdout())?;
                }
            }
            Err(pcap::Error::TimeoutExpired) => continue,
            Err(e) => {
                error!("Capture error: {}", e);
                break;
            }
        }
    }

    println!();
    println!();

    if rssi_samples.is_empty() {
        error!("No probe requests captured!");
        error!("Make sure:");
        error!("  - A device is sending probe requests (WiFi on, scanning)");
        error!("  - The interface is in monitor mode");
        return Ok(());
    }

    // Calculate calibration
    let avg_rssi = rssi_samples.iter().sum::<i32>() / rssi_samples.len() as i32;
    let min_rssi = *rssi_samples.iter().min().unwrap();
    let max_rssi = *rssi_samples.iter().max().unwrap();

    println!("=== Calibration Results ===");
    println!("Samples collected: {}", rssi_samples.len());
    println!("RSSI range: {} to {} dBm", min_rssi, max_rssi);
    println!("Average RSSI: {} dBm", avg_rssi);
    println!();

    if let Some(result) =
        calibrate_tx_power(avg_rssi, known_distance, config.distance.path_loss_exponent)
    {
        println!("Calculated TX Power: {:.1} dBm", result.calculated_tx_power);
        println!();

        // Update config
        config.distance.calibrated_tx_power = Some(result.calculated_tx_power);
        config.distance.calibration_distance_m = Some(known_distance);
        config.distance.calibrated_at = Some(chrono::Utc::now().to_rfc3339());

        // Save config
        if let Err(e) = config.save("config.json") {
            error!("Failed to save config: {}", e);
            println!("You can manually add to config.json:");
            println!(
                "  \"calibrated_tx_power\": {:.1}",
                result.calculated_tx_power
            );
        } else {
            println!("Configuration saved to config.json");
            println!();
            println!("Distance estimation will now use the calibrated value.");
        }
    } else {
        error!("Calibration failed - invalid input values");
    }

    Ok(())
}

fn extract_signal_for_calibration(data: &[u8]) -> Option<i32> {
    // Basic radiotap signal extraction
    if data.len() < 8 || data[0] != 0 {
        return None;
    }

    let radiotap_len = u16::from_le_bytes([data[2], data[3]]) as usize;
    if radiotap_len > data.len() || radiotap_len < 8 {
        return None;
    }

    let mut present_words: Vec<u32> = Vec::new();
    let mut pos = 4;
    loop {
        if pos + 4 > data.len() {
            return None;
        }
        let present = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        present_words.push(present);
        pos += 4;
        if present & (1 << 31) == 0 {
            break;
        }
    }

    let first_present = present_words[0];

    if first_present & (1 << 5) == 0 {
        return None;
    }

    let mut offset = pos;

    if first_present & (1 << 0) != 0 {
        offset = (offset + 7) & !7;
        offset += 8;
    }
    if first_present & (1 << 1) != 0 {
        offset += 1;
    }
    if first_present & (1 << 2) != 0 {
        offset += 1;
    }
    if first_present & (1 << 3) != 0 {
        offset = (offset + 1) & !1;
        offset += 4;
    }
    if first_present & (1 << 4) != 0 {
        offset += 2;
    }

    if offset < radiotap_len {
        let signal = data[offset] as i8;
        return Some(signal as i32);
    }

    None
}

async fn handle_capture(mut config: Config, set_monitor: bool) -> Result<()> {
    // Try to auto-detect monitor interface if configured one isn't in monitor mode
    let interface = if set_monitor {
        set_monitor_mode(&config.capture.interface)?;
        config.capture.interface.clone()
    } else if is_monitor_mode(&config.capture.interface)? {
        config.capture.interface.clone()
    } else if let Some(found) = find_monitor_interface()? {
        info!("Auto-detected monitor interface: {}", found);
        config.capture.interface = found.clone();
        found
    } else {
        error!(
            "Interface {} is not in monitor mode and no monitor interface found.",
            config.capture.interface
        );
        error!("Use --set-monitor or run 'prowl scan' to find interfaces.");
        return Ok(());
    };

    info!("Using interface: {}", interface);

    // Open database
    let db = Database::open(&config.capture.database).context("Failed to open database")?;

    // Load ignore lists
    let ignore_lists =
        IgnoreLists::load(&config.ignore_lists.mac, &config.ignore_lists.ssid).unwrap_or_default();

    // Set up shared running flag for signal handling
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // Set up Ctrl+C handler
    ctrlc::set_handler(move || {
        eprintln!("\nReceived Ctrl+C, stopping capture...");
        r.store(false, Ordering::SeqCst);
    })?;

    // Create capture engine with shared running flag
    let engine = CaptureEngine::new(config.clone(), db, ignore_lists, running);

    // Run capture
    if let Err(e) = engine.run().await {
        error!("Capture failed: {}", e);
        std::process::exit(1);
    }

    // Force exit to ensure all threads terminate
    info!("Exiting...");
    std::process::exit(0);
}

fn handle_analyze(config: Config, last_hours: u32, output: Option<PathBuf>) -> Result<()> {
    let db = Database::open(&config.capture.database).context("Failed to open database")?;

    let analyzer = SurveillanceAnalyzer::new(
        config.analysis.time_windows_minutes,
        config.analysis.persistence_threshold,
    );

    let alerts = analyzer.analyze(&db, last_hours)?;

    ReportGenerator::generate_surveillance_report(&alerts, output.as_deref())
}

fn handle_report(config: Config, output: Option<PathBuf>, report_type: String) -> Result<()> {
    let db = Database::open(&config.capture.database).context("Failed to open database")?;

    match report_type.as_str() {
        "devices" => ReportGenerator::generate_device_list(&db, output.as_deref()),
        "stats" => ReportGenerator::generate_stats(&db),
        _ => {
            error!("Unknown report type: {}", report_type);
            Ok(())
        }
    }
}

fn handle_list(config: Config, last_hours: Option<u32>, detailed: bool) -> Result<()> {
    let db = Database::open(&config.capture.database).context("Failed to open database")?;

    let devices = if let Some(hours) = last_hours {
        let now = chrono::Utc::now().timestamp();
        let start = now - (hours as i64 * 3600);
        db.get_devices_in_time_range(start, now)?
    } else {
        db.get_all_devices()?
    };

    println!("Found {} devices", devices.len());
    println!();

    for device in &devices {
        let probes = db.get_probes_for_device(device.id)?;
        let ssids = db.get_unique_ssids_for_device(device.id)?;

        println!("MAC: {}", device.mac);
        println!("  Probes: {}", probes.len());
        println!("  SSIDs: {}", ssids.join(", "));

        if detailed && !probes.is_empty() {
            println!("  Recent probes:");
            for probe in probes.iter().take(5) {
                let ssid = if probe.ssid.is_empty() {
                    "<broadcast>"
                } else {
                    &probe.ssid
                };
                println!(
                    "    - {} (ch:{:?}, signal:{:?})",
                    ssid, probe.channel, probe.signal_dbm
                );
            }
        }
        println!();
    }

    Ok(())
}

fn handle_stats(config: Config) -> Result<()> {
    let db = Database::open(&config.capture.database).context("Failed to open database")?;

    ReportGenerator::generate_stats(&db)
}

fn handle_init() -> Result<()> {
    info!("Initializing prowl configuration...");

    // Create default config
    let config = Config::default();
    config.save("config.json")?;
    info!("Created config.json");

    // Create ignore lists directory and files
    create_default_ignore_lists("ignore_lists")?;

    info!("Initialization complete!");
    info!("Edit config.json to customize settings.");
    info!("Run 'sudo prowl capture -i <interface> --set-monitor' to start.");

    Ok(())
}

fn handle_db(config: Config, action: DbCommands) -> Result<()> {
    use rusqlite::Connection;
    use std::fs::File;
    use std::io::{self, Write};

    let db_path = &config.capture.database;

    match action {
        DbCommands::Query { sql } => {
            let conn = Connection::open(db_path).context("Failed to open database")?;

            let mut stmt = conn.prepare(&sql)?;
            let column_count = stmt.column_count();
            let column_names: Vec<String> =
                stmt.column_names().iter().map(|s| s.to_string()).collect();

            // Print header
            println!("{}", column_names.join(" | "));
            println!("{}", "-".repeat(column_names.join(" | ").len()));

            // Execute and print rows
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let values: Vec<String> = (0..column_count)
                    .map(|i| {
                        row.get::<_, rusqlite::types::Value>(i)
                            .map(|v| format_value(&v))
                            .unwrap_or_else(|_| "NULL".to_string())
                    })
                    .collect();
                println!("{}", values.join(" | "));
            }
        }

        DbCommands::Schema => {
            let conn = Connection::open(db_path).context("Failed to open database")?;

            println!("Database: {}", db_path);
            println!();

            // Get all tables
            let mut stmt = conn
                .prepare("SELECT name, sql FROM sqlite_master WHERE type='table' ORDER BY name")?;
            let mut rows = stmt.query([])?;

            while let Some(row) = rows.next()? {
                let name: String = row.get(0)?;
                let sql: String = row.get(1)?;
                println!("-- Table: {}", name);
                println!("{};", sql);
                println!();
            }

            // Get all indexes
            let mut stmt = conn.prepare(
                "SELECT name, sql FROM sqlite_master WHERE type='index' AND sql IS NOT NULL ORDER BY name"
            )?;
            let mut rows = stmt.query([])?;

            println!("-- Indexes:");
            while let Some(row) = rows.next()? {
                let sql: String = row.get(1)?;
                println!("{};", sql);
            }
        }

        DbCommands::Export { table, output } => {
            let conn = Connection::open(db_path).context("Failed to open database")?;

            let sql = format!("SELECT * FROM {}", table);
            let mut stmt = conn.prepare(&sql)?;
            let column_names: Vec<String> =
                stmt.column_names().iter().map(|s| s.to_string()).collect();
            let column_count = stmt.column_count();

            let mut writer: Box<dyn Write> = match &output {
                Some(path) => Box::new(File::create(path)?),
                None => Box::new(io::stdout()),
            };

            // CSV header
            writeln!(writer, "{}", column_names.join(","))?;

            // CSV rows
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let values: Vec<String> = (0..column_count)
                    .map(|i| {
                        row.get::<_, rusqlite::types::Value>(i)
                            .map(|v| csv_escape(&format_value(&v)))
                            .unwrap_or_else(|_| "".to_string())
                    })
                    .collect();
                writeln!(writer, "{}", values.join(","))?;
            }

            if output.is_some() {
                info!("Exported {} to CSV", table);
            }
        }

        DbCommands::Import { source } => {
            let src_conn = Connection::open(&source).context("Failed to open source database")?;
            let dst_conn =
                Connection::open(db_path).context("Failed to open destination database")?;

            // Check if source has devices/probes tables
            let has_devices: bool = src_conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='devices'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(false);

            if !has_devices {
                anyhow::bail!("Source database doesn't have 'devices' table");
            }

            // Import devices
            let mut count = 0;
            {
                let mut stmt =
                    src_conn.prepare("SELECT mac, first_seen, last_seen FROM devices")?;
                let mut rows = stmt.query([])?;
                while let Some(row) = rows.next()? {
                    let mac: String = row.get(0)?;
                    let first_seen: i64 = row.get(1)?;
                    let last_seen: i64 = row.get(2)?;

                    dst_conn.execute(
                        "INSERT OR REPLACE INTO devices (mac, first_seen, last_seen) VALUES (?, ?, ?)",
                        rusqlite::params![mac, first_seen, last_seen],
                    )?;
                    count += 1;
                }
            }
            info!("Imported {} devices", count);

            // Import probes
            count = 0;
            {
                let mut stmt = src_conn.prepare(
                    "SELECT device_id, ssid, timestamp, lat, lon, signal_dbm, channel FROM probes",
                )?;
                let mut rows = stmt.query([])?;
                while let Some(row) = rows.next()? {
                    let device_id: i64 = row.get(0)?;
                    let ssid: String = row.get(1)?;
                    let timestamp: i64 = row.get(2)?;
                    let lat: Option<f64> = row.get(3)?;
                    let lon: Option<f64> = row.get(4)?;
                    let signal: Option<i32> = row.get(5)?;
                    let channel: Option<i32> = row.get(6)?;

                    dst_conn.execute(
                        "INSERT INTO probes (device_id, ssid, timestamp, lat, lon, signal_dbm, channel) VALUES (?, ?, ?, ?, ?, ?, ?)",
                        rusqlite::params![device_id, ssid, timestamp, lat, lon, signal, channel],
                    )?;
                    count += 1;
                }
            }
            info!("Imported {} probes", count);
        }

        DbCommands::Vacuum => {
            let conn = Connection::open(db_path).context("Failed to open database")?;

            let size_before: i64 = std::fs::metadata(db_path)?.len() as i64;
            conn.execute("VACUUM", [])?;
            let size_after: i64 = std::fs::metadata(db_path)?.len() as i64;

            println!("Database vacuumed:");
            println!("  Before: {} bytes", size_before);
            println!("  After:  {} bytes", size_after);
            println!("  Saved:  {} bytes", size_before - size_after);
        }
    }

    Ok(())
}

fn format_value(v: &rusqlite::types::Value) -> String {
    use rusqlite::types::Value;
    match v {
        Value::Null => "NULL".to_string(),
        Value::Integer(i) => i.to_string(),
        Value::Real(f) => f.to_string(),
        Value::Text(s) => s.clone(),
        Value::Blob(b) => format!("<blob {} bytes>", b.len()),
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
