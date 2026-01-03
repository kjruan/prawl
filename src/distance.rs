/// Distance estimation from RSSI signal strength
///
/// Uses the Log-Distance Path Loss Model:
/// distance = 10 ^ ((tx_power - rssi) / (10 * n))
///
/// Where:
/// - tx_power: Reference signal strength at 1 meter (dBm)
/// - rssi: Measured signal strength (dBm)
/// - n: Path loss exponent (environment dependent)

use std::collections::VecDeque;

/// Distance estimate with uncertainty bounds
#[derive(Debug, Clone, Copy)]
pub struct DistanceEstimate {
    pub center: f64,
    pub min: f64,
    pub max: f64,
    pub confidence: DistanceConfidence,
}

impl DistanceEstimate {
    /// Format as "~Xm (Y-Zm)"
    pub fn format(&self) -> String {
        if self.center < 1.0 {
            format!("<1m")
        } else if self.center < 10.0 {
            format!("~{:.0}m ({:.0}-{:.0}m)", self.center, self.min, self.max)
        } else {
            format!("~{:.0}m ({:.0}-{:.0}m)", self.center, self.min, self.max)
        }
    }

    /// Get just the center value formatted
    pub fn format_center(&self) -> String {
        if self.center < 10.0 {
            format!("{:.1}m", self.center)
        } else {
            format!("{:.0}m", self.center)
        }
    }
}

/// Confidence level based on sample count
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceConfidence {
    Low,      // 1-2 samples
    Medium,   // 3-4 samples
    High,     // 5+ samples
}

impl DistanceConfidence {
    pub fn from_sample_count(count: usize) -> Self {
        match count {
            0..=2 => DistanceConfidence::Low,
            3..=4 => DistanceConfidence::Medium,
            _ => DistanceConfidence::High,
        }
    }

    pub fn indicator(&self) -> &'static str {
        match self {
            DistanceConfidence::Low => "?",
            DistanceConfidence::Medium => "~",
            DistanceConfidence::High => "",
        }
    }
}

/// RSSI sample tracker for averaging
#[derive(Debug, Clone)]
pub struct RssiTracker {
    samples: VecDeque<i32>,
    max_samples: usize,
}

