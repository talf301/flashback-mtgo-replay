// MTGO server detection and BPF filter construction

// Known MTGO server IPs (confirmed via Wireshark capture, operated by Daybreak Games)
const MTGO_SERVER_IPS: &[&str] = &[
    "69.174.204.165",
];

// MTGO server port (confirmed via Wireshark capture)
const MTGO_GAME_PORT: u16 = 7770;

/// Check if an IP address is a known MTGO server
#[allow(dead_code)]
pub fn is_mtgo_server(addr: &str) -> bool {
    MTGO_SERVER_IPS.contains(&addr)
}

/// Build a BPF filter string to capture MTGO traffic
pub fn build_bpf_filter() -> String {
    if MTGO_SERVER_IPS.is_empty() {
        return format!("tcp port {}", MTGO_GAME_PORT);
    }

    let filters: Vec<String> = MTGO_SERVER_IPS
        .iter()
        .map(|ip| format!("(host {} and tcp port {})", ip, MTGO_GAME_PORT))
        .collect();

    format!("({})", filters.join(" or "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mtgo_server() {
        assert!(is_mtgo_server("69.174.204.165"));
        assert!(!is_mtgo_server("8.8.8.8"));
        assert!(!is_mtgo_server("not-an-ip"));
    }
}
