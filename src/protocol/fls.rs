//! FLS envelope decoding.
//!
//! Decodes Level 1 (FLS) messages from the MTGO wire protocol.
//! Both opcode 1153 (GsMessageMessage) and 1156 (GsReplayMessageMessage)
//! map to FlsMessage::GsMessage.

use std::io::{Cursor, Read};

use super::DecodeError;
use super::framing::RawMessage;
use super::opcodes;

#[derive(Debug, Clone)]
pub enum FlsMessage {
    /// Game state message containing an embedded game-level CSMessage.
    /// `meta` is a complete embedded CSMessage with its own 8-byte header.
    GsMessage { game_id: i32, meta: Vec<u8> },
    /// Game lifecycle status change.
    GameStatusChange { new_status: u32 },
    /// Player seat order (wire layout undocumented — raw passthrough).
    PlayerOrder { raw: Vec<u8> },
    /// Game created boundary signal.
    GameCreated { game_id: i32 },
    /// Game ended boundary signal.
    GameEnded { game_id: i32 },
    /// Match started boundary signal.
    MatchStarted { game_id: i32 },
    /// Unrecognized opcode — passed through as-is.
    Other(RawMessage),
}

/// Decode a framed RawMessage into an FLS-level message.
pub fn decode_fls(msg: RawMessage) -> Result<FlsMessage, DecodeError> {
    match msg.opcode {
        opcodes::GS_MESSAGE => decode_gs_message(msg),
        opcodes::GS_REPLAY_MESSAGE => decode_gs_replay_message(msg),
        opcodes::GSH_GAME_STATUS_CHANGE => decode_game_status_change(msg),
        opcodes::GS_PLAYER_ORDER => Ok(FlsMessage::PlayerOrder { raw: msg.payload }),
        opcodes::FLS_GAME_CREATED => decode_boundary_game_id(msg, |gid| FlsMessage::GameCreated { game_id: gid }),
        opcodes::FLS_GAME_ENDED => decode_boundary_game_id(msg, |gid| FlsMessage::GameEnded { game_id: gid }),
        opcodes::FLS_MATCH_GAME_STARTED => decode_boundary_game_id(msg, |gid| FlsMessage::MatchStarted { game_id: gid }),
        _ => Ok(FlsMessage::Other(msg)),
    }
}

fn decode_gs_message(msg: RawMessage) -> Result<FlsMessage, DecodeError> {
    let mut cursor = Cursor::new(&msg.payload);

    // MatchToken: Guid (16 bytes) — consumed, not stored
    let mut guid = [0u8; 16];
    cursor.read_exact(&mut guid).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsMessage: MatchToken",
    })?;

    // MatchID: int32 — consumed, not stored
    let mut buf4 = [0u8; 4];
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsMessage: MatchID",
    })?;

    // GameID: int32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsMessage: GameID",
    })?;
    let game_id = i32::from_le_bytes(buf4);

    // MetaMessage: byte[] — int32 count + N bytes
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsMessage: MetaMessage length",
    })?;
    let meta_len = i32::from_le_bytes(buf4);
    if meta_len < 0 {
        return Err(DecodeError::UnexpectedEof {
            context: "GsMessage: negative MetaMessage length",
        });
    }
    let mut meta = vec![0u8; meta_len as usize];
    cursor.read_exact(&mut meta).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsMessage: MetaMessage bytes",
    })?;

    Ok(FlsMessage::GsMessage { game_id, meta })
}

/// Decode GsReplayMessageMessage (opcode 1156).
/// Wire layout differs from 1153: GameID(int), HostGSHServerID(int), MetaMessage(byte[]).
/// No GUID or MatchID.
fn decode_gs_replay_message(msg: RawMessage) -> Result<FlsMessage, DecodeError> {
    let mut cursor = Cursor::new(&msg.payload);
    let mut buf4 = [0u8; 4];

    // GameID: int32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsReplayMessage: GameID",
    })?;
    let game_id = i32::from_le_bytes(buf4);

    // HostGSHServerID: int32 — consumed, not stored
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsReplayMessage: HostGSHServerID",
    })?;

    // MetaMessage: byte[] — int32 count + N bytes
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsReplayMessage: MetaMessage length",
    })?;
    let meta_len = i32::from_le_bytes(buf4);
    if meta_len < 0 {
        return Err(DecodeError::UnexpectedEof {
            context: "GsReplayMessage: negative MetaMessage length",
        });
    }
    let mut meta = vec![0u8; meta_len as usize];
    cursor.read_exact(&mut meta).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsReplayMessage: MetaMessage bytes",
    })?;

    Ok(FlsMessage::GsMessage { game_id, meta })
}

fn decode_game_status_change(msg: RawMessage) -> Result<FlsMessage, DecodeError> {
    if msg.payload.len() < 4 {
        return Err(DecodeError::UnexpectedEof {
            context: "GameStatusChange: NewStatus",
        });
    }
    let new_status = u32::from_le_bytes([
        msg.payload[0], msg.payload[1], msg.payload[2], msg.payload[3],
    ]);
    Ok(FlsMessage::GameStatusChange { new_status })
}

