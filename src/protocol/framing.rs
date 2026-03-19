//! MTGO protocol message framing.
//!
//! Every message uses an 8-byte header:
//! - Offset 0: i32 total length (including header), little-endian
//! - Offset 4: u16 opcode
//! - Offset 6: u16 type_check (MD5 prefix of type layout)
//! - Offset 8+: payload bytes

use std::io::Read;

use super::DecodeError;

/// Maximum allowed message size (16 MB) to prevent OOM on malformed input.
const MAX_MESSAGE_LEN: i32 = 16 * 1024 * 1024;

/// A parsed protocol message with opcode, type check value, and raw payload.
#[derive(Debug, Clone, PartialEq)]
pub struct RawMessage {
    pub opcode: u16,
    pub type_check: u16,
    pub payload: Vec<u8>,
}

/// Read a single framed message from a byte stream.
pub fn read_message(r: &mut impl Read) -> Result<RawMessage, DecodeError> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let total_len = i32::from_le_bytes(len_buf);

    if total_len < 8 || total_len > MAX_MESSAGE_LEN {
        return Err(DecodeError::UnexpectedEof {
            context: "frame length out of range",
        });
    }

    let remaining = (total_len as usize) - 4;
    let mut buf = vec![0u8; remaining];
    r.read_exact(&mut buf)?;

    let opcode = u16::from_le_bytes([buf[0], buf[1]]);
    let type_check = u16::from_le_bytes([buf[2], buf[3]]);
    let payload = buf[4..].to_vec();

    Ok(RawMessage {
        opcode,
        type_check,
        payload,
    })
}

/// Parse all complete messages from a byte slice.
///
/// Reads messages sequentially until end-of-data or until remaining bytes
/// cannot form a complete message. Returns all successfully parsed messages.
/// Trailing bytes that don't form a complete frame are silently ignored.
pub fn parse_messages(data: &[u8]) -> Result<Vec<RawMessage>, DecodeError> {
    let mut cursor = std::io::Cursor::new(data);
    let mut messages = Vec::new();

    while (cursor.position() as usize) < data.len() {
        match read_message(&mut cursor) {
            Ok(msg) => messages.push(msg),
            Err(e) if e.is_eof() => break,
            Err(e) => return Err(e),
        }
    }

    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Build a valid framed message from parts.
    fn make_frame(opcode: u16, type_check: u16, payload: &[u8]) -> Vec<u8> {
        let total_len = (8 + payload.len()) as i32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&total_len.to_le_bytes());
        buf.extend_from_slice(&opcode.to_le_bytes());
        buf.extend_from_slice(&type_check.to_le_bytes());
        buf.extend_from_slice(payload);
        buf
    }

    #[test]
    fn test_read_single_message() {
        let frame = make_frame(1153, 0xABCD, &[0x01, 0x02, 0x03]);
        let mut cursor = Cursor::new(&frame);
        let msg = read_message(&mut cursor).unwrap();
        assert_eq!(msg.opcode, 1153);
        assert_eq!(msg.type_check, 0xABCD);
        assert_eq!(msg.payload, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_read_empty_payload() {
        let frame = make_frame(42, 0, &[]);
        let mut cursor = Cursor::new(&frame);
        let msg = read_message(&mut cursor).unwrap();
        assert_eq!(msg.opcode, 42);
        assert!(msg.payload.is_empty());
    }

    #[test]
    fn test_frame_too_short() {
        let data = 4i32.to_le_bytes();
        let mut cursor = Cursor::new(&data[..]);
        let err = read_message(&mut cursor).unwrap_err();
        assert!(err.is_eof());
    }

    #[test]
    fn test_negative_length() {
        let data = (-1i32).to_le_bytes();
        let mut cursor = Cursor::new(&data[..]);
        let err = read_message(&mut cursor).unwrap_err();
        assert!(err.is_eof());
    }

    #[test]
    fn test_truncated_payload() {
        let mut data = 100i32.to_le_bytes().to_vec();
        data.extend_from_slice(&[0u8; 4]);
        let mut cursor = Cursor::new(&data);
        let err = read_message(&mut cursor).unwrap_err();
        assert!(err.is_eof());
    }

    #[test]
    fn test_parse_multiple_messages() {
        let mut data = make_frame(1153, 0, &[0x01]);
        data.extend(make_frame(4652, 0, &[0x02, 0x03]));

        let messages = parse_messages(&data).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].opcode, 1153);
        assert_eq!(messages[0].payload, vec![0x01]);
        assert_eq!(messages[1].opcode, 4652);
        assert_eq!(messages[1].payload, vec![0x02, 0x03]);
    }

    #[test]
    fn test_parse_empty_input() {
        let messages = parse_messages(&[]).unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_parse_trailing_bytes() {
        let mut data = make_frame(1153, 0, &[0x01]);
        data.extend_from_slice(&[0xFF, 0xFF]);

        let messages = parse_messages(&data).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].opcode, 1153);
    }
}
