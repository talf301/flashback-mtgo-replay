use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use chrono::Utc;
use thiserror::Error;
use tracing::debug;

#[derive(Error, Debug)]
pub enum DumperError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

pub struct PacketDumper {
    file: File,
    packet_count: u64,
}

impl PacketDumper {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, DumperError> {
        let file = File::create(path)?;
        Ok(Self {
            file,
            packet_count: 0,
        })
    }

    pub fn write_packet(&mut self, data: &[u8]) -> Result<(), DumperError> {
        let timestamp_us = Utc::now().timestamp_micros() as u64;
        debug!(
            "Writing packet: {} bytes, timestamp: {}",
            data.len(),
            timestamp_us
        );

        // Format: [timestamp_us:8][data_len:4][data...]
        self.file.write_all(&timestamp_us.to_be_bytes())?;
        self.file.write_all(&(data.len() as u32).to_be_bytes())?;
        self.file.write_all(data)?;
        self.file.flush()?;
        self.packet_count += 1;
        Ok(())
    }

    pub fn packet_count(&self) -> u64 {
        self.packet_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_raw_packet() {
        let test_file = tempfile::NamedTempFile::new().unwrap();
        let mut dumper = PacketDumper::new(test_file.path()).unwrap();

        let packet_data = b"test packet data";
        dumper.write_packet(packet_data).unwrap();

        assert_eq!(dumper.packet_count(), 1);

        let file_content = std::fs::read(test_file.path()).unwrap();

        // Verify timestamp (8 bytes, big-endian)
        let timestamp = u64::from_be_bytes([
            file_content[0], file_content[1], file_content[2], file_content[3],
            file_content[4], file_content[5], file_content[6], file_content[7],
        ]);
        // Timestamp is dynamically generated, just verify it's non-zero
        assert!(timestamp > 0);

        // Verify data length (4 bytes, big-endian)
        let data_len = u32::from_be_bytes([
            file_content[8], file_content[9], file_content[10], file_content[11],
        ]);
        assert_eq!(data_len, packet_data.len() as u32);

        // Verify data
        assert_eq!(&file_content[12..], packet_data);
    }

    #[test]
    fn test_multiple_packets() {
        let test_file = tempfile::NamedTempFile::new().unwrap();
        let mut dumper = PacketDumper::new(test_file.path()).unwrap();

        let packet1 = b"first packet";
        let packet2 = b"second packet data";
        let packet3 = b"third";

        dumper.write_packet(packet1).unwrap();
        dumper.write_packet(packet2).unwrap();
        dumper.write_packet(packet3).unwrap();

        assert_eq!(dumper.packet_count(), 3);

        let file_content = std::fs::read(test_file.path()).unwrap();

        // Packet 1: header at 0-11, data at 12-23 ("first packet" = 12 bytes)
        assert_eq!(&file_content[12..24], packet1);
        // Packet 2: starts at offset 24 (header1+data1), header at 24-35, data at 36-53
        let offset2 = 24 + 12;
        assert_eq!(&file_content[offset2..offset2 + 18], packet2); // "second packet data" = 18 bytes
        // Packet 3: starts after packet2 header+data at 54, header at 54-65, data at 66-70
        let offset3 = 36 + 18 + 12;
        assert_eq!(&file_content[offset3..offset3 + 5], packet3); // "third" = 5 bytes
    }

    #[test]
    fn test_empty_packet() {
        let test_file = tempfile::NamedTempFile::new().unwrap();
        let mut dumper = PacketDumper::new(test_file.path()).unwrap();

        let empty_packet = b"";
        dumper.write_packet(empty_packet).unwrap();

        assert_eq!(dumper.packet_count(), 1);

        let file_content = std::fs::read(test_file.path()).unwrap();

        // Verify data length is 0
        let data_len = u32::from_be_bytes([
            file_content[8], file_content[9], file_content[10], file_content[11],
        ]);
        assert_eq!(data_len, 0);

        // File should contain only headers (12 bytes)
        assert_eq!(file_content.len(), 12);
    }

    #[test]
    fn test_large_packet() {
        let test_file = tempfile::NamedTempFile::new().unwrap();
        let mut dumper = PacketDumper::new(test_file.path()).unwrap();

        // Create a packet >4KB
        let large_packet: Vec<u8> = (0..5000).map(|i| (i % 256) as u8).collect();

        dumper.write_packet(&large_packet).unwrap();

        assert_eq!(dumper.packet_count(), 1);

        let file_content = std::fs::read(test_file.path()).unwrap();

        // Verify data length
        let data_len = u32::from_be_bytes([
            file_content[8], file_content[9], file_content[10], file_content[11],
        ]);
        assert_eq!(data_len, 5000);

        // Verify file size (12 bytes header + 5000 bytes data)
        assert_eq!(file_content.len(), 5012);

        // Verify data content
        assert_eq!(&file_content[12..], &large_packet[..]);
    }

    #[test]
    fn test_write_error() {
        // Try to create a dumper at a path with a non-existent parent directory
        let temp_dir = tempfile::TempDir::new().unwrap();
        let nonexistent_path = temp_dir.path().join("nonexistent/subdir/file.pcap");

        // Should fail because parent directory doesn't exist
        let result = PacketDumper::new(&nonexistent_path);
        assert!(result.is_err());
    }
}