/// Attempt to read a game_id (i32) from the start of a boundary message payload.
/// If the payload is too short, fall back to Other.
fn decode_boundary_game_id(
    msg: RawMessage,
    make: impl FnOnce(i32) -> FlsMessage,
) -> Result<FlsMessage, DecodeError> {
    if msg.payload.len() < 4 {
        // Wire layout not confirmed — fall back to Other
        eprintln!(
            "WARNING: boundary signal opcode {} payload too short ({} bytes) for game_id, falling back to Other",
            msg.opcode,
            msg.payload.len()
        );
        return Ok(FlsMessage::Other(msg));
    }
    // Try first 4 bytes as game_id — this is a best-effort guess.
    let game_id = i32::from_le_bytes([
        msg.payload[0], msg.payload[1], msg.payload[2], msg.payload[3],
    ]);
    Ok(make(game_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::framing::RawMessage;

    fn make_gs_message_payload(game_id: i32, meta: &[u8]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&[0u8; 16]); // MatchToken GUID
        payload.extend_from_slice(&0i32.to_le_bytes()); // MatchID
        payload.extend_from_slice(&game_id.to_le_bytes()); // GameID
        payload.extend_from_slice(&(meta.len() as i32).to_le_bytes()); // byte[] count
        payload.extend_from_slice(meta);
        payload
    }

    #[test]
    fn test_decode_gs_message_1153() {
        let meta = vec![0x01, 0x02, 0x03];
        let raw = RawMessage {
            opcode: 1153,
            type_check: 0,
            payload: make_gs_message_payload(42, &meta),
        };
        let result = decode_fls(raw).unwrap();
        match result {
            FlsMessage::GsMessage { game_id, meta: m } => {
                assert_eq!(game_id, 42);
                assert_eq!(m, vec![0x01, 0x02, 0x03]);
            }
            _ => panic!("expected GsMessage"),
        }
    }

    fn make_gs_replay_message_payload(game_id: i32, meta: &[u8]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&game_id.to_le_bytes()); // GameID
        payload.extend_from_slice(&0i32.to_le_bytes()); // HostGSHServerID
        payload.extend_from_slice(&(meta.len() as i32).to_le_bytes()); // byte[] count
        payload.extend_from_slice(meta);
        payload
    }

    #[test]
    fn test_decode_gs_replay_message_1156() {
        let meta = vec![0xAA];
        let raw = RawMessage {
            opcode: 1156,
            type_check: 0,
            payload: make_gs_replay_message_payload(99, &meta),
        };
        let result = decode_fls(raw).unwrap();
        match result {
            FlsMessage::GsMessage { game_id, .. } => assert_eq!(game_id, 99),
            _ => panic!("expected GsMessage for opcode 1156"),
        }
    }

    #[test]
    fn test_decode_game_status_change() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&7u32.to_le_bytes()); // GAME_COMPLETED
        let raw = RawMessage {
            opcode: 1145,
            type_check: 0,
            payload,
        };
        let result = decode_fls(raw).unwrap();
        match result {
            FlsMessage::GameStatusChange { new_status } => assert_eq!(new_status, 7),
            _ => panic!("expected GameStatusChange"),
        }
    }

    #[test]
    fn test_decode_player_order_is_passthrough() {
        let raw = RawMessage {
            opcode: 1155,
            type_check: 0,
            payload: vec![0x01, 0x02, 0x03],
        };
        let result = decode_fls(raw).unwrap();
        match result {
            FlsMessage::PlayerOrder { raw: r } => assert_eq!(r, vec![0x01, 0x02, 0x03]),
            _ => panic!("expected PlayerOrder"),
        }
    }

    #[test]
    fn test_decode_unknown_opcode() {
        let raw = RawMessage {
            opcode: 9999,
            type_check: 0,
            payload: vec![],
        };
        let result = decode_fls(raw).unwrap();
        assert!(matches!(result, FlsMessage::Other(_)));
    }

    #[test]
    fn test_golden_file_fls_decode() {
        let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
        let messages = crate::protocol::framing::parse_messages(&data).unwrap();

        let mut gs_count = 0;
        let mut status_count = 0;
        let mut other_count = 0;
        let mut errors = 0;

        for msg in messages {
            match decode_fls(msg) {
                Ok(FlsMessage::GsMessage { .. }) => gs_count += 1,
                Ok(FlsMessage::GameStatusChange { .. }) => status_count += 1,
                Ok(FlsMessage::Other(_)) => other_count += 1,
                Ok(_) => {} // other known variants
                Err(e) => {
                    eprintln!("decode error: {e}");
                    errors += 1;
                }
            }
        }

        assert_eq!(errors, 0, "all messages should decode without errors");
        assert!(gs_count > 1000, "expected ~1202 GsMessage, got {gs_count}");
        eprintln!("FLS decode: {gs_count} GsMessage, {status_count} StatusChange, {other_count} Other");
    }
}
