use crate::protocol::{DecodedEvent, GameEvent};
use crate::replay::schema::{ReplayFile, ReplayHeader, ReplayAction, PlayerInfo, GameResult, ActionType};
use crate::replay::schema::Result as ReplayResult;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::Path;
use crate::replay::schema::{write_replay_file, ReplayError};

pub struct ReplayWriter {
    game_id: Option<String>,
    players: HashMap<String, PlayerInfo>,
    format: String,
    start_time: DateTime<Utc>,
    end_time: Option<DateTime<Utc>>,
    result: GameResult,
    actions: Vec<ReplayAction>,
    current_turn: i32,
    current_phase: String,
    current_player: Option<String>,
    metadata: HashMap<String, String>,
}

impl ReplayWriter {
    /// Create a new replay writer
    pub fn new(format: String) -> Self {
        let mut metadata = HashMap::new();
        metadata.insert("version".to_string(), "1.0".to_string());
        metadata.insert("recorder".to_string(), "flashback".to_string());
        
        ReplayWriter {
            game_id: None,
            players: HashMap::new(),
            format,
            start_time: Utc::now(),
            end_time: None,
            result: GameResult::Incomplete,
            actions: Vec::new(),
            current_turn: 1,
            current_phase: "upkeep".to_string(),
            current_player: None,
            metadata,
        }
    }
    
    /// Set the game ID
    pub fn set_game_id(&mut self, game_id: String) -> &mut Self {
        self.game_id = Some(game_id);
        self
    }
    
    /// Set or update a player's information
    pub fn set_player(&mut self, player_id: String, name: String, life_total: i32) -> &mut Self {
        let player_info = PlayerInfo {
            player_id: player_id.clone(),
            name,
            life_total,
        };
        self.players.insert(player_id, player_info);
        self
    }
    
    /// Add multiple decoded events to the replay
    pub fn add_events(&mut self, events: &[DecodedEvent]) -> ReplayResult<()> {
        for event in events {
            self.add_event(event)?;
        }
        Ok(())
    }
    
    /// Add a single decoded event to the replay
    pub fn add_event(&mut self, event: &DecodedEvent) -> ReplayResult<()> {
        match &event.event {
            GameEvent::GameStart { game_id } => {
                if self.game_id.is_none() {
                    self.game_id = Some(game_id.clone());
                    self.start_time = event.timestamp;
                }
            }
            GameEvent::GameEnd { winner } => {
                self.end_time = Some(event.timestamp);
                self.result = GameResult::Win {
                    winner_id: winner.clone(),
                };
            }
            GameEvent::LifeChange { player_id, new_life, .. } => {
                if let Some(player) = self.players.get_mut(player_id) {
                    player.life_total = *new_life;
                }
            }
            _ => {}
        }
        
        // Convert event to action and add to list
        if let Some(action) = self.event_to_action(event) {
            self.actions.push(action);
        }
        
        Ok(())
    }
    
    /// Convert a decoded event to a replay action
    fn event_to_action(&self, event: &DecodedEvent) -> Option<ReplayAction> {
        let action_type = match &event.event {
            GameEvent::DrawCard { player_id, card_id } => Some(ActionType::DrawCard {
                player_id: player_id.clone(),
                card_id: card_id.clone(),
            }),
            GameEvent::PlayLand { player_id, card_id } => Some(ActionType::PlayLand {
                player_id: player_id.clone(),
                card_id: card_id.clone(),
            }),
            GameEvent::CastSpell { player_id, card_id } => Some(ActionType::CastSpell {
                player_id: player_id.clone(),
                card_id: card_id.clone(),
            }),
            GameEvent::Attack { attacker_id, defender_id } => Some(ActionType::Attack {
                attacker_id: attacker_id.clone(),
                defender_id: defender_id.clone(),
            }),
            GameEvent::LifeChange { player_id, old_life, new_life } => Some(ActionType::LifeChange {
                player_id: player_id.clone(),
                old_life: *old_life,
                new_life: *new_life,
            }),
            GameEvent::GameStart { .. } => None, // Header info, not an action
            GameEvent::GameEnd { .. } => None,   // Header info, not an action
            GameEvent::Unknown { raw } => Some(ActionType::Unknown {
                description: format!("Unknown event: {} bytes", raw.len()),
            }),
        };
        
        action_type.map(|action_type| ReplayAction {
            timestamp: event.timestamp,
            turn: self.current_turn,
            phase: self.current_phase.clone(),
            active_player: self.current_player.clone().unwrap_or_default(),
            action_type,
        })
    }
    
