use libwifi::frame::components::{
    MacAddress, RsnAkmSuite, RsnCipherSuite, RsnInformation, StationInfo, VendorSpecificInfo,
    WpaAkmSuite, WpaCipherSuite, WpaInformation, WpsInformation,
};
use libwifi::frame::Frame;
use libwifi::parse_frame;
use log::{debug, trace};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ParsedProbeRequest {
    pub source_mac: String,
    pub ssid: String,
    pub signal_dbm: Option<i32>,
    pub capabilities: ProbeCapabilities,
}

/// Extracted capabilities from 802.11 probe request
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProbeCapabilities {
    pub supported_rates_mbps: Vec<f32>,
    pub extended_rates_mbps: Vec<f32>,
    pub has_ht: bool,
    pub has_vht: bool,
    pub has_he: bool,
    pub wifi_generation: String,
    pub max_rate_mbps: Option<f32>,
    pub ht_caps: Option<HtCapsSummary>,
    pub vht_caps: Option<VhtCapsSummary>,
    pub rsn_info: Option<RsnSummary>,
    pub wpa_info: Option<WpaSummary>,
    pub wps_info: Option<WpsSummary>,
    pub vendor_ies: Vec<VendorIeSummary>,
    pub ds_channel: Option<u8>,
    pub raw_ie_ids: Vec<u8>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HtCapsSummary {
    pub channel_width_40mhz: bool,
    pub short_gi_20: bool,
    pub short_gi_40: bool,
    pub tx_stbc: bool,
    pub rx_stbc: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VhtCapsSummary {
    pub max_mpdu_length: u16,
    pub supported_channel_width: u8,
    pub short_gi_80: bool,
    pub short_gi_160: bool,
    pub su_beamformer: bool,
    pub mu_beamformer: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RsnSummary {
    pub version: u16,
    pub group_cipher: String,
    pub pairwise_ciphers: Vec<String>,
    pub akm_suites: Vec<String>,
    pub mfp_required: bool,
    pub mfp_capable: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WpaSummary {
    pub version: u16,
    pub multicast_cipher: String,
    pub unicast_ciphers: Vec<String>,
    pub akm_suites: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WpsSummary {
    pub device_name: String,
    pub manufacturer: String,
    pub model: String,
    pub model_number: String,
    pub serial_number: String,
    pub device_type: String,
    pub configured: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VendorIeSummary {
    pub oui: String,
    pub oui_type: u8,
    pub vendor_name: Option<String>,
    pub data_len: usize,
}

pub fn parse_probe_request(data: &[u8], signal_dbm: Option<i32>) -> Option<ParsedProbeRequest> {
    // Skip radiotap header if present
    // Radiotap header starts with version (0) and pad (0), then length (2 bytes LE)
    let frame_data = if data.len() > 4 && data[0] == 0 {
        let radiotap_len = u16::from_le_bytes([data[2], data[3]]) as usize;
        if radiotap_len > data.len() {
            return None;
        }
        &data[radiotap_len..]
    } else {
        data
    };

    if frame_data.len() < 24 {
        return None;
    }

    // Parse the 802.11 frame (false = no FCS at end)
    match parse_frame(frame_data, false) {
        Ok(frame) => {
            match frame {
                Frame::ProbeRequest(probe_req) => {
                    // address_2 is the source address (transmitter) in probe requests
                    let source_mac = format_mac(&probe_req.header.address_2);
                    let ssid = probe_req
                        .station_info
                        .ssid
                        .clone()
                        .unwrap_or_default();

                    // Extract all capabilities
                    let capabilities = extract_capabilities(&probe_req.station_info);

                    debug!(
                        "Parsed probe request: MAC={}, SSID={:?}, WiFi={}",
                        source_mac, ssid, capabilities.wifi_generation
                    );

                    Some(ParsedProbeRequest {
                        source_mac,
                        ssid,
                        signal_dbm,
                        capabilities,
                    })
                }
                _ => {
                    trace!("Non-probe-request frame received");
                    None
                }
            }
        }
        Err(e) => {
            trace!("Failed to parse frame: {:?}", e);
            None
        }
    }
}

/// Extract all capabilities from StationInfo
fn extract_capabilities(station_info: &StationInfo) -> ProbeCapabilities {
    let mut caps = ProbeCapabilities::default();

    // Supported rates
    caps.supported_rates_mbps = station_info
        .supported_rates
        .iter()
        .map(|r| r.rate)
        .collect();

    if let Some(ext_rates) = &station_info.extended_supported_rates {
        caps.extended_rates_mbps = ext_rates.iter().map(|r| r.rate).collect();
    }

    // WiFi generation detection
    caps.has_ht = station_info.ht_capabilities.is_some();
    caps.has_vht = station_info.vht_capabilities.is_some();
    caps.has_he = check_for_he_capability(&station_info.data);

    caps.wifi_generation = determine_wifi_generation(caps.has_ht, caps.has_vht, caps.has_he);

    // Max rate calculation
    let mut all_rates: Vec<f32> = caps.supported_rates_mbps.clone();
    all_rates.extend(&caps.extended_rates_mbps);
    caps.max_rate_mbps = all_rates.iter().cloned().reduce(f32::max);

    // HT Capabilities
    if let Some(ht_raw) = &station_info.ht_capabilities {
        caps.ht_caps = Some(parse_ht_capabilities(ht_raw));
    }

    // VHT Capabilities
    if let Some(vht_raw) = &station_info.vht_capabilities {
        caps.vht_caps = Some(parse_vht_capabilities(vht_raw));
    }

    // RSN Information
    if let Some(rsn) = &station_info.rsn_information {
        caps.rsn_info = Some(extract_rsn_summary(rsn));
    }

    // WPA Information
    if let Some(wpa) = &station_info.wpa_info {
        caps.wpa_info = Some(extract_wpa_summary(wpa));
    }

    // WPS Information
    if let Some(wps) = &station_info.wps_info {
        caps.wps_info = Some(extract_wps_summary(wps));
    }

    // Vendor IEs
    caps.vendor_ies = station_info
        .vendor_specific
        .iter()
        .map(extract_vendor_ie_summary)
        .collect();

    // DS channel
    caps.ds_channel = station_info.ds_parameter_set;

    // Raw IE IDs for debugging
    caps.raw_ie_ids = station_info.data.iter().map(|(id, _)| *id).collect();

    caps
}

fn check_for_he_capability(data: &[(u8, Vec<u8>)]) -> bool {
    // HE Capabilities IE has ID 255 (Extension) with extension ID 35
    data.iter()
        .any(|(id, payload)| *id == 255 && payload.first() == Some(&35))
}

fn determine_wifi_generation(has_ht: bool, has_vht: bool, has_he: bool) -> String {
    if has_he {
        "802.11ax (WiFi 6)".to_string()
    } else if has_vht {
        "802.11ac (WiFi 5)".to_string()
    } else if has_ht {
        "802.11n (WiFi 4)".to_string()
    } else {
        "Legacy (802.11a/b/g)".to_string()
    }
}

fn parse_ht_capabilities(raw: &[u8]) -> HtCapsSummary {
    let mut summary = HtCapsSummary::default();

    if raw.len() >= 2 {
        let cap_info = u16::from_le_bytes([raw[0], raw[1]]);
        summary.channel_width_40mhz = (cap_info & 0x0002) != 0;
        summary.short_gi_20 = (cap_info & 0x0020) != 0;
        summary.short_gi_40 = (cap_info & 0x0040) != 0;
        summary.tx_stbc = (cap_info & 0x0080) != 0;
        summary.rx_stbc = ((cap_info >> 8) & 0x03) as u8;
    }

    summary
}

fn parse_vht_capabilities(raw: &[u8]) -> VhtCapsSummary {
    let mut summary = VhtCapsSummary::default();

    if raw.len() >= 4 {
        let cap_info = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
        summary.max_mpdu_length = match cap_info & 0x03 {
            0 => 3895,
            1 => 7991,
            _ => 11454,
        };
        summary.supported_channel_width = ((cap_info >> 2) & 0x03) as u8;
        summary.short_gi_80 = (cap_info & 0x20) != 0;
        summary.short_gi_160 = (cap_info & 0x40) != 0;
        summary.su_beamformer = (cap_info & 0x800) != 0;
        summary.mu_beamformer = (cap_info & 0x80000) != 0;
    }

    summary
}

fn extract_rsn_summary(rsn: &RsnInformation) -> RsnSummary {
    RsnSummary {
        version: rsn.version,
        group_cipher: format_rsn_cipher(&rsn.group_cipher_suite),
        pairwise_ciphers: rsn
            .pairwise_cipher_suites
            .iter()
            .map(format_rsn_cipher)
            .collect(),
        akm_suites: rsn.akm_suites.iter().map(format_rsn_akm).collect(),
        mfp_required: rsn.mfp_required,
        mfp_capable: rsn.mfp_capable,
    }
}

fn format_rsn_cipher(cipher: &RsnCipherSuite) -> String {
    match cipher {
        RsnCipherSuite::None => "None".to_string(),
        RsnCipherSuite::WEP => "WEP".to_string(),
        RsnCipherSuite::TKIP => "TKIP".to_string(),
        RsnCipherSuite::WRAP => "WRAP".to_string(),
        RsnCipherSuite::CCMP => "CCMP".to_string(),
        RsnCipherSuite::WEP104 => "WEP104".to_string(),
        RsnCipherSuite::Unknown(data) => format!("Unknown({:02X?})", data),
    }
}

fn format_rsn_akm(akm: &RsnAkmSuite) -> String {
    match akm {
        RsnAkmSuite::PSK => "PSK".to_string(),
        RsnAkmSuite::EAP => "EAP".to_string(),
        RsnAkmSuite::PSKFT => "PSK-FT".to_string(),
        RsnAkmSuite::EAPFT => "EAP-FT".to_string(),
        RsnAkmSuite::SAE => "SAE".to_string(),
        RsnAkmSuite::SUITEBEAP256 => "SUITE-B-EAP-256".to_string(),
        RsnAkmSuite::PSK256 => "PSK-256".to_string(),
        RsnAkmSuite::EAP256 => "EAP-256".to_string(),
        RsnAkmSuite::Unknown(data) => format!("Unknown({:02X?})", data),
    }
}

fn extract_wpa_summary(wpa: &WpaInformation) -> WpaSummary {
    WpaSummary {
        version: wpa.version,
        multicast_cipher: format_wpa_cipher(&wpa.multicast_cipher_suite),
        unicast_ciphers: wpa
            .unicast_cipher_suites
            .iter()
            .map(format_wpa_cipher)
            .collect(),
        akm_suites: wpa.akm_suites.iter().map(format_wpa_akm).collect(),
    }
}

fn format_wpa_cipher(cipher: &WpaCipherSuite) -> String {
    match cipher {
        WpaCipherSuite::Wep40 => "WEP40".to_string(),
        WpaCipherSuite::Wep104 => "WEP104".to_string(),
        WpaCipherSuite::Tkip => "TKIP".to_string(),
        WpaCipherSuite::Ccmp => "CCMP".to_string(),
        WpaCipherSuite::Unknown(data) => format!("Unknown({:02X?})", data),
    }
}

fn format_wpa_akm(akm: &WpaAkmSuite) -> String {
    match akm {
        WpaAkmSuite::Psk => "PSK".to_string(),
        WpaAkmSuite::Eap => "EAP".to_string(),
        WpaAkmSuite::Unknown(data) => format!("Unknown({:02X?})", data),
    }
}

fn extract_wps_summary(wps: &WpsInformation) -> WpsSummary {
    use libwifi::frame::components::WpsSetupState;

    WpsSummary {
        device_name: wps.device_name.clone(),
        manufacturer: wps.manufacturer.clone(),
        model: wps.model.clone(),
        model_number: wps.model_number.clone(),
        serial_number: wps.serial_number.clone(),
        device_type: wps.primary_device_type.clone(),
        configured: wps.setup_state == WpsSetupState::Configured,
    }
}

fn extract_vendor_ie_summary(vie: &VendorSpecificInfo) -> VendorIeSummary {
    let oui = format!("{:02X}:{:02X}:{:02X}", vie.oui[0], vie.oui[1], vie.oui[2]);
    let vendor_name = lookup_vendor_ie_oui(&vie.oui);

    VendorIeSummary {
        oui,
        oui_type: vie.oui_type,
        vendor_name,
        data_len: vie.data.len(),
    }
}

fn lookup_vendor_ie_oui(oui: &[u8; 3]) -> Option<String> {
    match oui {
        [0x00, 0x50, 0xF2] => Some("Microsoft".to_string()),
        [0x00, 0x0F, 0xAC] => Some("IEEE 802.11".to_string()),
        [0x00, 0x17, 0xF2] => Some("Qualcomm".to_string()),
        [0x00, 0x10, 0x18] => Some("Broadcom".to_string()),
        [0x00, 0x03, 0x7F] => Some("Atheros".to_string()),
        [0x00, 0x13, 0x74] => Some("Ralink".to_string()),
        [0x00, 0x90, 0x4C] => Some("Epigram".to_string()),
        [0x00, 0x1A, 0x11] => Some("Google".to_string()),
        [0x50, 0x6F, 0x9A] => Some("WiFi Alliance".to_string()),
        [0x00, 0x14, 0x6C] => Some("Netgear".to_string()),
        [0x00, 0x40, 0x96] => Some("Cisco".to_string()),
        [0x00, 0x0C, 0xE7] => Some("MediaTek".to_string()),
        _ => None,
    }
}

fn format_mac(mac: &MacAddress) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        mac.0[0], mac.0[1], mac.0[2], mac.0[3], mac.0[4], mac.0[5]
    )
}

/// Check if frame is a probe request by looking at frame control field
pub fn is_probe_request(data: &[u8]) -> bool {
    // Skip radiotap header if present
    let frame_data = if data.len() > 4 && data[0] == 0 {
        let radiotap_len = u16::from_le_bytes([data[2], data[3]]) as usize;
        if radiotap_len > data.len() {
            return false;
        }
        &data[radiotap_len..]
    } else {
        data
    };

    if frame_data.len() < 2 {
        return false;
    }

    // Frame control field: first 2 bytes
    // Type: bits 2-3 of first byte (Management = 0)
    // Subtype: bits 4-7 of first byte (Probe Request = 4)
    let frame_control = frame_data[0];
    let frame_type = (frame_control >> 2) & 0x03;
    let subtype = (frame_control >> 4) & 0x0F;

    // Type 0 = Management, Subtype 4 = Probe Request
    frame_type == 0 && subtype == 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_mac() {
        let mac = MacAddress([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        assert_eq!(format_mac(&mac), "AA:BB:CC:DD:EE:FF");
    }
}
