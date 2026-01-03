use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub capture: CaptureConfig,
    pub gps: GpsConfig,
    pub analysis: AnalysisConfig,
    pub ignore_lists: IgnoreListsConfig,
    #[serde(default)]
    pub distance: DistanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistanceConfig {
    /// Enable distance estimation
    pub enabled: bool,
    /// Default reference signal strength at 1 meter (dBm), typical range: -38 to -50
    #[serde(default = "default_tx_power")]
    pub tx_power_dbm: f64,
    /// Path loss exponent: 2.0 = free space, 2.5-4.0 = indoors with obstacles
    #[serde(default = "default_path_loss")]
    pub path_loss_exponent: f64,
    /// Use WiFi generation to estimate TX power (overrides tx_power_dbm when true)
    #[serde(default = "default_true")]
    pub use_smart_tx_power: bool,
    /// Calibrated TX power from user calibration (overrides smart estimation)
    #[serde(default)]
    pub calibrated_tx_power: Option<f64>,
    /// When calibration was performed (ISO 8601 timestamp)
    #[serde(default)]
    pub calibrated_at: Option<String>,
    /// Distance used for calibration
    #[serde(default)]
    pub calibration_distance_m: Option<f64>,
    /// Number of RSSI samples to average per device
    #[serde(default = "default_rssi_samples")]
    pub rssi_average_samples: usize,
}

fn default_tx_power() -> f64 { -43.0 }
fn default_path_loss() -> f64 { 3.0 }
fn default_true() -> bool { true }
fn default_rssi_samples() -> usize { 5 }

impl Default for DistanceConfig {
    fn default() -> Self {
        DistanceConfig {
            enabled: true,
            tx_power_dbm: -43.0,              // Updated default
            path_loss_exponent: 3.0,           // Indoor environment
            use_smart_tx_power: true,          // Use WiFi gen estimation
            calibrated_tx_power: None,
            calibrated_at: None,
            calibration_distance_m: None,
            rssi_average_samples: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    pub interface: String,
    pub channels: Vec<u8>,
    pub hop_interval_ms: u64,
    pub database: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    pub time_windows_minutes: Vec<u32>,
    pub persistence_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgnoreListsConfig {
    pub mac: String,
    pub ssid: String,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {:?}", path.as_ref()))?;

        let config: Config = serde_json::from_str(&content)
            .with_context(|| "Failed to parse config file")?;

        Ok(config)
    }

    pub fn default_config() -> Self {
        Config {
            capture: CaptureConfig {
                interface: "wlan1".to_string(),
                channels: vec![1, 6, 11],
                hop_interval_ms: 250,
                database: "./prowl.db".to_string(),
            },
            gps: GpsConfig {
                enabled: true,
                host: "localhost".to_string(),
                port: 2947,
            },
            analysis: AnalysisConfig {
                time_windows_minutes: vec![5, 10, 15, 20],
                persistence_threshold: 0.7,
            },
            ignore_lists: IgnoreListsConfig {
                mac: "ignore_lists/mac_list.json".to_string(),
                ssid: "ignore_lists/ssid_list.json".to_string(),
            },
            distance: DistanceConfig::default(),
        }
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::default_config()
    }
}
