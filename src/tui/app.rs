use crate::distance::{
    AdaptiveCalibrator, CalibrationStatus, DistanceEstimate, RssiTracker,
    estimate_distance_smart, estimate_tx_power_from_wifi_gen,
};
use crate::parser::ProbeCapabilities;
use crate::tui::TuiEvent;
use std::collections::VecDeque;
use tokio::sync::mpsc;

/// Maximum entries in the probe log ring buffer
const MAX_PROBE_LOG_ENTRIES: usize = 500;

/// Active panel for focus/navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivePanel {
    #[default]
    ProbeLog,
    DeviceTable,
}

/// Sorting options for device table
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeviceSortField {
    Mac,
    #[default]
    LastSeen,
    ProbeCount,
    Signal,
}

/// Statistics snapshot
#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub total_devices: usize,
    pub total_probes: usize,
    pub probes_per_minute: f64,
    pub devices_last_5min: usize,
    pub devices_last_15min: usize,
    pub unique_ssids: usize,
    pub capture_duration_secs: u64,
}

/// Device display entry with computed fields
#[derive(Debug, Clone)]
pub struct DeviceEntry {
    pub mac: String,
    pub first_seen: i64,
    pub last_seen: i64,
    pub probe_count: usize,
    pub ssids: Vec<String>,
    pub last_signal: Option<i32>,
    pub last_distance: Option<f64>,
    pub capabilities: Option<ProbeCapabilities>,
    pub wifi_generation: Option<String>,
    /// RSSI history for averaging
    pub rssi_tracker: RssiTracker,
    /// Computed distance estimate with uncertainty
    pub distance_estimate: Option<DistanceEstimate>,
}

/// Single probe log entry for display
#[derive(Debug, Clone)]
pub struct ProbeLogEntry {
    pub timestamp: i64,
    pub mac: String,
    pub ssid: String,
    pub signal_dbm: Option<i32>,
    pub distance_m: Option<f64>,
    pub channel: Option<u8>,
    pub capabilities: Option<ProbeCapabilities>,
}

/// Main application state
pub struct App {
    /// Whether the app is running
    pub running: bool,

    /// Currently active/focused panel
    pub active_panel: ActivePanel,

    /// Probe log entries (ring buffer)
    pub probe_log: VecDeque<ProbeLogEntry>,

    /// Device list (updated from probe events)
    pub devices: Vec<DeviceEntry>,

    /// Currently selected device index
    pub selected_device: usize,

    /// Device table scroll offset
    pub device_scroll: usize,

    /// Probe log scroll offset (0 = bottom/newest)
    pub log_scroll: usize,

    /// Current sort field and direction
    pub sort_field: DeviceSortField,
    pub sort_ascending: bool,

    /// Statistics
    pub stats: Stats,

    /// GPS status
    pub gps_position: Option<(f64, f64)>,
    pub gps_connected: bool,

    /// Current channel
    pub current_channel: Option<u8>,

    /// Capture status
    pub capture_active: bool,

    /// Help overlay visible
    pub show_help: bool,

    /// Device detail view (Some = viewing device at index)
    pub detail_view: Option<usize>,

    /// Event receiver
    pub event_rx: mpsc::Receiver<TuiEvent>,

    /// Probe count since start
    pub probes_since_start: usize,

    /// Adaptive path loss calibrator
    pub calibrator: AdaptiveCalibrator,

    /// Current calibration status for display
    pub calibration_status: Option<CalibrationStatus>,
}

impl App {
    pub fn new(event_rx: mpsc::Receiver<TuiEvent>, initial_stats: Stats) -> Self {
        App {
            running: true,
            active_panel: ActivePanel::ProbeLog,
            probe_log: VecDeque::with_capacity(MAX_PROBE_LOG_ENTRIES),
            devices: Vec::new(),
            selected_device: 0,
            device_scroll: 0,
            log_scroll: 0,
            sort_field: DeviceSortField::LastSeen,
            sort_ascending: false,
            stats: initial_stats,
            gps_position: None,
            gps_connected: false,
            current_channel: None,
            capture_active: false,
            show_help: false,
            detail_view: None,
            event_rx,
            probes_since_start: 0,
            calibrator: AdaptiveCalibrator::default(),
            calibration_status: None,
        }
    }

