use crate::analysis::SurveillanceAlert;
use crate::database::Database;
use anyhow::Result;
use chrono::{TimeZone, Utc};
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

pub struct ReportGenerator;

impl ReportGenerator {
    pub fn generate_surveillance_report(
        alerts: &[SurveillanceAlert],
        output: Option<&Path>,
    ) -> Result<()> {
        let mut writer: Box<dyn Write> = match output {
            Some(path) => Box::new(File::create(path)?),
            None => Box::new(io::stdout()),
        };

        writeln!(writer, "========================================")?;
        writeln!(writer, "   PROWL SURVEILLANCE ANALYSIS REPORT")?;
        writeln!(writer, "========================================")?;
        writeln!(writer)?;
        writeln!(
            writer,
            "Generated: {}",
            Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        )?;
        writeln!(writer, "Suspicious devices found: {}", alerts.len())?;
        writeln!(writer)?;

        if alerts.is_empty() {
            writeln!(writer, "No suspicious devices detected.")?;
            return Ok(());
        }

        for (i, alert) in alerts.iter().enumerate() {
            writeln!(writer, "----------------------------------------")?;
            writeln!(writer, "Device #{}: {}", i + 1, alert.device.mac)?;
            writeln!(writer, "----------------------------------------")?;
            writeln!(writer, "  Persistence Score: {:.2}%", alert.score * 100.0)?;
            writeln!(
                writer,
                "  First Seen: {}",
                format_timestamp(alert.device.first_seen)
            )?;
            writeln!(
                writer,
                "  Last Seen:  {}",
                format_timestamp(alert.device.last_seen)
            )?;
            writeln!(writer, "  Appearances: {}", alert.appearance_count)?;
            writeln!(writer, "  Locations: {}", alert.location_count)?;
            writeln!(writer)?;

            if !alert.probed_ssids.is_empty() {
                writeln!(writer, "  Probed SSIDs:")?;
                for ssid in &alert.probed_ssids {
                    writeln!(writer, "    - {}", ssid)?;
                }
                writeln!(writer)?;
            }

            writeln!(writer, "  Alert Reasons:")?;
            for reason in &alert.reasons {
                writeln!(writer, "    * {}", reason)?;
            }
            writeln!(writer)?;
        }

        writeln!(writer, "========================================")?;
        writeln!(writer, "              END OF REPORT")?;
        writeln!(writer, "========================================")?;

        Ok(())
    }

    pub fn generate_device_list(db: &Database, output: Option<&Path>) -> Result<()> {
        let mut writer: Box<dyn Write> = match output {
            Some(path) => Box::new(File::create(path)?),
            None => Box::new(io::stdout()),
        };

        let devices = db.get_all_devices()?;

        writeln!(writer, "MAC Address          | First Seen           | Last Seen            | Probes")?;
        writeln!(writer, "---------------------|----------------------|----------------------|-------")?;

        for device in &devices {
            let probes = db.get_probes_for_device(device.id)?;
            writeln!(
                writer,
                "{} | {} | {} | {}",
                device.mac,
                format_timestamp(device.first_seen),
                format_timestamp(device.last_seen),
                probes.len()
            )?;
        }

        writeln!(writer)?;
        writeln!(writer, "Total devices: {}", devices.len())?;
        writeln!(writer, "Total probes: {}", db.count_probes()?)?;

        Ok(())
    }

    pub fn generate_stats(db: &Database) -> Result<()> {
        let device_count = db.count_devices()?;
        let probe_count = db.count_probes()?;

        println!("Database Statistics");
        println!("-------------------");
        println!("Total devices: {}", device_count);
        println!("Total probes:  {}", probe_count);

        if device_count > 0 {
            let devices = db.get_all_devices()?;

            // Find time range
            let first = devices.iter().map(|d| d.first_seen).min().unwrap_or(0);
            let last = devices.iter().map(|d| d.last_seen).max().unwrap_or(0);

            println!();
            println!("Time Range");
            println!("----------");
            println!("First seen: {}", format_timestamp(first));
            println!("Last seen:  {}", format_timestamp(last));

            let duration_hours = (last - first) as f64 / 3600.0;
            if duration_hours > 0.0 {
                println!();
                println!("Averages");
                println!("--------");
                println!("Probes/hour: {:.2}", probe_count as f64 / duration_hours);
                println!("Devices/hour: {:.2}", device_count as f64 / duration_hours);
            }
        }

        Ok(())
    }
}

fn format_timestamp(ts: i64) -> String {
    Utc.timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "N/A".to_string())
}

pub fn print_probe_realtime(mac: &str, ssid: &str, signal: Option<i32>) {
    let timestamp = Utc::now().format("%H:%M:%S");
    let ssid_display = if ssid.is_empty() { "<broadcast>" } else { ssid };
    let signal_display = signal
        .map(|s| format!("{:4}dBm", s))
        .unwrap_or_else(|| "   N/A".to_string());

    println!(
        "[{}] {} | {} | {}",
        timestamp, mac, signal_display, ssid_display
    );
}
