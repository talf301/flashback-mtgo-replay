use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug)]
pub enum DumperError {
    Io(io::Error),
}

impl From<io::Error> for DumperError {
    fn from(err: io::Error) -> Self {
        DumperError::Io(err)
    }
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

    pub fn write_packet(&mut self, timestamp_us: u64, data: &[u8]) -> Result<(), DumperError> {
        // Format: [timestamp_us:8][data_len:4][data...]
        self.file.write_all(&timestamp_us.to_le_bytes())?;
        self.file.write_all(&(data.len() as u32).to_le_bytes())?;
        self.file.write_all(data)?;
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
        dumper.write_packet(12345678, packet_data).unwrap();

        assert_eq!(dumper.packet_count(), 1);

        let file_content = std::fs::read(test_file.path()).unwrap();

        // Verify timestamp (8 bytes, little-endian)
        let timestamp = u64::from_le_bytes([
            file_content[0], file_content[1], file_content[2], file_content[3],
            file_content[4], file_content[5], file_content[6], file_content[7],
        ]);
        assert_eq!(timestamp, 12345678);

        // Verify data length (4 bytes, little-endian)
        let data_len = u32::from_le_bytes([
            file_content[8], file_content[9], file_content[10], file_content[11],
        ]);
        assert_eq!(data_len, packet_data.len() as u32);

        // Verify data
        assert_eq!(&file_content[12..], packet_data);
    }
}