impl RssiTracker {
    pub fn new(max_samples: usize) -> Self {
        RssiTracker {
            samples: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    pub fn add_sample(&mut self, rssi: i32) {
        if self.samples.len() >= self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(rssi);
    }

    /// Get weighted average (recent samples weighted more)
    pub fn weighted_average(&self) -> Option<i32> {
        if self.samples.is_empty() {
            return None;
        }

        let mut weighted_sum = 0.0;
        let mut weight_total = 0.0;

        for (i, &rssi) in self.samples.iter().enumerate() {
            // Linear weighting: older samples have lower weight
            let weight = (i + 1) as f64;
            weighted_sum += rssi as f64 * weight;
            weight_total += weight;
        }

        Some((weighted_sum / weight_total).round() as i32)
    }

    /// Get simple average
    pub fn average(&self) -> Option<i32> {
        if self.samples.is_empty() {
            return None;
        }
        let sum: i32 = self.samples.iter().sum();
        Some(sum / self.samples.len() as i32)
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    pub fn confidence(&self) -> DistanceConfidence {
        DistanceConfidence::from_sample_count(self.samples.len())
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }

    /// Get samples as a slice for statistics calculation
    pub fn samples(&self) -> Vec<i32> {
        self.samples.iter().copied().collect()
    }

    /// Compute statistics from current samples
    pub fn stats(&self) -> Option<DeviceRssiStats> {
        let samples: Vec<i32> = self.samples.iter().copied().collect();
        DeviceRssiStats::from_samples(&samples)
    }
}

impl Default for RssiTracker {
    fn default() -> Self {
        Self::new(5)
    }
}

/// Estimate TX power based on WiFi generation
///
/// Modern devices (WiFi 6) typically transmit at higher power than older devices.
/// These are reference values at 1 meter distance.
pub fn estimate_tx_power_from_wifi_gen(wifi_generation: Option<&str>) -> f64 {
    match wifi_generation.unwrap_or("") {
        s if s.contains("802.11ax") || s.contains("WiFi 6") => -38.0,
        s if s.contains("802.11ac") || s.contains("WiFi 5") => -41.0,
        s if s.contains("802.11n") || s.contains("WiFi 4") => -45.0,
        s if s.contains("802.11a") || s.contains("802.11g") => -48.0,
        s if s.contains("802.11b") => -50.0,
        _ => -43.0, // Conservative default for unknown devices
    }
}

/// Estimate distance in meters from RSSI
///
/// # Arguments
/// * `rssi_dbm` - Received signal strength in dBm (negative value)
/// * `tx_power_dbm` - Reference signal at 1 meter (typically -38 to -50 dBm)
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
    if (rssi_dbm as f64) > tx_power_dbm {
        return Some(0.5); // Assume 0.5 meters
    }

    // Log-distance path loss model
    let exponent = (tx_power_dbm - rssi_dbm as f64) / (10.0 * path_loss_exponent);
    let distance = 10.0_f64.powf(exponent);

    // Clamp to reasonable range (0.1m to 100m)
    Some(distance.clamp(0.1, 100.0))
}

/// Estimate distance with uncertainty bounds
///
/// Calculates min/max by varying the path loss exponent ±0.5
pub fn estimate_distance_range(
    rssi_dbm: i32,
    tx_power_dbm: f64,
    path_loss_exponent: f64,
    sample_count: usize,
) -> Option<DistanceEstimate> {
    let center = estimate_distance(rssi_dbm, tx_power_dbm, path_loss_exponent)?;

    // Higher path loss = closer estimate, lower = farther
    let min = estimate_distance(rssi_dbm, tx_power_dbm, path_loss_exponent + 0.5)
        .unwrap_or(center * 0.5);
    let max = estimate_distance(rssi_dbm, tx_power_dbm, path_loss_exponent - 0.5)
        .unwrap_or(center * 2.0);

    Some(DistanceEstimate {
        center,
        min: min.max(0.1),
        max: max.min(100.0),
        confidence: DistanceConfidence::from_sample_count(sample_count),
    })
}

/// Smart distance estimation using WiFi generation
///
/// Combines WiFi generation-based TX power estimation with uncertainty bounds
pub fn estimate_distance_smart(
    rssi_dbm: i32,
    wifi_generation: Option<&str>,
    path_loss_exponent: f64,
    sample_count: usize,
    calibrated_tx_power: Option<f64>,
) -> Option<DistanceEstimate> {
    // Use calibrated value if available, otherwise estimate from WiFi gen
    let tx_power = calibrated_tx_power
        .unwrap_or_else(|| estimate_tx_power_from_wifi_gen(wifi_generation));

    estimate_distance_range(rssi_dbm, tx_power, path_loss_exponent, sample_count)
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
        d if d < 3.0 => "🔴",  // Very close - high concern
        d if d < 10.0 => "🟠", // Close - moderate concern
        d if d < 20.0 => "🟡", // Nearby - low concern
        _ => "🟢",             // Far - minimal concern
    }
}

/// Format distance for display (legacy single-value format)
pub fn format_distance(distance_m: f64) -> String {
    if distance_m < 10.0 {
        format!("{:.1}m", distance_m)
    } else {
        format!("{:.0}m", distance_m)
    }
}

/// Format distance estimate with uncertainty
pub fn format_distance_range(estimate: &DistanceEstimate) -> String {
    estimate.format()
}

/// Typical path loss exponents for different environments
pub mod environments {
    pub const FREE_SPACE: f64 = 2.0;
    pub const OPEN_INDOOR: f64 = 2.5;
    pub const TYPICAL_INDOOR: f64 = 3.0;
    pub const DENSE_INDOOR: f64 = 3.5;
    pub const HEAVY_OBSTACLES: f64 = 4.0;
}

/// Typical TX power values for different device types (at 1m reference)
pub mod tx_power {
    pub const WIFI6_DEVICE: f64 = -38.0;
    pub const WIFI5_DEVICE: f64 = -41.0;
    pub const WIFI4_DEVICE: f64 = -45.0;
    pub const LEGACY_DEVICE: f64 = -50.0;
    pub const DEFAULT: f64 = -43.0;

    // Legacy names for compatibility
    pub const SMARTPHONE_HIGH: f64 = -40.0;
    pub const SMARTPHONE_TYPICAL: f64 = -43.0;
    pub const SMARTPHONE_LOW: f64 = -48.0;
    pub const LAPTOP: f64 = -43.0;
    pub const IOT_DEVICE: f64 = -50.0;
    pub const TRACKER: f64 = -48.0;
}

/// Calibration result from measuring at known distance
#[derive(Debug, Clone)]
pub struct CalibrationResult {
    pub measured_rssi_avg: i32,
    pub known_distance_m: f64,
    pub calculated_tx_power: f64,
    pub sample_count: usize,
}

/// Calculate TX power from calibration measurement
///
/// Given a known distance and measured RSSI, back-calculate the TX power
pub fn calibrate_tx_power(
    measured_rssi_avg: i32,
    known_distance_m: f64,
    path_loss_exponent: f64,
) -> Option<CalibrationResult> {
    if known_distance_m <= 0.0 || path_loss_exponent <= 0.0 {
        return None;
    }

    // Rearranging: distance = 10^((tx - rssi)/(10*n))
    // log10(distance) = (tx - rssi) / (10 * n)
    // tx = rssi + 10 * n * log10(distance)
    let tx_power = measured_rssi_avg as f64 + 10.0 * path_loss_exponent * known_distance_m.log10();

    Some(CalibrationResult {
        measured_rssi_avg,
        known_distance_m,
        calculated_tx_power: tx_power,
        sample_count: 1,
    })
}

/// Statistics for a single device's RSSI observations
#[derive(Debug, Clone, Default)]
pub struct DeviceRssiStats {
    pub sample_count: usize,
    pub mean_rssi: f64,
    pub variance: f64,
    pub min_rssi: i32,
    pub max_rssi: i32,
}

impl DeviceRssiStats {
    pub fn from_samples(samples: &[i32]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }

