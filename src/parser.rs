use libwifi::frame::components::MacAddress;
use libwifi::frame::Frame;
use libwifi::parse_frame;
use log::{debug, trace};

#[derive(Debug, Clone)]
pub struct ParsedProbeRequest {
    pub source_mac: String,
    pub ssid: String,
    pub signal_dbm: Option<i32>,
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
                        .map(|s| s.to_string())
                        .unwrap_or_default();

                    debug!("Parsed probe request: MAC={}, SSID={:?}", source_mac, ssid);

                    Some(ParsedProbeRequest {
                        source_mac,
                        ssid,
                        signal_dbm,
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