    /// Write the replay to a file
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> ReplayResult<()> {
        let game_id = self.game_id.as_ref().ok_or_else(|| {
            ReplayError::InvalidFormat("No game ID set".to_string())
        })?;
        
        let players_vec: Vec<PlayerInfo> = self.players.values().cloned().collect();
        
        let header = ReplayHeader {
            game_id: game_id.clone(),
            players: players_vec,
            format: self.format.clone(),
            start_time: self.start_time,
            end_time: self.end_time,
            result: self.result.clone(),
        };
        
        let replay = ReplayFile {
            header,
            actions: self.actions.clone(),
            metadata: self.metadata.clone(),
        };
        
        write_replay_file(path, &replay)
    }
    
    /// Get the current number of actions
    pub fn action_count(&self) -> usize {
        self.actions.len()
    }
    
    /// Get the current turn number
    pub fn current_turn(&self) -> i32 {
        self.current_turn
    }
    
    /// Get the current phase
    pub fn current_phase(&self) -> &str {
        &self.current_phase
    }
    
    /// Get a mutable reference to the replay file (for advanced use cases)
    pub fn as_replay_file(&self) -> ReplayResult<ReplayFile> {
        let game_id = self.game_id.as_ref().ok_or_else(|| {
            ReplayError::InvalidFormat("No game ID set".to_string())
        })?;
        
        let players_vec: Vec<PlayerInfo> = self.players.values().cloned().collect();
        
        let header = ReplayHeader {
            game_id: game_id.clone(),
            players: players_vec,
            format: self.format.clone(),
            start_time: self.start_time,
            end_time: self.end_time,
            result: self.result.clone(),
        };
        
        Ok(ReplayFile {
            header,
            actions: self.actions.clone(),
            metadata: self.metadata.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    fn create_test_events() -> Vec<DecodedEvent> {
        let base_time = Utc::now();
        
        vec![
            DecodedEvent {
                timestamp: base_time,
                event: GameEvent::GameStart {
                    game_id: "test-game-001".to_string(),
                },
            },
            DecodedEvent {
                timestamp: base_time + chrono::Duration::seconds(1),
                event: GameEvent::DrawCard {
                    player_id: "player1".to_string(),
                    card_id: "forest-001".to_string(),
                },
            },
            DecodedEvent {
                timestamp: base_time + chrono::Duration::seconds(2),
                event: GameEvent::DrawCard {
                    player_id: "player2".to_string(),
                    card_id: "island-001".to_string(),
                },
            },
            DecodedEvent {
                timestamp: base_time + chrono::Duration::seconds(3),
                event: GameEvent::PlayLand {
                    player_id: "player1".to_string(),
                    card_id: "forest-001".to_string(),
                },
            },
            DecodedEvent {
                timestamp: base_time + chrono::Duration::seconds(4),
                event: GameEvent::CastSpell {
                    player_id: "player1".to_string(),
                    card_id: "lightning-bolt-001".to_string(),
                },
            },
            DecodedEvent {
                timestamp: base_time + chrono::Duration::seconds(5),
                event: GameEvent::Attack {
                    attacker_id: "player1".to_string(),
                    defender_id: "player2".to_string(),
                },
            },
            DecodedEvent {
                timestamp: base_time + chrono::Duration::seconds(6),
                event: GameEvent::LifeChange {
                    player_id: "player2".to_string(),
                    old_life: 20,
                    new_life: 17,
                },
            },
            DecodedEvent {
                timestamp: base_time + chrono::Duration::seconds(10),
                event: GameEvent::GameEnd {
                    winner: "player1".to_string(),
                },
            },
        ]
    }
    
    #[test]
    fn test_replay_writer_new() {
        let writer = ReplayWriter::new("Standard".to_string());
        assert_eq!(writer.action_count(), 0);
        assert_eq!(writer.current_turn(), 1);
        assert_eq!(writer.current_phase(), "upkeep");
    }
    
    #[test]
    fn test_set_player() {
        let mut writer = ReplayWriter::new("Standard".to_string());
        writer.set_game_id("test-game".to_string());
        writer.set_player("p1".to_string(), "Alice".to_string(), 20);
        writer.set_player("p2".to_string(), "Bob".to_string(), 20);
        let replay = writer.as_replay_file().unwrap();
        assert_eq!(replay.header.players.len(), 2);
        // HashMap doesn't preserve order, so find by player_id
        let alice = replay.header.players.iter().find(|p| p.player_id == "p1").unwrap();
        let bob = replay.header.players.iter().find(|p| p.player_id == "p2").unwrap();
        assert_eq!(alice.name, "Alice");
        assert_eq!(bob.name, "Bob");
    }
    
    #[test]
    fn test_add_events() {
        let mut writer = ReplayWriter::new("Standard".to_string());
        writer.set_player("player1".to_string(), "Alice".to_string(), 20);
        writer.set_player("player2".to_string(), "Bob".to_string(), 20);
        
        let events = create_test_events();
        writer.add_events(&events).unwrap();
        
        assert_eq!(writer.action_count(), 6); // Excluding GameStart and GameEnd
    }
    
    #[test]
    fn test_add_event_updates_life() {
        let mut writer = ReplayWriter::new("Standard".to_string());
        writer.set_player("player2".to_string(), "Bob".to_string(), 20);
        
        let events = create_test_events();
        writer.add_events(&events).unwrap();
        
        let replay = writer.as_replay_file().unwrap();
        let bob = replay.header.players.iter().find(|p| p.player_id == "player2").unwrap();
        assert_eq!(bob.life_total, 17);
    }
    
    #[test]
    fn test_write_replay_from_events() {
        let mut writer = ReplayWriter::new("Standard".to_string());
        writer.set_player("player1".to_string(), "Alice".to_string(), 20);
        writer.set_player("player2".to_string(), "Bob".to_string(), 20);
        
        let events = create_test_events();
        writer.add_events(&events).unwrap();
        
        let temp_file = NamedTempFile::new().expect("Should create temp file");
        writer.write_to_file(temp_file.path()).expect("Should write replay");
        
        // Verify file was written and can be read
        let loaded = crate::replay::schema::load_replay_file(temp_file.path())
            .expect("Should load replay");
        
        assert_eq!(loaded.header.game_id, "test-game-001");
        assert_eq!(loaded.header.format, "Standard");
        assert_eq!(loaded.actions.len(), 6);
        assert_eq!(loaded.header.players.len(), 2);
    }
    
    #[test]
    fn test_write_without_game_id() {
        let mut writer = ReplayWriter::new("Standard".to_string());
        writer.set_player("player1".to_string(), "Alice".to_string(), 20);
        
        // Add events without GameStart
        let events = vec![
            DecodedEvent {
                timestamp: Utc::now(),
                event: GameEvent::DrawCard {
                    player_id: "player1".to_string(),
                    card_id: "c1".to_string(),
                },
            },
        ];
        writer.add_events(&events).unwrap();
        
        let temp_file = NamedTempFile::new().expect("Should create temp file");
        let result = writer.write_to_file(temp_file.path());
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ReplayError::InvalidFormat(_)));
    }
    
    #[test]
    fn test_event_to_action_unknown() {
        let writer = ReplayWriter::new("Standard".to_string());
        let event = DecodedEvent {
            timestamp: Utc::now(),
            event: GameEvent::Unknown {
                raw: vec![0x01, 0x02, 0x03],
            },
        };
        
        let action = writer.event_to_action(&event);
        assert!(action.is_some());
        
        if let Some(action) = action {
            assert!(matches!(action.action_type, ActionType::Unknown { .. }));
        }
    }
    
    #[test]
    fn test_event_to_action_game_boundaries() {
        let writer = ReplayWriter::new("Standard".to_string());
        
        // GameStart should not produce an action
        let start = DecodedEvent {
            timestamp: Utc::now(),
            event: GameEvent::GameStart {
                game_id: "test".to_string(),
            },
        };
        assert!(writer.event_to_action(&start).is_none());
        
        // GameEnd should not produce an action
        let end = DecodedEvent {
            timestamp: Utc::now(),
            event: GameEvent::GameEnd {
                winner: "player1".to_string(),
            },
        };
        assert!(writer.event_to_action(&end).is_none());
    }
}
