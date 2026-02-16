use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GameEvent {
    Unknown { raw: Vec<u8> },
    GameStart { game_id: String },
    DrawCard { player_id: String, card_id: String },
    PlayLand { player_id: String, card_id: String },
    CastSpell { player_id: String, card_id: String },
    Attack { attacker_id: String, defender_id: String },
    LifeChange { player_id: String, old_life: i32, new_life: i32 },
    GameEnd { winner: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedEvent {
    pub timestamp: DateTime<Utc>,
    pub event: GameEvent,
}

/// Decode a single packet into game events
/// This is a placeholder that will be updated based on protocol research
pub fn decode_packet(packet: &[u8]) -> Vec<GameEvent> {
    // Placeholder: Return unknown event
    // Real implementation will parse MTGO protocol structure
    vec![GameEvent::Unknown {
        raw: packet.to_vec(),
    }]
}

/// Decode a stream of packets into events
pub fn decode_stream(packets: &[Vec<u8>]) -> Vec<DecodedEvent> {
    let mut events = Vec::new();
    let timestamp = Utc::now();

    for packet in packets {
        let game_events = decode_packet(packet);
        for event in game_events {
            events.push(DecodedEvent {
                timestamp: timestamp.clone(),
                event,
            });
        }
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_unknown_packet() {
        let packet = vec![0x00, 0x01, 0x02, 0x03];
        let events = decode_packet(&packet);
        // Should return Unknown event
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], GameEvent::Unknown { .. }));
    }

    #[test]
    fn test_event_serialization() {
        let event = GameEvent::DrawCard {
            player_id: "player1".to_string(),
            card_id: "12345".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: GameEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event, decoded);
    }
}
