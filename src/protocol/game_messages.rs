//! Inner game message decoding (Level 2, inside FLS MetaMessage).

use std::io::{Cursor, Read};

use super::DecodeError;
use super::framing;
use super::opcodes;

#[derive(Debug, Clone)]
pub struct GamePlayStatusMessage {
    pub time_left: [i32; 8],
    pub player_waiting_for: i32,
    pub game_id: u32,
    pub state_size: u32,
    pub undiffed_buffer_size: u32,
    pub n_state_elems: u32,
    pub priority_player: i32,
    pub checksum: i32,
    pub last_state_checksum: i32,
    pub flags: u8,
    pub game_state_timestamp: u32,
    pub state_buf_raw: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum GameMessage {
    GamePlayStatus(GamePlayStatusMessage),
    GameOver,
    Other { opcode: u16 },
}

/// Decode a game-level message from the raw MetaMessage bytes.
/// The `meta` bytes include the inner 8-byte CSMessage header.
pub fn decode_game_message(meta: &[u8]) -> Result<GameMessage, DecodeError> {
    let mut cursor = Cursor::new(meta);
    let inner = framing::read_message(&mut cursor)?;

    match inner.opcode {
        opcodes::GAME_PLAY_STATUS => decode_game_play_status(&inner.payload),
        opcodes::GAME_OVER => Ok(GameMessage::GameOver),
        _ => Ok(GameMessage::Other { opcode: inner.opcode }),
    }
}

fn decode_game_play_status(payload: &[u8]) -> Result<GameMessage, DecodeError> {
    let mut cursor = Cursor::new(payload);
    let mut buf4 = [0u8; 4];

    // StateBuf: byte[] — FIRST field on the wire (int32 count + data)
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: StateBuf length",
    })?;
    let buf_len = i32::from_le_bytes(buf4);
    if buf_len < 0 {
        return Err(DecodeError::UnexpectedEof {
            context: "GamePlayStatus: negative StateBuf length",
        });
    }
    let mut state_buf_raw = vec![0u8; buf_len as usize];
    cursor.read_exact(&mut state_buf_raw).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: StateBuf data",
    })?;

    // TimeLeft[8]: 8 x int32
    let mut time_left = [0i32; 8];
    for slot in &mut time_left {
        cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
            context: "GamePlayStatus: TimeLeft",
        })?;
        *slot = i32::from_le_bytes(buf4);
    }

    // PlayerWaitingFor: int32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: PlayerWaitingFor",
    })?;
    let player_waiting_for = i32::from_le_bytes(buf4);

    // GameID: uint32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: GameID",
    })?;
    let game_id = u32::from_le_bytes(buf4);

    // StateSize: uint32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: StateSize",
    })?;
    let state_size = u32::from_le_bytes(buf4);

    // UndiffedBufferSize: uint32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: UndiffedBufferSize",
    })?;
    let undiffed_buffer_size = u32::from_le_bytes(buf4);

    // NStateElems: uint32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: NStateElems",
    })?;
    let n_state_elems = u32::from_le_bytes(buf4);

    // PriorityPlayer: int32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: PriorityPlayer",
    })?;
    let priority_player = i32::from_le_bytes(buf4);

    // CheckSum: int32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: CheckSum",
    })?;
    let checksum = i32::from_le_bytes(buf4);

    // LastStateChecksum: int32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: LastStateChecksum",
    })?;
    let last_state_checksum = i32::from_le_bytes(buf4);

    // Replaying: byte — consumed, not stored
    let mut buf1 = [0u8; 1];
    cursor.read_exact(&mut buf1).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: Replaying",
    })?;

    // Flags: byte
    cursor.read_exact(&mut buf1).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: Flags",
    })?;
    let flags = buf1[0];

    // GameStateTimestamp: uint32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: GameStateTimestamp",
    })?;
    let game_state_timestamp = u32::from_le_bytes(buf4);

    Ok(GameMessage::GamePlayStatus(GamePlayStatusMessage {
        time_left,
        player_waiting_for,
        game_id,
        state_size,
        undiffed_buffer_size,
        n_state_elems,
        priority_player,
        checksum,
        last_state_checksum,
        flags,
        game_state_timestamp,
        state_buf_raw,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_game_play_status_payload(
        game_id: u32,
        state_size: u32,
        n_state_elems: u32,
        checksum: i32,
        last_checksum: i32,
        flags: u8,
        state_buf: &[u8],
    ) -> Vec<u8> {
        let mut payload = Vec::new();
        // StateBuf: byte[] — FIRST field on the wire (int32 count + data)
        payload.extend_from_slice(&(state_buf.len() as i32).to_le_bytes());
        payload.extend_from_slice(state_buf);
        // TimeLeft[8]: 8 x int32 = 32 bytes
        for _ in 0..8 {
            payload.extend_from_slice(&0i32.to_le_bytes());
        }
        payload.extend_from_slice(&0i32.to_le_bytes()); // PlayerWaitingFor
        payload.extend_from_slice(&game_id.to_le_bytes()); // GameID
        payload.extend_from_slice(&state_size.to_le_bytes()); // StateSize
        payload.extend_from_slice(&0u32.to_le_bytes()); // UndiffedBufferSize
        payload.extend_from_slice(&n_state_elems.to_le_bytes()); // NStateElems
        payload.extend_from_slice(&0i32.to_le_bytes()); // PriorityPlayer
        payload.extend_from_slice(&checksum.to_le_bytes()); // CheckSum
        payload.extend_from_slice(&last_checksum.to_le_bytes()); // LastStateChecksum
        payload.push(0); // Replaying
        payload.push(flags); // Flags
        payload.extend_from_slice(&0u32.to_le_bytes()); // GameStateTimestamp
        payload
    }

    fn wrap_as_meta(opcode: u16, payload: &[u8]) -> Vec<u8> {
        // Build a complete CSMessage (8-byte header + payload)
        let total_len = (8 + payload.len()) as i32;
        let mut meta = Vec::new();
        meta.extend_from_slice(&total_len.to_le_bytes());
        meta.extend_from_slice(&opcode.to_le_bytes());
        meta.extend_from_slice(&0u16.to_le_bytes()); // type_check
        meta.extend_from_slice(payload);
        meta
    }

    #[test]
    fn test_decode_game_play_status() {
        let state_buf = vec![0xAA, 0xBB];
        let inner = make_game_play_status_payload(100, 2, 1, 0, 0, 0x06, &state_buf);
        let meta = wrap_as_meta(4652, &inner);

        let result = decode_game_message(&meta).unwrap();
        match result {
            GameMessage::GamePlayStatus(msg) => {
                assert_eq!(msg.game_id, 100);
                assert_eq!(msg.state_size, 2);
                assert_eq!(msg.n_state_elems, 1);
                assert_eq!(msg.flags, 0x06); // Head + Tail
                assert_eq!(msg.state_buf_raw, vec![0xAA, 0xBB]);
            }
            _ => panic!("expected GamePlayStatus"),
        }
    }

    #[test]
    fn test_decode_game_over() {
        let meta = wrap_as_meta(4632, &[]);
        let result = decode_game_message(&meta).unwrap();
        assert!(matches!(result, GameMessage::GameOver));
    }

    #[test]
    fn test_decode_unknown_game_opcode() {
        let meta = wrap_as_meta(9999, &[0x01]);
        let result = decode_game_message(&meta).unwrap();
        match result {
            GameMessage::Other { opcode } => assert_eq!(opcode, 9999),
            _ => panic!("expected Other"),
        }
    }

    #[test]
    fn test_golden_file_game_messages() {
        let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
        let messages = crate::protocol::framing::parse_messages(&data).unwrap();

        let mut play_status_count = 0;
        let mut game_over_count = 0;
        let mut other_game_count = 0;
        let mut decode_errors = 0;

        for msg in &messages {
            if msg.opcode != 1153 && msg.opcode != 1156 {
                continue;
            }
            let fls = crate::protocol::fls::decode_fls(msg.clone()).unwrap();
            let meta = match fls {
                crate::protocol::fls::FlsMessage::GsMessage { meta, .. } => meta,
                _ => continue,
            };
            match decode_game_message(&meta) {
                Ok(GameMessage::GamePlayStatus(_)) => play_status_count += 1,
                Ok(GameMessage::GameOver) => game_over_count += 1,
                Ok(GameMessage::Other { .. }) => other_game_count += 1,
                Err(e) => {
                    eprintln!("game decode error: {e}");
                    decode_errors += 1;
                }
            }
        }

        assert_eq!(decode_errors, 0, "all game messages should decode");
        assert!(play_status_count > 0, "expected GamePlayStatus messages");
        eprintln!(
            "Game messages: {play_status_count} PlayStatus, {game_over_count} GameOver, {other_game_count} Other"
        );
    }
}
