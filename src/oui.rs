//! OUI (Organizationally Unique Identifier) lookup for MAC address vendor identification
//! and MAC randomization detection.

use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Common OUI prefixes mapped to vendor names
/// This is a subset of the IEEE OUI database for common device manufacturers
static OUI_DATABASE: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();

    // Apple
    m.insert("00:03:93", "Apple");
    m.insert("00:0A:27", "Apple");
    m.insert("00:0A:95", "Apple");
    m.insert("00:0D:93", "Apple");
    m.insert("00:10:FA", "Apple");
    m.insert("00:11:24", "Apple");
    m.insert("00:14:51", "Apple");
    m.insert("00:16:CB", "Apple");
    m.insert("00:17:F2", "Apple");
    m.insert("00:19:E3", "Apple");
    m.insert("00:1B:63", "Apple");
    m.insert("00:1C:B3", "Apple");
    m.insert("00:1D:4F", "Apple");
    m.insert("00:1E:52", "Apple");
    m.insert("00:1E:C2", "Apple");
    m.insert("00:1F:5B", "Apple");
    m.insert("00:1F:F3", "Apple");
    m.insert("00:21:E9", "Apple");
    m.insert("00:22:41", "Apple");
    m.insert("00:23:12", "Apple");
    m.insert("00:23:32", "Apple");
    m.insert("00:23:6C", "Apple");
    m.insert("00:23:DF", "Apple");
    m.insert("00:24:36", "Apple");
    m.insert("00:25:00", "Apple");
    m.insert("00:25:4B", "Apple");
    m.insert("00:25:BC", "Apple");
    m.insert("00:26:08", "Apple");
    m.insert("00:26:4A", "Apple");
    m.insert("00:26:B0", "Apple");
    m.insert("00:26:BB", "Apple");
    m.insert("00:30:65", "Apple");
    m.insert("00:3E:E1", "Apple");
    m.insert("00:50:E4", "Apple");
    m.insert("00:56:CD", "Apple");
    m.insert("00:61:71", "Apple");
    m.insert("00:6D:52", "Apple");
    m.insert("00:88:65", "Apple");
    m.insert("00:B3:62", "Apple");
    m.insert("00:C6:10", "Apple");
    m.insert("00:CD:FE", "Apple");
    m.insert("00:DB:70", "Apple");
    m.insert("00:F4:B9", "Apple");
    m.insert("00:F7:6F", "Apple");
    m.insert("04:0C:CE", "Apple");
    m.insert("04:15:52", "Apple");
    m.insert("04:26:65", "Apple");
    m.insert("04:48:9A", "Apple");
    m.insert("04:4B:ED", "Apple");
    m.insert("04:52:F3", "Apple");
    m.insert("04:54:53", "Apple");
    m.insert("04:69:F8", "Apple");
    m.insert("04:D3:CF", "Apple");
    m.insert("04:DB:56", "Apple");
    m.insert("04:E5:36", "Apple");
    m.insert("04:F1:3E", "Apple");
    m.insert("04:F7:E4", "Apple");

    // Samsung
    m.insert("00:00:F0", "Samsung");
    m.insert("00:02:78", "Samsung");
    m.insert("00:07:AB", "Samsung");
    m.insert("00:09:18", "Samsung");
    m.insert("00:0D:AE", "Samsung");
    m.insert("00:0D:E5", "Samsung");
    m.insert("00:12:47", "Samsung");
    m.insert("00:12:FB", "Samsung");
    m.insert("00:13:77", "Samsung");
    m.insert("00:15:99", "Samsung");
    m.insert("00:15:B9", "Samsung");
    m.insert("00:16:32", "Samsung");
    m.insert("00:16:6B", "Samsung");
    m.insert("00:16:6C", "Samsung");
    m.insert("00:16:DB", "Samsung");
    m.insert("00:17:C9", "Samsung");
    m.insert("00:17:D5", "Samsung");
    m.insert("00:18:AF", "Samsung");
    m.insert("00:1A:8A", "Samsung");
    m.insert("00:1B:98", "Samsung");
    m.insert("00:1C:43", "Samsung");
    m.insert("00:1D:25", "Samsung");
    m.insert("00:1D:F6", "Samsung");
    m.insert("00:1E:7D", "Samsung");
    m.insert("00:1F:CC", "Samsung");
    m.insert("00:1F:CD", "Samsung");
    m.insert("00:21:19", "Samsung");
    m.insert("00:21:4C", "Samsung");
    m.insert("00:21:D1", "Samsung");
    m.insert("00:21:D2", "Samsung");
    m.insert("00:23:39", "Samsung");
    m.insert("00:23:3A", "Samsung");
    m.insert("00:23:99", "Samsung");
    m.insert("00:23:D6", "Samsung");
    m.insert("00:23:D7", "Samsung");
    m.insert("00:24:54", "Samsung");
    m.insert("00:24:90", "Samsung");
    m.insert("00:24:91", "Samsung");
    m.insert("00:25:66", "Samsung");
    m.insert("00:25:67", "Samsung");
    m.insert("00:26:37", "Samsung");
    m.insert("00:26:5D", "Samsung");
    m.insert("00:26:5F", "Samsung");

    // Google
    m.insert("00:1A:11", "Google");
    m.insert("3C:5A:B4", "Google");
    m.insert("54:60:09", "Google");
    m.insert("94:EB:2C", "Google");
    m.insert("A4:77:33", "Google");
    m.insert("F4:F5:D8", "Google");
    m.insert("F4:F5:E8", "Google");

    // Intel
    m.insert("00:02:B3", "Intel");
    m.insert("00:03:47", "Intel");
    m.insert("00:04:23", "Intel");
    m.insert("00:07:E9", "Intel");
    m.insert("00:0C:F1", "Intel");
    m.insert("00:0E:0C", "Intel");
    m.insert("00:0E:35", "Intel");
    m.insert("00:11:11", "Intel");
    m.insert("00:12:F0", "Intel");
    m.insert("00:13:02", "Intel");
    m.insert("00:13:20", "Intel");
    m.insert("00:13:CE", "Intel");
    m.insert("00:13:E8", "Intel");
    m.insert("00:15:00", "Intel");
    m.insert("00:15:17", "Intel");
    m.insert("00:16:6F", "Intel");
    m.insert("00:16:76", "Intel");
    m.insert("00:16:EA", "Intel");
    m.insert("00:16:EB", "Intel");
    m.insert("00:18:DE", "Intel");
    m.insert("00:19:D1", "Intel");
    m.insert("00:19:D2", "Intel");
    m.insert("00:1B:21", "Intel");
    m.insert("00:1B:77", "Intel");
    m.insert("00:1C:BF", "Intel");
    m.insert("00:1C:C0", "Intel");
    m.insert("00:1D:E0", "Intel");
    m.insert("00:1D:E1", "Intel");
    m.insert("00:1E:64", "Intel");
    m.insert("00:1E:65", "Intel");
    m.insert("00:1E:67", "Intel");
    m.insert("00:1F:3B", "Intel");
    m.insert("00:1F:3C", "Intel");
    m.insert("00:20:E0", "Intel");
    m.insert("00:21:5C", "Intel");
    m.insert("00:21:5D", "Intel");
    m.insert("00:21:6A", "Intel");
    m.insert("00:21:6B", "Intel");
    m.insert("00:22:FA", "Intel");
    m.insert("00:22:FB", "Intel");
    m.insert("00:23:14", "Intel");
    m.insert("00:23:15", "Intel");
    m.insert("00:24:D6", "Intel");
    m.insert("00:24:D7", "Intel");
    m.insert("00:26:C6", "Intel");
    m.insert("00:26:C7", "Intel");
    m.insert("00:27:10", "Intel");

    // Microsoft
    m.insert("00:0D:3A", "Microsoft");
    m.insert("00:12:5A", "Microsoft");
    m.insert("00:15:5D", "Microsoft");
    m.insert("00:17:FA", "Microsoft");
    m.insert("00:1D:D8", "Microsoft");
    m.insert("00:22:48", "Microsoft");
    m.insert("00:25:AE", "Microsoft");
    m.insert("00:50:F2", "Microsoft");
    m.insert("28:18:78", "Microsoft");
    m.insert("30:59:B7", "Microsoft");
    m.insert("50:1A:C5", "Microsoft");
    m.insert("58:82:A8", "Microsoft");
    m.insert("60:45:BD", "Microsoft");
    m.insert("7C:1E:52", "Microsoft");
    m.insert("7C:ED:8D", "Microsoft");

    // Huawei
    m.insert("00:18:82", "Huawei");
    m.insert("00:1E:10", "Huawei");
    m.insert("00:22:A1", "Huawei");
    m.insert("00:25:68", "Huawei");
    m.insert("00:25:9E", "Huawei");
    m.insert("00:34:FE", "Huawei");
    m.insert("00:46:4B", "Huawei");
    m.insert("00:5A:13", "Huawei");
    m.insert("00:66:4B", "Huawei");
    m.insert("00:9A:CD", "Huawei");
    m.insert("00:E0:FC", "Huawei");
    m.insert("00:F8:1C", "Huawei");

    // Xiaomi
    m.insert("00:9E:C8", "Xiaomi");
    m.insert("04:CF:8C", "Xiaomi");
    m.insert("0C:1D:AF", "Xiaomi");
    m.insert("10:2A:B3", "Xiaomi");
    m.insert("14:F6:5A", "Xiaomi");
    m.insert("18:59:36", "Xiaomi");
    m.insert("20:34:FB", "Xiaomi");
    m.insert("28:6C:07", "Xiaomi");
    m.insert("34:80:B3", "Xiaomi");
    m.insert("38:A4:ED", "Xiaomi");
    m.insert("3C:BD:3E", "Xiaomi");
    m.insert("44:23:7C", "Xiaomi");
    m.insert("50:8F:4C", "Xiaomi");
    m.insert("58:44:98", "Xiaomi");
    m.insert("64:09:80", "Xiaomi");
    m.insert("64:B4:73", "Xiaomi");
    m.insert("68:DF:DD", "Xiaomi");
    m.insert("74:23:44", "Xiaomi");
    m.insert("78:02:F8", "Xiaomi");
    m.insert("78:11:DC", "Xiaomi");

    // OnePlus
    m.insert("00:1B:52", "OnePlus");
    m.insert("64:A2:F9", "OnePlus");
    m.insert("94:65:2D", "OnePlus");
    m.insert("C0:EE:FB", "OnePlus");

    // Amazon
    m.insert("00:FC:8B", "Amazon");
    m.insert("0C:47:C9", "Amazon");
    m.insert("10:AE:60", "Amazon");
    m.insert("18:74:2E", "Amazon");
    m.insert("34:D2:70", "Amazon");
    m.insert("40:B4:CD", "Amazon");
    m.insert("44:65:0D", "Amazon");
    m.insert("50:DC:E7", "Amazon");
    m.insert("68:37:E9", "Amazon");
    m.insert("68:54:FD", "Amazon");
    m.insert("74:C2:46", "Amazon");
    m.insert("84:D6:D0", "Amazon");
    m.insert("A0:02:DC", "Amazon");
    m.insert("AC:63:BE", "Amazon");
    m.insert("B4:7C:9C", "Amazon");
    m.insert("F0:27:2D", "Amazon");
    m.insert("FC:A1:83", "Amazon");

    // Espressif (ESP8266/ESP32 IoT devices)
    m.insert("18:FE:34", "Espressif");
    m.insert("24:0A:C4", "Espressif");
    m.insert("24:62:AB", "Espressif");
    m.insert("24:6F:28", "Espressif");
    m.insert("24:B2:DE", "Espressif");
    m.insert("2C:3A:E8", "Espressif");
    m.insert("30:AE:A4", "Espressif");
    m.insert("3C:61:05", "Espressif");
    m.insert("3C:71:BF", "Espressif");
    m.insert("48:3F:DA", "Espressif");
    m.insert("4C:11:AE", "Espressif");
    m.insert("5C:CF:7F", "Espressif");
    m.insert("60:01:94", "Espressif");
    m.insert("68:C6:3A", "Espressif");
    m.insert("84:0D:8E", "Espressif");
    m.insert("84:CC:A8", "Espressif");
    m.insert("84:F3:EB", "Espressif");
    m.insert("A0:20:A6", "Espressif");
    m.insert("A4:7B:9D", "Espressif");
    m.insert("A4:CF:12", "Espressif");
    m.insert("AC:D0:74", "Espressif");
    m.insert("B4:E6:2D", "Espressif");
    m.insert("BC:DD:C2", "Espressif");
    m.insert("C4:4F:33", "Espressif");
    m.insert("C8:2B:96", "Espressif");
    m.insert("CC:50:E3", "Espressif");
    m.insert("D8:A0:1D", "Espressif");
    m.insert("DC:4F:22", "Espressif");
    m.insert("EC:FA:BC", "Espressif");

    // Raspberry Pi
    m.insert("B8:27:EB", "Raspberry Pi");
    m.insert("DC:A6:32", "Raspberry Pi");
    m.insert("E4:5F:01", "Raspberry Pi");

    // TP-Link
    m.insert("00:1D:0F", "TP-Link");
    m.insert("00:27:19", "TP-Link");
    m.insert("14:CC:20", "TP-Link");
    m.insert("14:CF:92", "TP-Link");
    m.insert("18:A6:F7", "TP-Link");
    m.insert("1C:3B:F3", "TP-Link");
    m.insert("30:B5:C2", "TP-Link");
    m.insert("50:C7:BF", "TP-Link");
    m.insert("54:C8:0F", "TP-Link");
    m.insert("5C:89:9A", "TP-Link");
    m.insert("60:E3:27", "TP-Link");
    m.insert("64:56:01", "TP-Link");
    m.insert("64:70:02", "TP-Link");
    m.insert("6C:B0:CE", "TP-Link");
    m.insert("78:44:76", "TP-Link");
    m.insert("90:F6:52", "TP-Link");
    m.insert("94:D9:B3", "TP-Link");
    m.insert("98:DA:C4", "TP-Link");
    m.insert("A0:F3:C1", "TP-Link");
    m.insert("AC:84:C6", "TP-Link");
    m.insert("B0:4E:26", "TP-Link");
    m.insert("B0:95:75", "TP-Link");
    m.insert("C0:25:E9", "TP-Link");
    m.insert("C4:6E:1F", "TP-Link");
    m.insert("C8:3A:35", "TP-Link");
    m.insert("D4:6E:0E", "TP-Link");
    m.insert("D8:07:B6", "TP-Link");
    m.insert("E8:94:F6", "TP-Link");
    m.insert("EC:08:6B", "TP-Link");
    m.insert("EC:17:2F", "TP-Link");
    m.insert("F4:F2:6D", "TP-Link");

    // Sony
    m.insert("00:01:4A", "Sony");
    m.insert("00:04:1F", "Sony");
    m.insert("00:13:A9", "Sony");
    m.insert("00:15:C1", "Sony");
    m.insert("00:19:63", "Sony");
    m.insert("00:1A:80", "Sony");
    m.insert("00:1D:BA", "Sony");
    m.insert("00:1E:A4", "Sony");
    m.insert("00:21:9E", "Sony");
    m.insert("00:23:45", "Sony");
    m.insert("00:24:BE", "Sony");
    m.insert("00:EB:2D", "Sony");
    m.insert("04:5D:4B", "Sony");
    m.insert("04:76:6E", "Sony");
    m.insert("10:4F:A8", "Sony");
    m.insert("24:21:AB", "Sony");
    m.insert("28:0D:FC", "Sony");
    m.insert("30:17:C8", "Sony");
    m.insert("30:A9:DE", "Sony");

    // LG
    m.insert("00:1C:62", "LG");
    m.insert("00:1E:75", "LG");
    m.insert("00:1F:6B", "LG");
    m.insert("00:1F:E3", "LG");
    m.insert("00:22:A9", "LG");
    m.insert("00:25:E5", "LG");
    m.insert("00:26:E2", "LG");
    m.insert("00:34:DA", "LG");
    m.insert("00:AA:70", "LG");
    m.insert("00:E0:91", "LG");
    m.insert("04:33:89", "LG");
    m.insert("08:D4:2B", "LG");
    m.insert("10:68:3F", "LG");
    m.insert("10:F9:6F", "LG");
    m.insert("14:C9:13", "LG");
    m.insert("1C:BC:D0", "LG");
    m.insert("20:21:A5", "LG");
    m.insert("28:3F:69", "LG");

    m
});

