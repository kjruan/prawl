use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub struct Database {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct Device {
    pub id: i64,
    pub mac: String,
    pub first_seen: i64,
    pub last_seen: i64,
}

#[derive(Debug, Clone)]
pub struct Probe {
    pub id: i64,
    pub device_id: i64,
    pub ssid: String,
    pub timestamp: i64,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub signal_dbm: Option<i32>,
    pub channel: Option<u8>,
    pub distance_m: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ProbeCapture {
    pub mac: String,
    pub ssid: String,
    pub timestamp: i64,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub signal_dbm: Option<i32>,
    pub channel: Option<u8>,
    pub distance_m: Option<f64>,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path.as_ref())
            .with_context(|| format!("Failed to open database: {:?}", path.as_ref()))?;

        let db = Database { conn };
        db.initialize()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Database { conn };
        db.initialize()?;
        Ok(db)
    }

    fn initialize(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS devices (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                mac TEXT UNIQUE NOT NULL,
                first_seen INTEGER NOT NULL,
                last_seen INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS probes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                device_id INTEGER NOT NULL,
                ssid TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                lat REAL,
                lon REAL,
                signal_dbm INTEGER,
                channel INTEGER,
                distance_m REAL,
                FOREIGN KEY (device_id) REFERENCES devices(id)
            );

            CREATE INDEX IF NOT EXISTS idx_devices_mac ON devices(mac);
            CREATE INDEX IF NOT EXISTS idx_devices_last_seen ON devices(last_seen);
            CREATE INDEX IF NOT EXISTS idx_probes_timestamp ON probes(timestamp);
            CREATE INDEX IF NOT EXISTS idx_probes_ssid ON probes(ssid);
            CREATE INDEX IF NOT EXISTS idx_probes_device_id ON probes(device_id);
            "#,
        )?;

        // Migration: add distance_m column if it doesn't exist
        let _ = self.conn.execute(
            "ALTER TABLE probes ADD COLUMN distance_m REAL",
            [],
        );

        Ok(())
    }

    pub fn insert_probe(&self, capture: &ProbeCapture) -> Result<()> {
        let now = capture.timestamp;

        // Insert or update device
        let device_id: i64 = {
            // Try to get existing device
            let existing: Option<i64> = self.conn
                .query_row(
                    "SELECT id FROM devices WHERE mac = ?",
                    params![&capture.mac],
                    |row| row.get(0),
                )
                .optional()?;

            match existing {
                Some(id) => {
                    // Update last_seen
                    self.conn.execute(
                        "UPDATE devices SET last_seen = ? WHERE id = ?",
                        params![now, id],
                    )?;
                    id
                }
                None => {
                    // Insert new device
                    self.conn.execute(
                        "INSERT INTO devices (mac, first_seen, last_seen) VALUES (?, ?, ?)",
                        params![&capture.mac, now, now],
                    )?;
                    self.conn.last_insert_rowid()
                }
            }
        };

        // Insert probe
        self.conn.execute(
            "INSERT INTO probes (device_id, ssid, timestamp, lat, lon, signal_dbm, channel, distance_m)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                device_id,
                &capture.ssid,
                capture.timestamp,
                capture.lat,
                capture.lon,
                capture.signal_dbm,
                capture.channel.map(|c| c as i32),
                capture.distance_m,
            ],
        )?;

        Ok(())
    }

    pub fn get_device_by_mac(&self, mac: &str) -> Result<Option<Device>> {
        let device = self.conn
            .query_row(
                "SELECT id, mac, first_seen, last_seen FROM devices WHERE mac = ?",
                params![mac],
                |row| {
                    Ok(Device {
                        id: row.get(0)?,
                        mac: row.get(1)?,
                        first_seen: row.get(2)?,
                        last_seen: row.get(3)?,
                    })
                },
            )
            .optional()?;
        Ok(device)
    }

    pub fn get_devices_in_time_range(&self, start: i64, end: i64) -> Result<Vec<Device>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, mac, first_seen, last_seen FROM devices
             WHERE last_seen >= ? AND last_seen <= ?
             ORDER BY last_seen DESC"
        )?;

        let devices = stmt
            .query_map(params![start, end], |row| {
                Ok(Device {
                    id: row.get(0)?,
                    mac: row.get(1)?,
                    first_seen: row.get(2)?,
                    last_seen: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(devices)
    }

    pub fn get_probes_for_device(&self, device_id: i64) -> Result<Vec<Probe>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, device_id, ssid, timestamp, lat, lon, signal_dbm, channel, distance_m
             FROM probes WHERE device_id = ? ORDER BY timestamp DESC"
        )?;

        let probes = stmt
            .query_map(params![device_id], |row| {
                Ok(Probe {
                    id: row.get(0)?,
                    device_id: row.get(1)?,
                    ssid: row.get(2)?,
                    timestamp: row.get(3)?,
                    lat: row.get(4)?,
                    lon: row.get(5)?,
                    signal_dbm: row.get(6)?,
                    channel: row.get::<_, Option<i32>>(7)?.map(|c| c as u8),
                    distance_m: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(probes)
    }

    pub fn get_probes_in_time_range(&self, start: i64, end: i64) -> Result<Vec<Probe>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, device_id, ssid, timestamp, lat, lon, signal_dbm, channel, distance_m
             FROM probes WHERE timestamp >= ? AND timestamp <= ?
             ORDER BY timestamp DESC"
        )?;

        let probes = stmt
            .query_map(params![start, end], |row| {
                Ok(Probe {
                    id: row.get(0)?,
                    device_id: row.get(1)?,
                    ssid: row.get(2)?,
                    timestamp: row.get(3)?,
                    lat: row.get(4)?,
                    lon: row.get(5)?,
                    signal_dbm: row.get(6)?,
                    channel: row.get::<_, Option<i32>>(7)?.map(|c| c as u8),
                    distance_m: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(probes)
    }

    pub fn get_unique_ssids_for_device(&self, device_id: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT ssid FROM probes WHERE device_id = ? AND ssid != ''"
        )?;

        let ssids = stmt
            .query_map(params![device_id], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(ssids)
    }

    pub fn get_device_location_count(&self, device_id: i64) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT CAST(lat * 1000 AS INTEGER) || ',' || CAST(lon * 1000 AS INTEGER))
             FROM probes WHERE device_id = ? AND lat IS NOT NULL AND lon IS NOT NULL",
            params![device_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn count_devices(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM devices",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn count_probes(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM probes",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    pub fn get_all_devices(&self) -> Result<Vec<Device>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, mac, first_seen, last_seen FROM devices ORDER BY last_seen DESC"
        )?;

        let devices = stmt
            .query_map([], |row| {
                Ok(Device {
                    id: row.get(0)?,
                    mac: row.get(1)?,
                    first_seen: row.get(2)?,
                    last_seen: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(devices)
    }
}
