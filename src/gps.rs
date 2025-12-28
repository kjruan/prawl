use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

pub struct GpsClient {
    host: String,
    port: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct GpsPosition {
    pub lat: f64,
    pub lon: f64,
    pub alt: Option<f64>,
    pub speed: Option<f64>,
    pub timestamp: i64,
}

impl GpsClient {
    pub fn new(host: String, port: u16) -> Self {
        GpsClient { host, port }
    }

    pub async fn run(
        &self,
        tx: mpsc::Sender<(f64, f64)>,
        running: Arc<AtomicBool>,
    ) -> Result<()> {
        info!("Connecting to gpsd at {}:{}", self.host, self.port);

        loop {
            if !running.load(Ordering::SeqCst) {
                break;
            }

            match self.connect_and_read(&tx, &running) {
                Ok(_) => {}
                Err(e) => {
                    warn!("GPS connection error: {}, retrying in 5s", e);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }

        info!("GPS client stopped");
        Ok(())
    }

    fn connect_and_read(
        &self,
        tx: &mpsc::Sender<(f64, f64)>,
        running: &Arc<AtomicBool>,
    ) -> Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        let mut stream = TcpStream::connect(&addr)
            .context("Failed to connect to gpsd")?;

        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;

        // Enable JSON watch mode
        stream.write_all(b"?WATCH={\"enable\":true,\"json\":true}\n")?;
        stream.flush()?;

        info!("Connected to gpsd, waiting for position data");

        let reader = BufReader::new(stream);

        for line in reader.lines() {
            if !running.load(Ordering::SeqCst) {
                break;
            }

            match line {
                Ok(json) => {
                    if let Some(pos) = parse_gpsd_json(&json) {
                        debug!("GPS: lat={}, lon={}", pos.lat, pos.lon);
                        if tx.blocking_send((pos.lat, pos.lon)).is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut
                    {
                        continue;
                    }
                    return Err(e.into());
                }
            }
        }

        Ok(())
    }
}

fn parse_gpsd_json(json: &str) -> Option<GpsPosition> {
    // Simple JSON parsing for gpsd TPV (Time-Position-Velocity) messages
    // Format: {"class":"TPV","lat":..., "lon":..., ...}

    if !json.contains("\"class\":\"TPV\"") {
        return None;
    }

    let lat = extract_number(json, "\"lat\":")?;
    let lon = extract_number(json, "\"lon\":")?;

    // Validate coordinates
    if lat < -90.0 || lat > 90.0 || lon < -180.0 || lon > 180.0 {
        return None;
    }

    // Ignore invalid/zero coordinates
    if lat == 0.0 && lon == 0.0 {
        return None;
    }

    let alt = extract_number(json, "\"alt\":");
    let speed = extract_number(json, "\"speed\":");

    Some(GpsPosition {
        lat,
        lon,
        alt,
        speed,
        timestamp: chrono::Utc::now().timestamp(),
    })
}

fn extract_number(json: &str, key: &str) -> Option<f64> {
    let start = json.find(key)? + key.len();
    let rest = &json[start..];

    // Find end of number (comma, }, or end of string)
    let end = rest
        .find(|c: char| c == ',' || c == '}' || c == ' ')
        .unwrap_or(rest.len());

    rest[..end].trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gpsd_json() {
        let json = r#"{"class":"TPV","device":"/dev/ttyACM0","mode":3,"time":"2024-01-15T10:30:00.000Z","ept":0.005,"lat":33.4484,"lon":-112.0740,"alt":350.0,"epx":10.0,"epy":10.0,"epv":15.0,"track":90.0,"speed":0.0,"climb":0.0}"#;

        let pos = parse_gpsd_json(json).unwrap();
        assert!((pos.lat - 33.4484).abs() < 0.0001);
        assert!((pos.lon - (-112.0740)).abs() < 0.0001);
        assert!((pos.alt.unwrap() - 350.0).abs() < 0.1);
    }

    #[test]
    fn test_parse_gpsd_json_non_tpv() {
        let json = r#"{"class":"VERSION","release":"3.24","rev":"3.24"}"#;
        assert!(parse_gpsd_json(json).is_none());
    }
}
