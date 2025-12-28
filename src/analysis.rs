use crate::database::{Database, Device, Probe};
use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use log::{debug, info, warn};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct SurveillanceAlert {
    pub device: Device,
    pub score: f64,
    pub reasons: Vec<String>,
    pub probed_ssids: Vec<String>,
    pub location_count: usize,
    pub appearance_count: usize,
}

pub struct SurveillanceAnalyzer {
    time_windows_minutes: Vec<u32>,
    persistence_threshold: f64,
}

impl SurveillanceAnalyzer {
    pub fn new(time_windows_minutes: Vec<u32>, persistence_threshold: f64) -> Self {
        SurveillanceAnalyzer {
            time_windows_minutes,
            persistence_threshold,
        }
    }

    pub fn analyze(&self, db: &Database, hours: u32) -> Result<Vec<SurveillanceAlert>> {
        let now = chrono::Utc::now().timestamp();
        let start = now - (hours as i64 * 3600);

        info!(
            "Analyzing surveillance patterns from {} to {}",
            format_timestamp(start),
            format_timestamp(now)
        );

        let devices = db.get_devices_in_time_range(start, now)?;
        info!("Found {} devices in time range", devices.len());

        let mut alerts = Vec::new();

        for device in devices {
            let probes = db.get_probes_for_device(device.id)?;
            if probes.is_empty() {
                continue;
            }

            let score = self.calculate_persistence_score(&device, &probes, start, now);
            let reasons = self.get_alert_reasons(&device, &probes, score);

            if score >= self.persistence_threshold {
                let ssids = db.get_unique_ssids_for_device(device.id)?;
                let location_count = db.get_device_location_count(device.id)?;

                alerts.push(SurveillanceAlert {
                    device: device.clone(),
                    score,
                    reasons,
                    probed_ssids: ssids,
                    location_count,
                    appearance_count: probes.len(),
                });
            }
        }

        // Sort by score descending
        alerts.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        info!("Found {} potential surveillance devices", alerts.len());
        Ok(alerts)
    }

    fn calculate_persistence_score(
        &self,
        device: &Device,
        probes: &[Probe],
        start: i64,
        end: i64,
    ) -> f64 {
        let mut score = 0.0;
        let total_duration = (end - start) as f64;

        // 1. Time window coverage score (40% weight)
        let window_score = self.calculate_window_coverage(probes, start, end);
        score += window_score * 0.4;

        // 2. Appearance frequency score (30% weight)
        let frequency_score = self.calculate_frequency_score(probes, start, end);
        score += frequency_score * 0.3;

        // 3. Session duration score (20% weight)
        let duration = (device.last_seen - device.first_seen) as f64;
        let duration_score = (duration / total_duration).min(1.0);
        score += duration_score * 0.2;

        // 4. Location diversity score (10% weight)
        let location_score = self.calculate_location_score(probes);
        score += location_score * 0.1;

        score.min(1.0)
    }

    fn calculate_window_coverage(&self, probes: &[Probe], start: i64, end: i64) -> f64 {
        if self.time_windows_minutes.is_empty() {
            return 0.0;
        }

        let mut windows_hit = 0;

        for window_minutes in &self.time_windows_minutes {
            let window_seconds = *window_minutes as i64 * 60;
            let window_start = end - window_seconds;
            let window_end = end;

            let has_probe = probes
                .iter()
                .any(|p| p.timestamp >= window_start && p.timestamp <= window_end);

            if has_probe {
                windows_hit += 1;
            }
        }

        windows_hit as f64 / self.time_windows_minutes.len() as f64
    }

    fn calculate_frequency_score(&self, probes: &[Probe], start: i64, end: i64) -> f64 {
        let duration_hours = ((end - start) as f64 / 3600.0).max(1.0);
        let probes_per_hour = probes.len() as f64 / duration_hours;

        // Normalize: 10+ probes/hour = 1.0, 1 probe/hour = 0.1
        (probes_per_hour / 10.0).min(1.0)
    }

    fn calculate_location_score(&self, probes: &[Probe]) -> f64 {
        // Count unique locations (rounded to ~100m precision)
        let locations: HashSet<(i64, i64)> = probes
            .iter()
            .filter_map(|p| {
                match (p.lat, p.lon) {
                    (Some(lat), Some(lon)) if lat != 0.0 || lon != 0.0 => {
                        Some(((lat * 1000.0) as i64, (lon * 1000.0) as i64))
                    }
                    _ => None,
                }
            })
            .collect();

        if locations.is_empty() {
            return 0.0;
        }

        // More locations = higher suspicion (following behavior)
        // 1 location = 0.2, 3+ locations = 1.0
        ((locations.len() as f64 - 1.0) / 2.0).min(1.0).max(0.2)
    }

    fn get_alert_reasons(&self, device: &Device, probes: &[Probe], score: f64) -> Vec<String> {
        let mut reasons = Vec::new();

        // Check for persistence across multiple time windows
        let window_coverage = self.calculate_window_coverage(
            probes,
            device.first_seen,
            device.last_seen,
        );
        if window_coverage >= 0.75 {
            reasons.push("Present across multiple time windows".to_string());
        }

        // Check for high frequency
        let duration_hours = ((device.last_seen - device.first_seen) as f64 / 3600.0).max(0.1);
        let probes_per_hour = probes.len() as f64 / duration_hours;
        if probes_per_hour > 5.0 {
            reasons.push(format!("High probe frequency: {:.1}/hour", probes_per_hour));
        }

        // Check for multiple locations
        let locations: HashSet<(i64, i64)> = probes
            .iter()
            .filter_map(|p| {
                match (p.lat, p.lon) {
                    (Some(lat), Some(lon)) if lat != 0.0 || lon != 0.0 => {
                        Some(((lat * 1000.0) as i64, (lon * 1000.0) as i64))
                    }
                    _ => None,
                }
            })
            .collect();

        if locations.len() > 1 {
            reasons.push(format!("Seen at {} different locations", locations.len()));
        }

        // Check for long duration
        let duration_minutes = (device.last_seen - device.first_seen) / 60;
        if duration_minutes > 30 {
            reasons.push(format!("Present for {} minutes", duration_minutes));
        }

        if reasons.is_empty() {
            reasons.push(format!("Persistence score: {:.2}", score));
        }

        reasons
    }

    pub fn get_time_window_devices(
        &self,
        db: &Database,
        window_minutes: u32,
    ) -> Result<Vec<Device>> {
        let now = chrono::Utc::now().timestamp();
        let start = now - (window_minutes as i64 * 60);
        db.get_devices_in_time_range(start, now)
    }
}

fn format_timestamp(ts: i64) -> String {
    Utc.timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "Invalid timestamp".to_string())
}

#[derive(Debug)]
pub struct TimeWindowAnalysis {
    pub window_minutes: u32,
    pub device_count: usize,
    pub devices: Vec<String>, // MAC addresses
}

pub fn analyze_time_windows(
    db: &Database,
    windows: &[u32],
) -> Result<Vec<TimeWindowAnalysis>> {
    let now = chrono::Utc::now().timestamp();
    let mut results = Vec::new();

    for &window in windows {
        let start = now - (window as i64 * 60);
        let devices = db.get_devices_in_time_range(start, now)?;

        results.push(TimeWindowAnalysis {
            window_minutes: window,
            device_count: devices.len(),
            devices: devices.iter().map(|d| d.mac.clone()).collect(),
        });
    }

    Ok(results)
}
