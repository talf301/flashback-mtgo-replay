//! GAME_RESULTS message decoder (opcode 4485).

use std::io::{Cursor, Read};
use super::DecodeError;

/// Decoded GAME_RESULTS payload.
#[derive(Debug, Clone)]
pub struct GameResultsMessage {
    pub game_id: u32,
    pub winner_seat: Option<u8>, // None = draw or unparseable
    pub players: Vec<GameResultPlayer>,
}

#[derive(Debug, Clone)]
pub struct GameResultPlayer {
    pub seat_id: u8,
    pub rating: u16,
}

pub fn decode_game_results(payload: &[u8]) -> Result<GameResultsMessage, DecodeError> {
    let mut cursor = Cursor::new(payload);
    let mut buf4 = [0u8; 4];
    let mut buf2 = [0u8; 2];
    let mut buf1 = [0u8; 1];

    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GameResults: game_id",
    })?;
    let game_id = u32::from_le_bytes(buf4);

    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GameResults: winner_index",
    })?;
    let winner_index = u32::from_le_bytes(buf4);

    cursor.read_exact(&mut buf1).map_err(|_| DecodeError::UnexpectedEof {
        context: "GameResults: unknown byte",
    })?;

    cursor.read_exact(&mut buf2).map_err(|_| DecodeError::UnexpectedEof {
        context: "GameResults: player_count",
    })?;
    let player_count = u16::from_le_bytes(buf2) as usize;

    cursor.read_exact(&mut buf2).map_err(|_| DecodeError::UnexpectedEof {
        context: "GameResults: unknown u16",
    })?;

    let mut players = Vec::with_capacity(player_count);
    for _ in 0..player_count {
        cursor.read_exact(&mut buf1).map_err(|_| DecodeError::UnexpectedEof {
            context: "GameResults: player seat_id",
        })?;
        let seat_id = buf1[0];

        cursor.read_exact(&mut buf2).map_err(|_| DecodeError::UnexpectedEof {
            context: "GameResults: player rating",
        })?;
        let rating = u16::from_le_bytes(buf2);

        cursor.read_exact(&mut buf2).map_err(|_| DecodeError::UnexpectedEof {
            context: "GameResults: player padding",
        })?;

        players.push(GameResultPlayer { seat_id, rating });
    }

    let winner_seat = if (winner_index as usize) < player_count {
        Some(players[winner_index as usize].seat_id)
    } else {
        None
    };

    Ok(GameResultsMessage {
        game_id,
        winner_seat,
        players,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn golden_game1_payload() -> Vec<u8> {
        vec![
            0x6e, 0x00, 0x5d, 0x38, // game_id
            0x01, 0x00, 0x00, 0x00, // winner_index = 1
            0x00,                   // unknown
            0x02, 0x00,             // player_count = 2
            0x00, 0x00,             // unknown
            0x02, 0x67, 0x04, 0x00, 0x00, // seat 2, rating 1127
            0x03, 0x96, 0x04, 0x00, 0x00, // seat 3, rating 1174
        ]
    }

    #[test]
    fn test_decode_game_results_winner() {
        let msg = decode_game_results(&golden_game1_payload()).unwrap();
        assert_eq!(msg.game_id, 945619054);
        assert_eq!(msg.winner_seat, Some(3)); // seat 3 = entries[1]
        assert_eq!(msg.players.len(), 2);
        assert_eq!(msg.players[0].seat_id, 2);
        assert_eq!(msg.players[0].rating, 1127);
        assert_eq!(msg.players[1].seat_id, 3);
        assert_eq!(msg.players[1].rating, 1174);
    }

    #[test]
    fn test_decode_game_results_draw_sentinel() {
        let mut payload = golden_game1_payload();
        payload[4] = 0x02; // winner_index = 2, but only 2 players
        let msg = decode_game_results(&payload).unwrap();
        assert_eq!(msg.winner_seat, None);
    }

    #[test]
    fn test_decode_game_results_empty_payload() {
        let result = decode_game_results(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_golden_file_has_3_game_results() {
        use crate::protocol::{fls, framing, opcodes};
        let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
        let messages = framing::parse_messages(&data).unwrap();

        let mut results = Vec::new();
        for msg in &messages {
            if msg.opcode != 1153 && msg.opcode != 1156 { continue; }
            let fls_msg = fls::decode_fls(msg.clone()).unwrap();
            let meta = match fls_msg {
                fls::FlsMessage::GsMessage { meta, .. } => meta,
                _ => continue,
            };
            let mut cursor = std::io::Cursor::new(&meta);
            let inner = framing::read_message(&mut cursor).unwrap();
            if inner.opcode == opcodes::GAME_RESULTS {
                results.push(decode_game_results(&inner.payload).unwrap());
            }
        }

        assert_eq!(results.len(), 3, "Bo3 should have 3 GAME_RESULTS");
        assert_eq!(results[0].winner_seat, Some(3));
        assert_eq!(results[1].winner_seat, Some(2));
        assert_eq!(results[2].winner_seat, Some(3));
    }
}