        let n = samples.len();
        let sum: i64 = samples.iter().map(|&x| x as i64).sum();
        let mean = sum as f64 / n as f64;

        let variance = if n > 1 {
            samples.iter()
                .map(|&x| (x as f64 - mean).powi(2))
                .sum::<f64>() / (n - 1) as f64
        } else {
            0.0
        };

        Some(DeviceRssiStats {
            sample_count: n,
            mean_rssi: mean,
            variance,
            min_rssi: *samples.iter().min().unwrap(),
            max_rssi: *samples.iter().max().unwrap(),
        })
    }

    /// Check if device appears stationary (low RSSI variance)
    pub fn is_stationary(&self) -> bool {
        // Standard deviation < 5 dBm suggests stationary
        self.sample_count >= 5 && self.variance.sqrt() < 5.0
    }

    /// Get standard deviation
    pub fn std_dev(&self) -> f64 {
        self.variance.sqrt()
    }
}

/// Adaptive path loss calibrator
///
/// Learns optimal path_loss_exponent by analyzing RSSI patterns from
/// stationary devices. The key insight is that for truly stationary devices,
/// distance estimates should be stable. If RSSI is stable but distance
/// estimates vary wildly, the path loss exponent needs adjustment.
#[derive(Debug, Clone)]
pub struct AdaptiveCalibrator {
    /// Current path loss exponent
    current_path_loss: f64,
    /// Accumulated evidence for adjustment
    adjustment_accumulator: f64,
    /// Number of observations contributing to adjustment
    observation_count: usize,
    /// Learning rate (how fast to adapt)
    learning_rate: f64,
    /// Minimum path loss (physical lower bound)
    min_path_loss: f64,
    /// Maximum path loss (physical upper bound)
    max_path_loss: f64,
    /// Peak RSSI observed (for TX power estimation)
    peak_rssi: i32,
    /// Count of peak observations
    peak_count: usize,
}

impl AdaptiveCalibrator {
    pub fn new(initial_path_loss: f64) -> Self {
        AdaptiveCalibrator {
            current_path_loss: initial_path_loss,
            adjustment_accumulator: 0.0,
            observation_count: 0,
            learning_rate: 0.01, // Slow adaptation
            min_path_loss: 2.0,  // Free space
            max_path_loss: 5.0,  // Very obstructed
            peak_rssi: i32::MIN,
            peak_count: 0,
        }
    }

    /// Record a strong signal observation (for TX power estimation)
    pub fn record_peak_rssi(&mut self, rssi: i32) {
        // Only consider very strong signals as "peak" (close devices)
        if rssi > -45 {
            if rssi > self.peak_rssi {
                self.peak_rssi = rssi;
                self.peak_count = 1;
            } else if rssi >= self.peak_rssi - 3 {
                // Within 3 dBm of peak
                self.peak_count += 1;
            }
        }
    }

    /// Get inferred TX power from peak observations
    ///
    /// Very strong signals (> -40 dBm) likely come from devices < 1m away,
    /// so peak_rssi approximates tx_power at 1m reference distance.
    pub fn inferred_tx_power(&self) -> Option<f64> {
        if self.peak_count >= 3 && self.peak_rssi > i32::MIN {
            // Add small offset since peak is likely from < 1m
            Some(self.peak_rssi as f64 - 3.0)
        } else {
            None
        }
    }

