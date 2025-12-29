use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

pub struct ChannelHopper {
    interface: String,
    channels: Vec<u8>,
    hop_interval_ms: u64,
}

impl ChannelHopper {
    pub fn new(interface: String, channels: Vec<u8>, hop_interval_ms: u64) -> Self {
        ChannelHopper {
            interface,
            channels,
            hop_interval_ms,
        }
    }

    pub fn channels(&self) -> &[u8] {
        &self.channels
    }

    pub fn hop_interval_ms(&self) -> u64 {
        self.hop_interval_ms
    }

    pub async fn run(&self, running: Arc<AtomicBool>) -> Result<()> {
        if self.channels.is_empty() {
            warn!("No channels configured, using default 2.4GHz channels");
            return Ok(());
        }

        info!(
            "Starting channel hopper on {} with channels: {:?}, interval: {}ms",
            self.interface, self.channels, self.hop_interval_ms
        );

        let mut channel_idx = 0;

        while running.load(Ordering::SeqCst) {
            let channel = self.channels[channel_idx];

            if let Err(e) = self.set_channel(channel) {
                error!("Failed to set channel {}: {}", channel, e);
            } else {
                debug!("Switched to channel {}", channel);
            }

            channel_idx = (channel_idx + 1) % self.channels.len();
            sleep(Duration::from_millis(self.hop_interval_ms)).await;
        }

        info!("Channel hopper stopped");
        Ok(())
    }

    fn set_channel(&self, channel: u8) -> Result<()> {
        // Use iw to set channel
        let output = Command::new("iw")
            .args([
                "dev",
                &self.interface,
                "set",
                "channel",
                &channel.to_string(),
            ])
            .output()
            .context("Failed to execute iw command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("iw command failed: {}", stderr);
        }

        Ok(())
    }
}

/// Set interface to monitor mode
pub fn set_monitor_mode(interface: &str) -> Result<()> {
    info!("Setting {} to monitor mode", interface);

    // Bring interface down
    let output = Command::new("ip")
        .args(["link", "set", interface, "down"])
        .output()
        .context("Failed to bring interface down")?;

    if !output.status.success() {
        warn!("Failed to bring interface down: {:?}", output.stderr);
    }

    // Set monitor mode
    let output = Command::new("iw")
        .args(["dev", interface, "set", "type", "monitor"])
        .output()
        .context("Failed to set monitor mode")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to set monitor mode: {}", stderr);
    }

    // Bring interface up
    let output = Command::new("ip")
        .args(["link", "set", interface, "up"])
        .output()
        .context("Failed to bring interface up")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to bring interface up: {}", stderr);
    }

    info!("Interface {} is now in monitor mode", interface);
    Ok(())
}

/// Check if interface is in monitor mode
pub fn is_monitor_mode(interface: &str) -> Result<bool> {
    let output = Command::new("iw")
        .args(["dev", interface, "info"])
        .output()
        .context("Failed to get interface info")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains("type monitor"))
}

/// Find the first wireless interface in monitor mode
pub fn find_monitor_interface() -> Result<Option<String>> {
    let output = Command::new("iw")
        .args(["dev"])
        .output()
        .context("Failed to list wireless devices")?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse iw dev output to find interfaces
    let mut current_interface: Option<String> = None;

    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("Interface ") {
            current_interface = Some(line.strip_prefix("Interface ").unwrap_or("").to_string());
        } else if line.starts_with("type ") && line.contains("monitor") {
            if let Some(iface) = current_interface.take() {
                info!("Found monitor mode interface: {}", iface);
                return Ok(Some(iface));
            }
        }
    }

    Ok(None)
}

/// List all wireless interfaces with their modes
pub fn list_wireless_interfaces() -> Result<Vec<(String, String)>> {
    let output = Command::new("iw")
        .args(["dev"])
        .output()
        .context("Failed to list wireless devices")?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut interfaces = Vec::new();
    let mut current_interface: Option<String> = None;
    let mut current_type = String::from("unknown");

    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("Interface ") {
            // Save previous interface if exists
            if let Some(iface) = current_interface.take() {
                interfaces.push((iface, current_type.clone()));
            }
            current_interface = Some(line.strip_prefix("Interface ").unwrap_or("").to_string());
            current_type = String::from("unknown");
        } else if line.starts_with("type ") {
            current_type = line.strip_prefix("type ").unwrap_or("unknown").to_string();
        }
    }

    // Don't forget the last interface
    if let Some(iface) = current_interface {
        interfaces.push((iface, current_type));
    }

    Ok(interfaces)
}

/// Get list of available 2.4GHz channels
pub fn get_2ghz_channels() -> Vec<u8> {
    vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
}

/// Get list of common 5GHz channels
pub fn get_5ghz_channels() -> Vec<u8> {
    vec![
        36, 40, 44, 48, 52, 56, 60, 64, 100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140, 144,
        149, 153, 157, 161, 165,
    ]
}

/// Get all channels (2.4GHz + 5GHz)
pub fn get_all_channels() -> Vec<u8> {
    let mut channels = get_2ghz_channels();
    channels.extend(get_5ghz_channels());
    channels
}

/// Parse channel configuration string
pub fn parse_channels(config: &str) -> Vec<u8> {
    match config.to_lowercase().as_str() {
        "all" => get_all_channels(),
        "2ghz" | "2.4ghz" => get_2ghz_channels(),
        "5ghz" => get_5ghz_channels(),
        _ => {
            // Try to parse as comma-separated list
            config
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect()
        }
    }
}
