use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerInfo {
    pub player_id: String,
    pub name: String,
    pub life_total: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GameResult {
    Win { winner_id: String },
    Draw,
    Incomplete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionType {
    DrawCard { player_id: String, card_id: String },
    PlayLand { player_id: String, card_id: String },
    CastSpell { player_id: String, card_id: String },
    ActivateAbility { player_id: String, card_id: String, ability_id: String },
    Attack { attacker_id: String, defender_id: String },
    Block { attacker_id: String, blocker_id: String },
    Resolve { card_id: String },
    LifeChange { player_id: String, old_life: i32, new_life: i32 },
    ZoneTransition {
        card_id: String,
        from_zone: String,
        to_zone: String,
        player_id: Option<String>,
    },
    TapPermanent { card_id: String },
    UntapPermanent { card_id: String },
    DamageMarked { card_id: String, damage: i32 },
    SummoningSickness { card_id: String, has_sickness: bool },
    FaceDown { card_id: String },
    FaceUp { card_id: String },
    Attach { card_id: String, attached_to_id: String },
    Detach { card_id: String },
    CounterUpdate { card_id: String, counter_type: String, count: i32 },
    PowerToughnessUpdate { card_id: String, power: i32, toughness: i32 },
    PassPriority { player_id: String },
    PhaseChange { phase: String },
    TurnChange { turn: i32, player_id: String },
    Unknown { description: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplayAction {
    pub timestamp: DateTime<Utc>,
    pub turn: i32,
    pub phase: String,
    pub active_player: String,
    pub action_type: ActionType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplayHeader {
    pub game_id: String,
    pub players: Vec<PlayerInfo>,
    pub format: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub result: GameResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReplayFile {
    pub header: ReplayHeader,
    pub actions: Vec<ReplayAction>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Replay file not found: {0}")]
    NotFound(String),
    #[error("Invalid replay format: {0}")]
    InvalidFormat(String),
}

pub type Result<T> = std::result::Result<T, ReplayError>;

pub fn create_test_replay() -> ReplayFile {
    let start_time = Utc::now();
    
    let players = vec![
        PlayerInfo {
            player_id: "player1".to_string(),
            name: "Alice".to_string(),
            life_total: 20,
        },
        PlayerInfo {
            player_id: "player2".to_string(),
            name: "Bob".to_string(),
            life_total: 20,
        },
    ];
    
    let header = ReplayHeader {
        game_id: "test-game-001".to_string(),
        players,
        format: "Standard".to_string(),
        start_time,
        end_time: None,
        result: GameResult::Incomplete,
    };
    
    let actions = vec![
        ReplayAction {
            timestamp: start_time,
            turn: 1,
            phase: "upkeep".to_string(),
            active_player: "player1".to_string(),
            action_type: ActionType::DrawCard {
                player_id: "player1".to_string(),
                card_id: "forest-001".to_string(),
            },
        },
        ReplayAction {
            timestamp: start_time + chrono::Duration::seconds(5),
            turn: 1,
            phase: "main".to_string(),
            active_player: "player1".to_string(),
            action_type: ActionType::PlayLand {
                player_id: "player1".to_string(),
                card_id: "forest-001".to_string(),
            },
        },
        ReplayAction {
            timestamp: start_time + chrono::Duration::seconds(10),
            turn: 1,
            phase: "combat".to_string(),
            active_player: "player1".to_string(),
            action_type: ActionType::Attack {
                attacker_id: "player1".to_string(),
                defender_id: "player2".to_string(),
            },
        },
    ];
    
    let mut metadata = HashMap::new();
    metadata.insert("version".to_string(), "1.0".to_string());
    metadata.insert("recorder".to_string(), "flashback".to_string());
    
    ReplayFile {
        header,
        actions,
        metadata,
    }
}

pub fn write_replay_file<P: AsRef<Path>>(path: P, replay: &ReplayFile) -> Result<()> {
    let json = serde_json::to_string_pretty(replay)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn load_replay_file<P: AsRef<Path>>(path: P) -> Result<ReplayFile> {
    let content = fs::read_to_string(path)?;
    let replay: ReplayFile = serde_json::from_str(&content)?;
    Ok(replay)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_replay_file_serialization() {
        let replay = create_test_replay();
        
        // Serialize to JSON
        let json = serde_json::to_string(&replay).expect("Should serialize");
        assert!(!json.is_empty());
        
        // Deserialize back
        let deserialized: ReplayFile = serde_json::from_str(&json).expect("Should deserialize");
        
        assert_eq!(replay, deserialized);
    }
    
    #[test]
    fn test_write_and_load_replay() {
        let replay = create_test_replay();
        let temp_file = NamedTempFile::new().expect("Should create temp file");
        
        // Write
        write_replay_file(temp_file.path(), &replay).expect("Should write");
        
        // Load
        let loaded = load_replay_file(temp_file.path()).expect("Should load");
        
        assert_eq!(replay, loaded);
    }
    
    #[test]
    fn test_action_types() {
        let actions = vec![
            ActionType::DrawCard {
                player_id: "p1".to_string(),
                card_id: "c1".to_string(),
            },
            ActionType::PlayLand {
                player_id: "p1".to_string(),
                card_id: "c2".to_string(),
            },
            ActionType::CastSpell {
                player_id: "p1".to_string(),
                card_id: "c3".to_string(),
            },
            ActionType::ActivateAbility {
                player_id: "p1".to_string(),
                card_id: "c4".to_string(),
                ability_id: "tap".to_string(),
            },
            ActionType::Attack {
                attacker_id: "p1".to_string(),
                defender_id: "p2".to_string(),
            },
            ActionType::Block {
                attacker_id: "p1".to_string(),
                blocker_id: "p2".to_string(),
            },
            ActionType::Resolve {
                card_id: "c3".to_string(),
            },
            ActionType::LifeChange {
                player_id: "p2".to_string(),
                old_life: 20,
                new_life: 18,
            },
            ActionType::ZoneTransition {
                card_id: "c3".to_string(),
                from_zone: "stack".to_string(),
                to_zone: "graveyard".to_string(),
                player_id: Some("p1".to_string()),
            },
            ActionType::TapPermanent {
                card_id: "c5".to_string(),
            },
            ActionType::UntapPermanent {
                card_id: "c5".to_string(),
            },
            ActionType::DamageMarked {
                card_id: "c5".to_string(),
                damage: 3,
            },
            ActionType::SummoningSickness {
                card_id: "c5".to_string(),
                has_sickness: true,
            },
            ActionType::FaceDown {
                card_id: "c5".to_string(),
            },
            ActionType::FaceUp {
                card_id: "c5".to_string(),
            },
            ActionType::Attach {
                card_id: "c6".to_string(),
                attached_to_id: "c5".to_string(),
            },
            ActionType::Detach {
                card_id: "c6".to_string(),
            },
            ActionType::CounterUpdate {
                card_id: "c5".to_string(),
                counter_type: "+1/+1".to_string(),
                count: 2,
            },
            ActionType::PowerToughnessUpdate {
                card_id: "c5".to_string(),
                power: 4,
                toughness: 5,
            },
            ActionType::PassPriority {
                player_id: "p1".to_string(),
            },
            ActionType::PhaseChange {
                phase: "combat".to_string(),
            },
            ActionType::TurnChange {
                turn: 2,
                player_id: "p2".to_string(),
            },
            ActionType::Unknown {
                description: "test".to_string(),
            },
        ];
        
        // Verify all action types serialize and deserialize correctly
        for action in actions {
            let json = serde_json::to_string(&action).expect("Should serialize");
            let deserialized: ActionType = serde_json::from_str(&json).expect("Should deserialize");
            assert_eq!(action, deserialized);
        }
    }
}
