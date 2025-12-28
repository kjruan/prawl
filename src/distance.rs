/// Distance estimation from RSSI signal strength
///
/// Uses the Log-Distance Path Loss Model:
/// distance = 10 ^ ((tx_power - rssi) / (10 * n))
///
/// Where:
/// - tx_power: Reference signal strength at 1 meter (dBm)
/// - rssi: Measured signal strength (dBm)
/// - n: Path loss exponent (environment dependent)

/// Estimate distance in meters from RSSI
///
/// # Arguments
/// * `rssi_dbm` - Received signal strength in dBm (negative value)
/// * `tx_power_dbm` - Reference signal at 1 meter (typically -40 to -50 dBm)
/// * `path_loss_exponent` - Environment factor (2.0=free space, 3.0=indoor, 4.0=dense obstacles)
///
/// # Returns
/// Estimated distance in meters, or None if RSSI is invalid
pub fn estimate_distance(rssi_dbm: i32, tx_power_dbm: f64, path_loss_exponent: f64) -> Option<f64> {
    // Validate inputs
    if rssi_dbm > 0 || path_loss_exponent <= 0.0 {
        return None;
    }

    // If RSSI is stronger than tx_power, device is very close (< 1 meter)
    if (rssi_dbm as f64) >= tx_power_dbm {
        return Some(0.5); // Assume 0.5 meters
    }

    // Log-distance path loss model
    let exponent = (tx_power_dbm - rssi_dbm as f64) / (10.0 * path_loss_exponent);
    let distance = 10.0_f64.powf(exponent);

    // Clamp to reasonable range (0.1m to 100m)
    Some(distance.clamp(0.1, 100.0))
}

/// Get a human-readable distance category
pub fn distance_category(distance_m: f64) -> &'static str {
    match distance_m {
        d if d < 1.0 => "immediate (<1m)",
        d if d < 3.0 => "very close (1-3m)",
        d if d < 10.0 => "close (3-10m)",
        d if d < 20.0 => "nearby (10-20m)",
        d if d < 40.0 => "far (20-40m)",
        _ => "very far (>40m)",
    }
}

/// Get a threat level indicator based on distance
pub fn distance_threat_indicator(distance_m: f64) -> &'static str {
    match distance_m {
        d if d < 3.0 => "🔴",   // Very close - high concern
        d if d < 10.0 => "🟠",  // Close - moderate concern
        d if d < 20.0 => "🟡",  // Nearby - low concern
        _ => "🟢",              // Far - minimal concern
    }
}

/// Format distance for display
pub fn format_distance(distance_m: f64) -> String {
    if distance_m < 1.0 {
        format!("{:.1}m", distance_m)
    } else if distance_m < 10.0 {
        format!("{:.1}m", distance_m)
    } else {
        format!("{:.0}m", distance_m)
    }
}

/// Typical path loss exponents for different environments
pub mod environments {
    pub const FREE_SPACE: f64 = 2.0;
    pub const OPEN_INDOOR: f64 = 2.5;
    pub const TYPICAL_INDOOR: f64 = 3.0;
    pub const DENSE_INDOOR: f64 = 3.5;
    pub const HEAVY_OBSTACLES: f64 = 4.0;
}

/// Typical TX power values for different device types
pub mod tx_power {
    pub const SMARTPHONE_HIGH: f64 = -40.0;
    pub const SMARTPHONE_TYPICAL: f64 = -45.0;
    pub const SMARTPHONE_LOW: f64 = -50.0;
    pub const LAPTOP: f64 = -45.0;
    pub const IOT_DEVICE: f64 = -55.0;
    pub const TRACKER: f64 = -50.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_distance() {
        // At tx_power, should be ~1 meter
        let d = estimate_distance(-45, -45.0, 3.0).unwrap();
        assert!((d - 1.0).abs() < 0.1);

        // Weaker signal = farther
        let d = estimate_distance(-60, -45.0, 3.0).unwrap();
        assert!(d > 3.0);

        // Stronger signal = closer
        let d = estimate_distance(-35, -45.0, 3.0).unwrap();
        assert!(d < 1.0);
    }

    #[test]
    fn test_distance_category() {
        assert_eq!(distance_category(0.5), "immediate (<1m)");
        assert_eq!(distance_category(2.0), "very close (1-3m)");
        assert_eq!(distance_category(5.0), "close (3-10m)");
        assert_eq!(distance_category(15.0), "nearby (10-20m)");
        assert_eq!(distance_category(30.0), "far (20-40m)");
        assert_eq!(distance_category(50.0), "very far (>40m)");
    }

    #[test]
    fn test_invalid_inputs() {
        assert!(estimate_distance(10, -45.0, 3.0).is_none()); // Positive RSSI invalid
        assert!(estimate_distance(-50, -45.0, 0.0).is_none()); // Zero exponent invalid
    }
}