/// Check if a MAC address is locally administered (randomized)
/// Bit 1 of the first octet indicates local administration
pub fn is_randomized_mac(mac: &str) -> bool {
    let mac_clean = mac.replace([':', '-', '.'], "");
    if mac_clean.len() < 2 {
        return false;
    }

    if let Ok(first_byte) = u8::from_str_radix(&mac_clean[0..2], 16) {
        // Bit 1 (second least significant bit) = locally administered
        (first_byte & 0x02) != 0
    } else {
        false
    }
}

/// Look up the vendor/manufacturer for a MAC address
pub fn lookup_vendor(mac: &str) -> Option<&'static str> {
    // Normalize MAC format to XX:XX:XX
    let mac_upper = mac.to_uppercase();
    let parts: Vec<&str> = mac_upper.split(['.', '-', '.']).collect();
    // let parts: Vec<&str> = mac_upper.split(|c| c == ':' || c == '-' || c == '.').collect();

    if parts.len() < 3 {
        return None;
    }

    let oui = format!("{}:{}:{}", parts[0], parts[1], parts[2]);
    OUI_DATABASE.get(oui.as_str()).copied()
}

/// Get a short vendor code (3-4 chars) for display
pub fn vendor_short(mac: &str) -> String {
    if is_randomized_mac(mac) {
        return "RND".to_string();
    }

    match lookup_vendor(mac) {
        Some("Apple") => "AAPL".to_string(),
        Some("Samsung") => "SMSN".to_string(),
        Some("Google") => "GOOG".to_string(),
        Some("Intel") => "INTL".to_string(),
        Some("Microsoft") => "MSFT".to_string(),
        Some("Huawei") => "HWAI".to_string(),
        Some("Xiaomi") => "XIAO".to_string(),
        Some("OnePlus") => "1+".to_string(),
        Some("Amazon") => "AMZN".to_string(),
        Some("Espressif") => "ESP".to_string(),
        Some("Raspberry Pi") => "RPI".to_string(),
        Some("TP-Link") => "TPLK".to_string(),
        Some("Sony") => "SONY".to_string(),
        Some("LG") => "LG".to_string(),
        Some(v) => v.chars().take(4).collect::<String>().to_uppercase(),
        None => "UNK".to_string(),
    }
}

