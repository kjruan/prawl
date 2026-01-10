use anyhow::Result;
use log::{info, warn};
use std::net::TcpStream;
use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::channels::{find_monitor_interface, is_monitor_mode, set_monitor_mode};
use crate::config::{Config, GpsConfig};

/// Result of startup validation
pub struct ValidationResult {
    /// The resolved interface name to use (may differ from config if auto-detected)
    pub interface: String,
    /// Whether GPS is available (None if disabled, Some(true) if working, Some(false) if failed)
    pub gps_available: Option<bool>,
    /// Error message if GPS failed to initialize
    pub gps_error: Option<String>,
}

/// Errors that are fatal and should stop startup
#[derive(Debug)]
pub enum ValidationError {
    NoMonitorInterface {
        configured_interface: String,
        message: String,
    },
    GpsUnavailable {
        host: String,
        port: u16,
        message: String,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::NoMonitorInterface { message, .. } => {
                write!(f, "{}", message)
            }
            ValidationError::GpsUnavailable { message, .. } => {
                write!(f, "{}", message)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Check if gpsd daemon is reachable via TCP connection
pub fn check_gpsd_reachable(config: &GpsConfig) -> bool {
    let addr = format!("{}:{}", config.host, config.port);

    match TcpStream::connect_timeout(
        &addr.parse().unwrap_or_else(|_| format!("127.0.0.1:{}", config.port).parse().unwrap()),
        Duration::from_secs(2),
    ) {
        Ok(_) => {
            info!("GPS daemon reachable at {}", addr);
            true
        }
        Err(_) => false,
    }
}

/// Attempt to start gpsd via systemctl
pub fn start_gpsd() -> Result<bool> {
    info!("Attempting to start gpsd via systemctl...");

    let output = Command::new("systemctl")
        .args(["start", "gpsd"])
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                info!("gpsd service started successfully");
                Ok(true)
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                warn!("Failed to start gpsd: {}", stderr);
                Ok(false)
            }
        }
        Err(e) => {
            warn!("Failed to execute systemctl: {}", e);
            Ok(false)
        }
    }
}

/// Ensure gpsd is running, attempting to start it if necessary
pub fn ensure_gpsd_running(config: &GpsConfig) -> Result<(), ValidationError> {
    // First check if already reachable
    if check_gpsd_reachable(config) {
        return Ok(());
    }

    info!(
        "GPS daemon not reachable at {}:{}, attempting to start...",
        config.host, config.port
    );

    // Attempt to start gpsd
    let _ = start_gpsd();

    // Wait briefly for gpsd to start
    thread::sleep(Duration::from_secs(2));

    // Recheck connection
    if check_gpsd_reachable(config) {
        return Ok(());
    }

    // Still not reachable, return error
    Err(ValidationError::GpsUnavailable {
        host: config.host.clone(),
        port: config.port,
        message: format!(
            "GPS validation failed.\n\n\
            GPS is enabled but gpsd daemon is not reachable at {}:{}.\n\
            Attempted to start gpsd via 'systemctl start gpsd' but it failed or is still unreachable.\n\n\
            To fix this:\n\
            1. Check gpsd status: systemctl status gpsd\n\
            2. Start manually: sudo gpsd /dev/ttyACM0 -F /var/run/gpsd.sock\n\
            3. Or disable GPS in config.json: \"gps\": {{ \"enabled\": false, ... }}",
            config.host, config.port
        ),
    })
}

/// Resolve and validate the monitor mode interface
pub fn resolve_monitor_interface(
    configured_interface: &str,
    set_monitor: bool,
) -> Result<String, ValidationError> {
    if set_monitor {
        // User explicitly requested to set monitor mode
        match set_monitor_mode(configured_interface) {
            Ok(()) => return Ok(configured_interface.to_string()),
            Err(e) => {
                return Err(ValidationError::NoMonitorInterface {
                    configured_interface: configured_interface.to_string(),
                    message: format!(
                        "Failed to set monitor mode on '{}'.\n\n\
                        Error: {}\n\n\
                        Troubleshooting:\n\
                        1. Ensure you're running with sudo/root privileges\n\
                        2. Check that interface '{}' exists: ip link show\n\
                        3. Verify the interface supports monitor mode: iw list | grep -A 5 'Supported interface modes'",
                        configured_interface, e, configured_interface
                    ),
                });
            }
        }
    }

    // Check if configured interface is already in monitor mode
    match is_monitor_mode(configured_interface) {
        Ok(true) => return Ok(configured_interface.to_string()),
        Ok(false) => {
            // Interface exists but not in monitor mode, try auto-detect
        }
        Err(_) => {
            // Interface might not exist, try auto-detect
        }
    }

    // Try to auto-detect a monitor interface
    match find_monitor_interface() {
        Ok(Some(iface)) => {
            info!("Auto-detected monitor interface: {}", iface);
            Ok(iface)
        }
        Ok(None) | Err(_) => Err(ValidationError::NoMonitorInterface {
            configured_interface: configured_interface.to_string(),
            message: format!(
                "No monitor mode interface found.\n\n\
                The configured interface '{}' is not in monitor mode,\n\
                and no other monitor mode interface was detected.\n\n\
                To fix this, either:\n\
                1. Run with --set-monitor flag to auto-enable monitor mode:\n\
                   sudo prowl capture --set-monitor\n\
                   sudo prowl tui --set-monitor\n\n\
                2. Manually enable monitor mode:\n\
                   sudo ip link set {} down\n\
                   sudo iw dev {} set type monitor\n\
                   sudo ip link set {} up\n\n\
                3. Run 'prowl scan' to see available wireless interfaces",
                configured_interface,
                configured_interface,
                configured_interface,
                configured_interface
            ),
        }),
    }
}

/// Perform all startup validations for capture/tui modes
///
/// This function should be called early in startup, before database init.
///
/// Returns:
/// - Ok(ValidationResult) with resolved interface and GPS status
/// - Err only if WLAN validation fails (GPS errors are non-fatal)
pub fn validate_startup(
    config: &Config,
    set_monitor: bool,
) -> Result<ValidationResult, ValidationError> {
    // 1. Validate GPS if enabled (non-fatal if fails)
    let (gps_available, gps_error) = if config.gps.enabled {
        match ensure_gpsd_running(&config.gps) {
            Ok(()) => (Some(true), None),
            Err(e) => {
                warn!("GPS validation failed (continuing without GPS): {}", e);
                (Some(false), Some(e.to_string()))
            }
        }
    } else {
        (None, None)
    };

    // 2. Validate WLAN monitor mode (fatal if fails)
    let interface = resolve_monitor_interface(&config.capture.interface, set_monitor)?;

    Ok(ValidationResult {
        interface,
        gps_available,
        gps_error,
    })
}
