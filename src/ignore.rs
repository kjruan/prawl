use anyhow::{Context, Result};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct IgnoreLists {
    mac_list: HashSet<String>,
    ssid_list: HashSet<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MacListFile {
    macs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SsidListFile {
    ssids: Vec<String>,
}

impl IgnoreLists {
    pub fn new() -> Self {
        IgnoreLists::default()
    }

    pub fn load<P: AsRef<Path>>(mac_path: P, ssid_path: P) -> Result<Self> {
        let mut lists = IgnoreLists::new();

        // Load MAC list
        if mac_path.as_ref().exists() {
            match load_mac_list(mac_path.as_ref()) {
                Ok(macs) => {
                    info!("Loaded {} MAC addresses to ignore", macs.len());
                    lists.mac_list = macs;
                }
                Err(e) => {
                    warn!("Failed to load MAC ignore list: {}", e);
                }
            }
        } else {
            debug!("MAC ignore list not found: {:?}", mac_path.as_ref());
        }

        // Load SSID list
        if ssid_path.as_ref().exists() {
            match load_ssid_list(ssid_path.as_ref()) {
                Ok(ssids) => {
                    info!("Loaded {} SSIDs to ignore", ssids.len());
                    lists.ssid_list = ssids;
                }
                Err(e) => {
                    warn!("Failed to load SSID ignore list: {}", e);
                }
            }
        } else {
            debug!("SSID ignore list not found: {:?}", ssid_path.as_ref());
        }

        Ok(lists)
    }

    pub fn should_ignore_mac(&self, mac: &str) -> bool {
        // Normalize MAC address for comparison
        let normalized = mac.to_uppercase().replace(['-', '.'], ":");
        self.mac_list.contains(&normalized)
    }

    pub fn should_ignore_ssid(&self, ssid: &str) -> bool {
        self.ssid_list.contains(ssid)
    }

    pub fn add_mac(&mut self, mac: &str) {
        let normalized = mac.to_uppercase().replace(['-', '.'], ":");
        self.mac_list.insert(normalized);
    }

    pub fn add_ssid(&mut self, ssid: &str) {
        self.ssid_list.insert(ssid.to_string());
    }

    pub fn remove_mac(&mut self, mac: &str) -> bool {
        let normalized = mac.to_uppercase().replace(['-', '.'], ":");
        self.mac_list.remove(&normalized)
    }

    pub fn remove_ssid(&mut self, ssid: &str) -> bool {
        self.ssid_list.remove(ssid)
    }

    pub fn mac_count(&self) -> usize {
        self.mac_list.len()
    }

    pub fn ssid_count(&self) -> usize {
        self.ssid_list.len()
    }

    pub fn save_mac_list<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let file = MacListFile {
            macs: self.mac_list.iter().cloned().collect(),
        };
        let content = serde_json::to_string_pretty(&file)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn save_ssid_list<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let file = SsidListFile {
            ssids: self.ssid_list.iter().cloned().collect(),
        };
        let content = serde_json::to_string_pretty(&file)?;
        fs::write(path, content)?;
        Ok(())
    }
}

fn load_mac_list(path: &Path) -> Result<HashSet<String>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read MAC list: {:?}", path))?;

    let file: MacListFile = serde_json::from_str(&content)
        .with_context(|| "Failed to parse MAC list JSON")?;

    // Normalize all MAC addresses
    let macs: HashSet<String> = file
        .macs
        .into_iter()
        .map(|m| m.to_uppercase().replace(['-', '.'], ":"))
        .collect();

    Ok(macs)
}

fn load_ssid_list(path: &Path) -> Result<HashSet<String>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read SSID list: {:?}", path))?;

    let file: SsidListFile = serde_json::from_str(&content)
        .with_context(|| "Failed to parse SSID list JSON")?;

    Ok(file.ssids.into_iter().collect())
}

/// Create default ignore list files if they don't exist
pub fn create_default_ignore_lists<P: AsRef<Path>>(dir: P) -> Result<()> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir)?;

    let mac_path = dir.join("mac_list.json");
    if !mac_path.exists() {
        let content = r#"{
  "macs": []
}"#;
        fs::write(&mac_path, content)?;
        info!("Created default MAC ignore list: {:?}", mac_path);
    }

    let ssid_path = dir.join("ssid_list.json");
    if !ssid_path.exists() {
        let content = r#"{
  "ssids": []
}"#;
        fs::write(&ssid_path, content)?;
        info!("Created default SSID ignore list: {:?}", ssid_path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mac_normalization() {
        let mut lists = IgnoreLists::new();
        lists.add_mac("aa:bb:cc:dd:ee:ff");

        assert!(lists.should_ignore_mac("AA:BB:CC:DD:EE:FF"));
        assert!(lists.should_ignore_mac("aa:bb:cc:dd:ee:ff"));
        assert!(lists.should_ignore_mac("AA-BB-CC-DD-EE-FF"));
        assert!(!lists.should_ignore_mac("11:22:33:44:55:66"));
    }

    #[test]
    fn test_ssid_matching() {
        let mut lists = IgnoreLists::new();
        lists.add_ssid("MyHomeNetwork");

        assert!(lists.should_ignore_ssid("MyHomeNetwork"));
        assert!(!lists.should_ignore_ssid("myhomenetwork")); // Case sensitive
        assert!(!lists.should_ignore_ssid("OtherNetwork"));
    }
}
