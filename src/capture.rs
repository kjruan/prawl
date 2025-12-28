use crate::channels::ChannelHopper;
use crate::config::Config;
use crate::database::{Database, ProbeCapture};
use crate::distance::{estimate_distance, format_distance, distance_category};
use crate::gps::GpsClient;
use crate::ignore::IgnoreLists;
use crate::parser::parse_probe_request;
use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use pcap::Capture;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

pub struct CaptureEngine {
    config: Config,
    db: Database,
    ignore_lists: IgnoreLists,
    running: Arc<AtomicBool>,
}

impl CaptureEngine {
    pub fn new(config: Config, db: Database, ignore_lists: IgnoreLists, running: Arc<AtomicBool>) -> Self {
        CaptureEngine {
            config,
            db,
            ignore_lists,
            running,
        }
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn running_flag(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }

    pub async fn run(&self) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);

        let interface = &self.config.capture.interface;
        info!("Starting capture on interface: {}", interface);

        // Open capture handle
        let mut cap = Capture::from_device(interface.as_str())
            .context("Failed to open capture device")?
            .promisc(true)
            .snaplen(65535)
            .timeout(1000)
            .open()
            .context("Failed to activate capture")?;

        // Set to monitor mode filter for probe requests
        // BPF filter for management frames type 0 subtype 4 (probe request)
        if let Err(e) = cap.filter("type mgt subtype probe-req", true) {
            warn!("Failed to set BPF filter, will filter in software: {}", e);
        }

        // Start channel hopper in background
        let hopper = ChannelHopper::new(
            interface.clone(),
            self.config.capture.channels.clone(),
            self.config.capture.hop_interval_ms,
        );
        let running_clone = self.running.clone();
        let hopper_handle = tokio::spawn(async move {
            if let Err(e) = hopper.run(running_clone).await {
                error!("Channel hopper error: {}", e);
            }
        });

        // Start GPS client if enabled
        let gps_rx = if self.config.gps.enabled {
            let (tx, rx) = mpsc::channel(1);
            let gps_client = GpsClient::new(
                self.config.gps.host.clone(),
                self.config.gps.port,
            );
            let running_gps = self.running.clone();
            tokio::spawn(async move {
                if let Err(e) = gps_client.run(tx, running_gps).await {
                    warn!("GPS client error: {}", e);
                }
            });
            Some(rx)
        } else {
            None
        };

        let mut gps_position: Option<(f64, f64)> = None;
        let mut gps_rx = gps_rx;
        let mut current_channel: Option<u8> = None;
        let mut packet_count = 0u64;
        let mut probe_count = 0u64;

        info!("Capture started. Press Ctrl+C to stop.");

        while self.running.load(Ordering::SeqCst) {
            // Update GPS position if available
            if let Some(ref mut rx) = gps_rx {
                if let Ok(pos) = rx.try_recv() {
                    gps_position = Some(pos);
                    debug!("GPS position updated: {:?}", gps_position);
                }
            }

            // Capture packet
            match cap.next_packet() {
                Ok(packet) => {
                    packet_count += 1;

                    // Extract signal strength from radiotap header if present
                    let signal_dbm = extract_signal_dbm(packet.data);

                    // Parse probe request
                    if let Some(probe) = parse_probe_request(packet.data, signal_dbm) {
                        // Check ignore lists
                        if self.ignore_lists.should_ignore_mac(&probe.source_mac) {
                            debug!("Ignoring MAC: {}", probe.source_mac);
                            continue;
                        }
                        if !probe.ssid.is_empty() && self.ignore_lists.should_ignore_ssid(&probe.ssid) {
                            debug!("Ignoring SSID: {}", probe.ssid);
                            continue;
                        }

                        probe_count += 1;
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i64;

                        // Calculate estimated distance from signal strength
                        let distance_m = if self.config.distance.enabled {
                            probe.signal_dbm.and_then(|rssi| {
                                estimate_distance(
                                    rssi,
                                    self.config.distance.tx_power_dbm,
                                    self.config.distance.path_loss_exponent,
                                )
                            })
                        } else {
                            None
                        };

                        let capture = ProbeCapture {
                            mac: probe.source_mac.clone(),
                            ssid: probe.ssid.clone(),
                            timestamp: now,
                            lat: gps_position.map(|(lat, _)| lat),
                            lon: gps_position.map(|(_, lon)| lon),
                            signal_dbm: probe.signal_dbm,
                            channel: current_channel,
                            distance_m,
                        };

                        if let Err(e) = self.db.insert_probe(&capture) {
                            error!("Failed to insert probe: {}", e);
                        } else {
                            // Format output with distance if available
                            let distance_str = distance_m
                                .map(|d| format!(" ~{} ({})", format_distance(d), distance_category(d)))
                                .unwrap_or_default();

                            info!(
                                "Probe: MAC={} SSID={:?} Signal={:?}dBm{}",
                                probe.source_mac,
                                if probe.ssid.is_empty() { "<broadcast>" } else { &probe.ssid },
                                probe.signal_dbm,
                                distance_str
                            );
                        }
                    }
                }
                Err(pcap::Error::TimeoutExpired) => {
                    // Normal timeout, continue
                    continue;
                }
                Err(e) => {
                    if self.running.load(Ordering::SeqCst) {
                        error!("Capture error: {}", e);
                    }
                    break;
                }
            }
        }

        info!("Capture stopped. Packets: {}, Probes: {}", packet_count, probe_count);
        hopper_handle.abort();
        Ok(())
    }
}

fn extract_signal_dbm(data: &[u8]) -> Option<i32> {
    // Check for radiotap header
    if data.len() < 8 || data[0] != 0 {
        return None;
    }

    let radiotap_len = u16::from_le_bytes([data[2], data[3]]) as usize;
    if radiotap_len > data.len() || radiotap_len < 8 {
        return None;
    }

    // Present flags are at bytes 4-7 (can extend further)
    let present = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

    // Radiotap field order is fixed. We need to count bytes to find signal.
    // Bit 5 = DBM Antenna Signal
    if present & (1 << 5) == 0 {
        return None;
    }

    // Count bytes before signal field
    let mut offset = 8;

    // TSFT (bit 0) - 8 bytes, aligned to 8
    if present & (1 << 0) != 0 {
        offset = (offset + 7) & !7; // align to 8
        offset += 8;
    }

    // Flags (bit 1) - 1 byte
    if present & (1 << 1) != 0 {
        offset += 1;
    }

    // Rate (bit 2) - 1 byte
    if present & (1 << 2) != 0 {
        offset += 1;
    }

    // Channel (bit 3) - 4 bytes, aligned to 2
    if present & (1 << 3) != 0 {
        offset = (offset + 1) & !1; // align to 2
        offset += 4;
    }

    // FHSS (bit 4) - 2 bytes
    if present & (1 << 4) != 0 {
        offset += 2;
    }

    // Now we're at DBM Antenna Signal (bit 5) - 1 byte, signed
    if offset < radiotap_len {
        let signal = data[offset] as i8;
        return Some(signal as i32);
    }

    None
}
