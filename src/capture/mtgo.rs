// MTGO server detection and BPF filter construction

use std::str::FromStr;
// Placeholder IP ranges for MTGO servers
// TODO: Update with actual MTGO server IP ranges from community research.
// Current placeholders are NOT verified and may not represent real MTGO infrastructure.
// Research sources: MTGO community forums, Wizards of the Coast support documentation,
// network analysis tools like Wireshark with MTGO traffic captures.
const MTGO_SERVER_RANGES: &[&str] = &[
    "72.5.10.0/24",    // Placeholder: MTGO server range 1
    "199.7.55.0/24",   // Placeholder: MTGO server range 2
];

// MTGO server port constants
/// Standard game traffic port for Magic: The Gathering Online
const MTGO_GAME_PORT: u16 = 4724;

/// Client update/patch port for Magic: The Gathering Online
const MTGO_UPDATE_PORT: u16 = 4765;

/// Check if an IP address belongs to a known MTGO server range
///
/// # Arguments
/// * `addr` - IP address as a string
///
/// # Returns
/// * `true` if the address is in a known MTGO server range
/// * `false` otherwise
///
/// # Limitations
/// - Currently supports IPv4 addresses only (IPv6 returns false)
/// - Server IP ranges are placeholders and require verification
#[allow(dead_code)]  // Used in tests, not currently exported
pub fn is_mtgo_server(addr: &str) -> bool {
    // IPv4-only: Parse as IPv4 address, IPv6 returns false
    let ip = match addr.parse::<std::net::Ipv4Addr>() {
        Ok(ip) => ip,
        Err(_) => return false, // Not IPv4, not a MTGO server
    };

    // Check against each server range
    for range in MTGO_SERVER_RANGES {
        if let Ok(net) = ipnet::Ipv4Net::from_str(range) {
            if net.contains(&ip) {
                return true;
            }
        }
    }

    false
}

/// Build a BPF filter string to capture MTGO traffic
///
/// Creates a filter that captures traffic to/from known MTGO server IP ranges
/// on the standard ports (TCP 4724 for game traffic, TCP 4765 for client updates).
///
/// # Returns
/// BPF filter string
pub fn build_bpf_filter() -> String {
    let mut filters = Vec::new();

    // Build host filters for each MTGO server range
    // TODO: Verify BPF filter CIDR syntax - pcap may not support CIDR notation
    // in host expressions. Consider expanding ranges to individual IPs if needed.
    for range in MTGO_SERVER_RANGES {
        filters.push(format!("(host {} and (tcp port {} or tcp port {}))", range, MTGO_GAME_PORT, MTGO_UPDATE_PORT));
    }

    // Combine all filters with OR
    if filters.is_empty() {
        // Fallback: capture all traffic on MTGO ports if no ranges defined
        format!("(tcp port {} or tcp port {})", MTGO_GAME_PORT, MTGO_UPDATE_PORT)
    } else {
        format!("({})", filters.join(" or "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mtgo_server() {
        // Test with an IP in the first range
        assert!(is_mtgo_server("72.5.10.1"));

        // Test with an IP in the second range
        assert!(is_mtgo_server("199.7.55.100"));

        // Test with an IP outside MTGO ranges
        assert!(!is_mtgo_server("8.8.8.8"));

        // Test with IPv6 (should return false for now)
        assert!(!is_mtgo_server("2001:4860:4860::8888"));

        // Test with invalid IP
        assert!(!is_mtgo_server("not-an-ip"));
    }
}