    /// Analyze a stationary device's RSSI pattern and suggest path loss adjustment
    ///
    /// Logic:
    /// - If RSSI is very stable but estimated distance would swing a lot,
    ///   path_loss is too low (need higher to dampen sensitivity)
    /// - If RSSI varies but distance stays constant, path_loss might be too high
    pub fn analyze_device(&mut self, stats: &DeviceRssiStats, tx_power: f64) {
        if !stats.is_stationary() || stats.sample_count < 10 {
            return;
        }

        // Calculate distance sensitivity at this RSSI level
        // d(distance)/d(rssi) ∝ distance / (10 * n)
        // Lower n = higher sensitivity = distances swing more per dBm change

        let rssi = stats.mean_rssi as i32;
        let current_distance = estimate_distance(rssi, tx_power, self.current_path_loss)
            .unwrap_or(1.0);

        // Expected distance variance given RSSI variance
        // Using linear approximation: var(d) ≈ (∂d/∂rssi)² * var(rssi)
        let sensitivity = current_distance * (10.0_f64).ln() / (10.0 * self.current_path_loss);
        let expected_distance_std = sensitivity * stats.std_dev();

        // If expected distance variation is > 50% of distance, path_loss is too low
        let relative_variation = expected_distance_std / current_distance.max(0.1);

        // Target: relative variation should be around 20-30% for "normal" uncertainty
        let target_variation = 0.25;

        if relative_variation > target_variation * 1.5 {
            // Too much variation, increase path_loss
            self.adjustment_accumulator += 0.1;
        } else if relative_variation < target_variation * 0.5 {
            // Too little variation (distances too compressed), decrease path_loss
            self.adjustment_accumulator -= 0.1;
        }

        self.observation_count += 1;

        // Apply accumulated adjustment periodically
        if self.observation_count >= 10 {
            self.apply_adjustment();
        }
    }

    /// Apply accumulated adjustments
    fn apply_adjustment(&mut self) {
        if self.observation_count == 0 {
            return;
        }

        let avg_adjustment = self.adjustment_accumulator / self.observation_count as f64;
        let delta = avg_adjustment * self.learning_rate;

        self.current_path_loss = (self.current_path_loss + delta)
            .clamp(self.min_path_loss, self.max_path_loss);

        // Reset accumulators
        self.adjustment_accumulator = 0.0;
        self.observation_count = 0;
    }

    /// Force application of pending adjustments
    pub fn flush(&mut self) {
        self.apply_adjustment();
    }

    /// Get current calibrated path loss exponent
    pub fn path_loss(&self) -> f64 {
        self.current_path_loss
    }

    /// Get calibration status for display
    pub fn status(&self) -> CalibrationStatus {
        CalibrationStatus {
            path_loss_exponent: self.current_path_loss,
            peak_rssi: if self.peak_rssi > i32::MIN { Some(self.peak_rssi) } else { None },
            inferred_tx_power: self.inferred_tx_power(),
            observation_count: self.observation_count,
        }
    }
}

impl Default for AdaptiveCalibrator {
    fn default() -> Self {
        Self::new(3.0)
    }
}

/// Status of adaptive calibration for display
#[derive(Debug, Clone)]
pub struct CalibrationStatus {
    pub path_loss_exponent: f64,
    pub peak_rssi: Option<i32>,
    pub inferred_tx_power: Option<f64>,
    pub observation_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_distance() {
        // At tx_power, should be ~1 meter
        let d = estimate_distance(-43, -43.0, 3.0).unwrap();
        assert!((d - 1.0).abs() < 0.1);

        // Weaker signal = farther
        let d = estimate_distance(-60, -43.0, 3.0).unwrap();
        assert!(d > 3.0);

        // Stronger signal = closer
        let d = estimate_distance(-35, -43.0, 3.0).unwrap();
        assert!(d < 1.0);
    }

    #[test]
    fn test_distance_range() {
        let range = estimate_distance_range(-55, -43.0, 3.0, 5).unwrap();
        assert!(range.min < range.center);
        assert!(range.center < range.max);
        assert_eq!(range.confidence, DistanceConfidence::High);
    }

    #[test]
    fn test_rssi_tracker() {
        let mut tracker = RssiTracker::new(5);
        tracker.add_sample(-50);
        tracker.add_sample(-52);
        tracker.add_sample(-48);

        let avg = tracker.average().unwrap();
        assert_eq!(avg, -50);

        assert_eq!(tracker.confidence(), DistanceConfidence::Medium);
    }

    #[test]
    fn test_wifi_gen_tx_power() {
        assert_eq!(estimate_tx_power_from_wifi_gen(Some("802.11ax (WiFi 6)")), -38.0);
        assert_eq!(estimate_tx_power_from_wifi_gen(Some("802.11ac (WiFi 5)")), -41.0);
        assert_eq!(estimate_tx_power_from_wifi_gen(Some("802.11n (WiFi 4)")), -45.0);
        assert_eq!(estimate_tx_power_from_wifi_gen(None), -43.0);
    }

    #[test]
    fn test_calibration() {
        // At 3m with RSSI -55, what TX power gives us that?
        let result = calibrate_tx_power(-55, 3.0, 3.0).unwrap();
        // TX = -55 + 10 * 3.0 * log10(3) ≈ -55 + 14.3 ≈ -40.7
        assert!((result.calculated_tx_power - (-40.7)).abs() < 0.5);
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
        assert!(estimate_distance(10, -43.0, 3.0).is_none()); // Positive RSSI invalid
        assert!(estimate_distance(-50, -43.0, 0.0).is_none()); // Zero exponent invalid
    }
}
