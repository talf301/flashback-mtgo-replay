// Raw packet analysis tools for protocol research

#[derive(Debug, Clone, PartialEq)]
pub struct PacketAnalysis {
    pub len: usize,
    pub header: Vec<u8>,
    pub body: Vec<u8>,
    pub patterns: Vec<Pattern>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    pub name: String,
    pub location: usize,
    pub value: Vec<u8>,
}

pub fn analyze_packet(data: &[u8]) -> PacketAnalysis {
    let len = data.len();

    // Split into header (first 2 bytes) and body (rest)
    let header_len = std::cmp::min(2, len);
    let header = data[..header_len].to_vec();
    let body = data[header_len..].to_vec();

    PacketAnalysis {
        len,
        header,
        body,
        patterns: Vec::new(),
    }
}

pub fn analyze_dump_file(path: &std::path::Path) -> Result<Vec<PacketAnalysis>, std::io::Error> {
    // Read the entire file as raw bytes
    let data = std::fs::read(path)?;

    // For now, treat the entire file as a single packet
    // In a real implementation, you'd parse actual packet boundaries
    Ok(vec![analyze_packet(&data)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_raw_packet() {
        // Test basic packet analysis
        let data = [0x01, 0x02, 0x03, 0x04, 0x05];
        let analysis = analyze_packet(&data);

        assert_eq!(analysis.len, 5);
        assert_eq!(analysis.header, vec![0x01, 0x02]);
        assert_eq!(analysis.body, vec![0x03, 0x04, 0x05]);
        assert!(analysis.patterns.is_empty());
    }
}
