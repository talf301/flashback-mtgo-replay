use pcap::Capture;
use pcap::Device;
use pcap::Error as PcapError;
use std::net::IpAddr;

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("PCAP error: {0}")]
    Pcap(#[from] PcapError),
    #[error("No suitable network interface found")]
    NoInterface,
    #[error("Invalid IP address: {0}")]
    InvalidIp(String),
}

/// Lists all available network interfaces with their IP addresses.
pub fn list_interfaces() -> Result<Vec<(String, IpAddr)>, CaptureError> {
    let mut interfaces = Vec::new();

    for device in Device::list()? {
        for addr in &device.addresses {
            if matches!(addr.addr, IpAddr::V4(_)) {
                interfaces.push((device.name.clone(), addr.addr));
            }
        }
    }

    if interfaces.is_empty() {
        Err(CaptureError::NoInterface)
    } else {
        Ok(interfaces)
    }
}

/// Raw packet capture using pcap.
pub struct PacketCapture {
    capture: Capture<pcap::Active>,
}

impl PacketCapture {
    /// Creates a new packet capture on the specified interface.
    ///
    /// # Arguments
    /// * `interface` - Name of the network interface to capture from (e.g., "eth0", "wlan0")
    ///
    /// # Returns
    /// A new `PacketCapture` instance ready to capture packets.
    ///
    /// # Errors
    /// Returns `CaptureError` if the interface doesn't exist or capture setup fails.
    pub fn new(interface: &str) -> Result<Self, CaptureError> {
        let capture = Capture::from_device(Device::from(interface))
            .map_err(CaptureError::Pcap)?
            .promisc(true)
            .snaplen(65535)
            .timeout(1000) // 1 second timeout
            .open()
            .map_err(CaptureError::Pcap)?;

        Ok(PacketCapture { capture })
    }

    /// Sets a BPF (Berkeley Packet Filter) filter for the capture.
    ///
    /// # Arguments
    /// * `filter` - BPF filter expression (e.g., "tcp port 4747")
    ///
    /// # Errors
    /// Returns `CaptureError` if the filter expression is invalid.
    pub fn set_filter(&mut self, filter: &str) -> Result<(), CaptureError> {
        self.capture
            .filter(filter, true)
            .map_err(CaptureError::Pcap)?;
        Ok(())
    }

    /// Captures the next packet.
    ///
    /// # Returns
    /// The raw packet data as a byte vector.
    ///
    /// # Errors
    /// Returns `CaptureError` if no packet is available or capture fails.
    pub fn next_packet(&mut self) -> Result<Vec<u8>, CaptureError> {
        match self.capture.next_packet() {
            Ok(packet) => Ok(packet.data.to_vec()),
            Err(e) => match e {
                pcap::Error::TimeoutExpired => Ok(Vec::new()),
                e => Err(CaptureError::Pcap(e)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_network_interfaces() {
        let interfaces = list_interfaces().unwrap();
        // Should return at least one interface on any networked system
        assert!(!interfaces.is_empty(), "No network interfaces found");
        // Each entry should have a name and a valid IPv4 address
        for (name, ip) in &interfaces {
            assert!(!name.is_empty(), "Interface name is empty");
            assert!(ip.is_ipv4(), "Only IPv4 addresses expected: {}", ip);
        }
    }

    #[test]
    #[ignore]
    fn test_capture_setup() {
        // This test is ignored because it requires root privileges and
        // actual network traffic to work properly.
        let interfaces = list_interfaces().unwrap();
        if let Some((interface_name, _)) = interfaces.first() {
            let mut capture = PacketCapture::new(interface_name)
                .expect("Failed to create capture");
            
            // Set a simple filter for TCP traffic
            capture.set_filter("tcp").expect("Failed to set filter");
            
            // Note: next_packet() will likely timeout with no traffic,
            // but we're just testing the setup here
            let _ = capture.next_packet();
        }
    }
}