    pub fn tick(&mut self) {
        // Calculate probes per minute
        if self.stats.capture_duration_secs > 0 {
            self.stats.probes_per_minute = (self.probes_since_start as f64)
                / (self.stats.capture_duration_secs as f64 / 60.0);
        }

        // Run adaptive calibration on devices with enough samples
        let path_loss = self.calibrator.path_loss();
        for device in &mut self.devices {
            // Only analyze devices with enough RSSI history
            if device.rssi_tracker.sample_count() >= 10 {
                if let Some(stats) = device.rssi_tracker.stats() {
                    let tx_power = estimate_tx_power_from_wifi_gen(
                        device.wifi_generation.as_deref()
                    );
                    self.calibrator.analyze_device(&stats, tx_power);
                }
            }

            // Update distance estimate using calibrator's path loss
            if let Some(avg_rssi) = device.rssi_tracker.weighted_average() {
                device.distance_estimate = estimate_distance_smart(
                    avg_rssi,
                    device.wifi_generation.as_deref(),
                    path_loss,
                    device.rssi_tracker.sample_count(),
                    self.calibrator.inferred_tx_power(),
                );
            }
        }

        // Update calibration status for display
        self.calibration_status = Some(self.calibrator.status());
    }

    pub fn handle_event(&mut self, event: TuiEvent) {
        match event {
            TuiEvent::ProbeReceived(entry) => {
                self.probes_since_start += 1;

                // Update or add device
                if let Some(device) = self.devices.iter_mut().find(|d| d.mac == entry.mac) {
                    device.probe_count += 1;
                    device.last_seen = entry.timestamp;
                    device.last_signal = entry.signal_dbm;
                    device.last_distance = entry.distance_m;
                    if !entry.ssid.is_empty() && !device.ssids.contains(&entry.ssid) {
                        device.ssids.push(entry.ssid.clone());
                    }
                    // Track RSSI for averaging and calibration
                    if let Some(rssi) = entry.signal_dbm {
                        device.rssi_tracker.add_sample(rssi);
                        // Record strong signals for TX power estimation
                        self.calibrator.record_peak_rssi(rssi);
                    }
                    // Update capabilities if present (keep most recent)
                    if entry.capabilities.is_some() {
                        device.capabilities = entry.capabilities.clone();
                        device.wifi_generation = entry.capabilities.as_ref()
                            .map(|c| c.wifi_generation.clone())
                            .filter(|s| !s.is_empty());
                    }
                } else {
                    let mut ssids = Vec::new();
                    if !entry.ssid.is_empty() {
                        ssids.push(entry.ssid.clone());
                    }
                    let wifi_generation = entry.capabilities.as_ref()
                        .map(|c| c.wifi_generation.clone())
                        .filter(|s| !s.is_empty());
                    let mut rssi_tracker = RssiTracker::default();
                    if let Some(rssi) = entry.signal_dbm {
                        rssi_tracker.add_sample(rssi);
                        // Record strong signals for TX power estimation
                        self.calibrator.record_peak_rssi(rssi);
                    }
                    self.devices.push(DeviceEntry {
                        mac: entry.mac.clone(),
                        first_seen: entry.timestamp,
                        last_seen: entry.timestamp,
                        probe_count: 1,
                        ssids,
                        last_signal: entry.signal_dbm,
                        last_distance: entry.distance_m,
                        capabilities: entry.capabilities.clone(),
                        wifi_generation,
                        rssi_tracker,
                        distance_estimate: None,
                    });
                }

                // Add to log
                if self.probe_log.len() >= MAX_PROBE_LOG_ENTRIES {
                    self.probe_log.pop_front();
                }
                self.probe_log.push_back(entry);

                // Sort devices
                self.sort_devices();
            }
            TuiEvent::GpsUpdate(lat, lon) => {
                self.gps_position = Some((lat, lon));
                self.gps_connected = true;
            }
            TuiEvent::GpsDisconnected => {
                self.gps_connected = false;
            }
            TuiEvent::ChannelChanged(ch) => {
                self.current_channel = Some(ch);
            }
            TuiEvent::StatsUpdate(stats) => {
                // Preserve probes_per_minute from local calculation
                let ppm = self.stats.probes_per_minute;
                self.stats = stats;
                self.stats.probes_per_minute = ppm;
            }
            TuiEvent::CaptureStarted => {
                self.capture_active = true;
            }
            TuiEvent::CaptureStopped => {
                self.capture_active = false;
            }
            TuiEvent::Error(_msg) => {
                // Could display in status bar
            }
        }
    }