/// Get device type hint based on vendor and other characteristics
pub fn infer_device_type(mac: &str, vendor: Option<&str>) -> &'static str {
    if is_randomized_mac(mac) {
        return "Phone/Tablet"; // Most randomized MACs are mobile devices
    }

    match vendor {
        Some("Apple") | Some("Samsung") | Some("Huawei") | Some("Xiaomi") | Some("OnePlus")
        | Some("Google") => "Phone/Tablet",
        Some("Intel") | Some("Microsoft") => "Laptop/PC",
        Some("Espressif") | Some("Raspberry Pi") => "IoT",
        Some("Amazon") => "Echo/Fire",
        Some("Sony") => "PlayStation/TV",
        Some("LG") => "TV/Phone",
        Some("TP-Link") => "Router/IoT",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_randomized_mac() {
        // Locally administered (randomized) - second char is 2, 6, A, or E
        assert!(is_randomized_mac("02:00:00:00:00:00"));
        assert!(is_randomized_mac("06:00:00:00:00:00"));
        assert!(is_randomized_mac("0A:00:00:00:00:00"));
        assert!(is_randomized_mac("0E:00:00:00:00:00"));
        assert!(is_randomized_mac("42:00:00:00:00:00"));

        // Universally administered (real)
        assert!(!is_randomized_mac("00:00:00:00:00:00"));
        assert!(!is_randomized_mac("04:00:00:00:00:00"));
        assert!(!is_randomized_mac("08:00:00:00:00:00"));
    }

    #[test]
    fn test_vendor_lookup() {
        assert_eq!(lookup_vendor("00:03:93:00:00:00"), Some("Apple"));
        assert_eq!(lookup_vendor("B8:27:EB:00:00:00"), Some("Raspberry Pi"));
        assert_eq!(lookup_vendor("5C:CF:7F:00:00:00"), Some("Espressif"));
        assert_eq!(lookup_vendor("FF:FF:FF:00:00:00"), None);
    }
}