    pub fn next_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::ProbeLog => ActivePanel::DeviceTable,
            ActivePanel::DeviceTable => ActivePanel::ProbeLog,
        };
    }

    pub fn prev_panel(&mut self) {
        self.next_panel(); // Only 2 panels, same as next
    }

    pub fn scroll_up(&mut self) {
        match self.active_panel {
            ActivePanel::ProbeLog => {
                if self.log_scroll < self.probe_log.len().saturating_sub(1) {
                    self.log_scroll += 1;
                }
            }
            ActivePanel::DeviceTable => {
                if self.selected_device > 0 {
                    self.selected_device -= 1;
                }
            }
        }
    }

    pub fn scroll_down(&mut self) {
        match self.active_panel {
            ActivePanel::ProbeLog => {
                if self.log_scroll > 0 {
                    self.log_scroll -= 1;
                }
            }
            ActivePanel::DeviceTable => {
                if self.selected_device < self.devices.len().saturating_sub(1) {
                    self.selected_device += 1;
                }
            }
        }
    }

    pub fn cycle_sort(&mut self) {
        self.sort_field = match self.sort_field {
            DeviceSortField::Mac => DeviceSortField::LastSeen,
            DeviceSortField::LastSeen => DeviceSortField::ProbeCount,
            DeviceSortField::ProbeCount => DeviceSortField::Signal,
            DeviceSortField::Signal => DeviceSortField::Mac,
        };
        self.sort_devices();
    }

    pub fn reverse_sort(&mut self) {
        self.sort_ascending = !self.sort_ascending;
        self.sort_devices();
    }

    fn sort_devices(&mut self) {
        let ascending = self.sort_ascending;
        match self.sort_field {
            DeviceSortField::Mac => {
                self.devices.sort_by(|a, b| {
                    if ascending {
                        a.mac.cmp(&b.mac)
                    } else {
                        b.mac.cmp(&a.mac)
                    }
                });
            }
            DeviceSortField::LastSeen => {
                self.devices.sort_by(|a, b| {
                    if ascending {
                        a.last_seen.cmp(&b.last_seen)
                    } else {
                        b.last_seen.cmp(&a.last_seen)
                    }
                });
            }
            DeviceSortField::ProbeCount => {
                self.devices.sort_by(|a, b| {
                    if ascending {
                        a.probe_count.cmp(&b.probe_count)
                    } else {
                        b.probe_count.cmp(&a.probe_count)
                    }
                });
            }
            DeviceSortField::Signal => {
                self.devices.sort_by(|a, b| {
                    let a_sig = a.last_signal.unwrap_or(i32::MIN);
                    let b_sig = b.last_signal.unwrap_or(i32::MIN);
                    if ascending {
                        a_sig.cmp(&b_sig)
                    } else {
                        b_sig.cmp(&a_sig)
                    }
                });
            }
        }
    }

    pub fn select_device(&mut self) {
        if !self.devices.is_empty() && self.selected_device < self.devices.len() {
            self.detail_view = Some(self.selected_device);
        }
    }
}
