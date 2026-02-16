/**
 * End-to-End Integration Tests
 *
 * Tests the full pipeline from replay creation through loading and verification.
 */

use flashback::replay::{
    ReplayFile, ReplayAction, ReplayHeader, PlayerInfo, GameResult, ActionType,
    create_test_replay, write_replay_file, load_replay_file,
};
use flashback::protocol::GameEvent;
use flashback::replay::boundary::{is_game_start_event, is_game_end_event, find_game_boundaries};
use chrono::{Utc, Duration};
use std::collections::HashMap;
use tempfile::TempDir;

#[test]
fn test_full_replay_pipeline() {
    // Step 1: Create a replay file
    let temp_dir = TempDir::new().expect("Should create temp directory");
    let replay_path = temp_dir.path().join("test_replay.flashback");

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
        game_id: "integration-test-game-001".to_string(),
        players: players.clone(),
        format: "Standard".to_string(),
        start_time,
        end_time: Some(start_time + Duration::minutes(15)),
        result: GameResult::Win {
            winner_id: "player1".to_string(),
        },
    };

    let actions = vec![
        // Turn 1 - Player 1
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
            timestamp: start_time + Duration::seconds(5),
            turn: 1,
            phase: "main".to_string(),
            active_player: "player1".to_string(),
            action_type: ActionType::PlayLand {
                player_id: "player1".to_string(),
                card_id: "forest-001".to_string(),
            },
        },
        ReplayAction {
            timestamp: start_time + Duration::seconds(6),
            turn: 1,
            phase: "main".to_string(),
            active_player: "player1".to_string(),
            action_type: ActionType::ZoneTransition {
                card_id: "forest-001".to_string(),
                from_zone: "hand".to_string(),
                to_zone: "battlefield".to_string(),
                player_id: Some("player1".to_string()),
            },
        },
        ReplayAction {
            timestamp: start_time + Duration::seconds(10),
            turn: 1,
            phase: "combat".to_string(),
            active_player: "player1".to_string(),
            action_type: ActionType::PassPriority {
                player_id: "player1".to_string(),
            },
        },
        // Turn 1 - Player 2
        ReplayAction {
            timestamp: start_time + Duration::seconds(30),
            turn: 1,
            phase: "upkeep".to_string(),
            active_player: "player2".to_string(),
            action_type: ActionType::DrawCard {
                player_id: "player2".to_string(),
                card_id: "mountain-001".to_string(),
            },
        },
        ReplayAction {
            timestamp: start_time + Duration::seconds(35),
            turn: 1,
            phase: "main".to_string(),
            active_player: "player2".to_string(),
            action_type: ActionType::PlayLand {
                player_id: "player2".to_string(),
                card_id: "mountain-001".to_string(),
            },
        },
        ReplayAction {
            timestamp: start_time + Duration::seconds(36),
            turn: 1,
            phase: "main".to_string(),
            active_player: "player2".to_string(),
            action_type: ActionType::ZoneTransition {
                card_id: "mountain-001".to_string(),
                from_zone: "hand".to_string(),
                to_zone: "battlefield".to_string(),
                player_id: Some("player2".to_string()),
            },
        },
        ReplayAction {
            timestamp: start_time + Duration::seconds(40),
            turn: 1,
            phase: "main".to_string(),
            active_player: "player2".to_string(),
            action_type: ActionType::CastSpell {
                player_id: "player2".to_string(),
                card_id: "lightning-bolt-001".to_string(),
            },
        },
        // Lightning Bolt resolves
        ReplayAction {
            timestamp: start_time + Duration::seconds(45),
            turn: 1,
            phase: "main".to_string(),
            active_player: "player2".to_string(),
            action_type: ActionType::Resolve {
                card_id: "lightning-bolt-001".to_string(),
            },
        },
        // Life change from Lightning Bolt
        ReplayAction {
            timestamp: start_time + Duration::seconds(45),
            turn: 1,
            phase: "main".to_string(),
            active_player: "player2".to_string(),
            action_type: ActionType::LifeChange {
                player_id: "player1".to_string(),
                old_life: 20,
                new_life: 17,
            },
        },
        // Lightning Bolt goes to graveyard
        ReplayAction {
            timestamp: start_time + Duration::seconds(46),
            turn: 1,
            phase: "main".to_string(),
            active_player: "player2".to_string(),
            action_type: ActionType::ZoneTransition {
                card_id: "lightning-bolt-001".to_string(),
                from_zone: "stack".to_string(),
                to_zone: "graveyard".to_string(),
                player_id: Some("player2".to_string()),
            },
        },
        // Turn 2 - Player 1
        ReplayAction {
            timestamp: start_time + Duration::seconds(60),
            turn: 2,
            phase: "upkeep".to_string(),
            active_player: "player1".to_string(),
            action_type: ActionType::DrawCard {
                player_id: "player1".to_string(),
                card_id: "llanowar-elves-001".to_string(),
            },
        },
        ReplayAction {
            timestamp: start_time + Duration::seconds(65),
            turn: 2,
            phase: "main".to_string(),
            active_player: "player1".to_string(),
            action_type: ActionType::PlayLand {
                player_id: "player1".to_string(),
                card_id: "forest-002".to_string(),
            },
        },
        ReplayAction {
            timestamp: start_time + Duration::seconds(66),
            turn: 2,
            phase: "main".to_string(),
            active_player: "player1".to_string(),
            action_type: ActionType::ZoneTransition {
                card_id: "forest-002".to_string(),
                from_zone: "hand".to_string(),
                to_zone: "battlefield".to_string(),
                player_id: Some("player1".to_string()),
            },
        },
        ReplayAction {
            timestamp: start_time + Duration::seconds(70),
            turn: 2,
            phase: "main".to_string(),
            active_player: "player1".to_string(),
            action_type: ActionType::CastSpell {
                player_id: "player1".to_string(),
                card_id: "llanowar-elves-001".to_string(),
            },
        },
        ReplayAction {
            timestamp: start_time + Duration::seconds(75),
            turn: 2,
            phase: "main".to_string(),
            active_player: "player1".to_string(),
            action_type: ActionType::Resolve {
                card_id: "llanowar-elves-001".to_string(),
            },
        },
        // Creature enters battlefield
        ReplayAction {
            timestamp: start_time + Duration::seconds(76),
            turn: 2,
            phase: "main".to_string(),
            active_player: "player1".to_string(),
            action_type: ActionType::ZoneTransition {
                card_id: "llanowar-elves-001".to_string(),
                from_zone: "stack".to_string(),
                to_zone: "battlefield".to_string(),
                player_id: Some("player1".to_string()),
            },
        },
    ];

    let mut metadata = HashMap::new();
    metadata.insert("version".to_string(), "1.0".to_string());
    metadata.insert("recorder".to_string(), "flashback".to_string());
    metadata.insert("mtgo_version".to_string(), "4.0.0".to_string());

    let replay = ReplayFile {
        header: header.clone(),
        actions: actions.clone(),
        metadata: metadata.clone(),
    };

    // Step 2: Write replay file
    write_replay_file(&replay_path, &replay).expect("Should write replay file");
    assert!(replay_path.exists(), "Replay file should exist");

    // Step 3: Load replay file
    let loaded = load_replay_file(&replay_path).expect("Should load replay file");

    // Step 4: Verify integrity
    assert_eq!(loaded.header.game_id, header.game_id);
    assert_eq!(loaded.header.players.len(), header.players.len());
    assert_eq!(loaded.header.format, header.format);
    assert_eq!(loaded.actions.len(), actions.len());
    assert_eq!(loaded.metadata.get("version"), Some(&"1.0".to_string()));

    // Step 5: Verify action sequence
    assert_eq!(loaded.actions[0].turn, 1);
    assert_eq!(loaded.actions[0].phase, "upkeep");
    assert_eq!(loaded.actions[0].active_player, "player1");

    // Verify all actions are in order
    for i in 1..loaded.actions.len() {
        assert!(
            loaded.actions[i].timestamp >= loaded.actions[i - 1].timestamp,
            "Actions should be in chronological order"
        );
    }

    // Step 6: Verify game state reconstruction
    let mut player1_life = 20;
    let mut player2_life = 20;
    let mut battlefield_cards: Vec<String> = Vec::new();

    for action in &loaded.actions {
        match &action.action_type {
            ActionType::LifeChange { player_id, new_life, .. } => {
                if player_id == "player1" {
                    player1_life = *new_life;
                } else if player_id == "player2" {
                    player2_life = *new_life;
                }
            }
            ActionType::ZoneTransition { card_id, to_zone, .. } => {
                if to_zone == "battlefield" {
                    battlefield_cards.push(card_id.clone());
                }
            }
            _ => {}
        }
    }

    // Verify final state
    assert_eq!(player1_life, 17, "Player 1 should have 17 life after Lightning Bolt");
    assert_eq!(player2_life, 20, "Player 2 should have 20 life");
    assert_eq!(battlefield_cards.len(), 4, "Should have 4 cards on battlefield");
    assert!(battlefield_cards.contains(&"forest-001".to_string()));
    assert!(battlefield_cards.contains(&"llanowar-elves-001".to_string()));
    assert!(battlefield_cards.contains(&"forest-002".to_string()));
    assert!(battlefield_cards.contains(&"mountain-001".to_string()));
}

#[test]
fn test_game_boundary_detection() {
    // Test game start event detection
    let start_event = GameEvent::GameStart {
        game_id: "test-game".to_string(),
    };
    assert!(is_game_start_event(&start_event));

    // Test game end event detection
    let end_event = GameEvent::GameEnd {
        winner: "player1".to_string(),
    };
    assert!(is_game_end_event(&end_event));

    // Test non-boundary event
    let action_event = GameEvent::CastSpell {
        player_id: "player1".to_string(),
        card_id: "spell-001".to_string(),
    };
    assert!(!is_game_start_event(&action_event));
    assert!(!is_game_end_event(&action_event));
}

#[test]
fn test_find_game_boundaries() {
    let events = vec![
        GameEvent::GameStart {
            game_id: "game1".to_string(),
        },
        GameEvent::DrawCard {
            player_id: "player1".to_string(),
            card_id: "card1".to_string(),
        },
        GameEvent::CastSpell {
            player_id: "player1".to_string(),
            card_id: "spell1".to_string(),
        },
        GameEvent::GameEnd {
            winner: "player1".to_string(),
        },
    ];

    let boundaries = find_game_boundaries(&events);

    assert_eq!(boundaries.len(), 1);
    assert_eq!(boundaries[0].start, 0);
    assert_eq!(boundaries[0].end, 4);
}

#[test]
fn test_multiple_replays_in_sequence() {
    let temp_dir = TempDir::new().expect("Should create temp directory");

    // Create and save first replay
    let replay1_path = temp_dir.path().join("game1.flashback");
    let replay1 = create_test_replay();
    write_replay_file(&replay1_path, &replay1).expect("Should write first replay");

    // Create and save second replay
    let replay2_path = temp_dir.path().join("game2.flashback");
    let mut replay2 = create_test_replay();
    replay2.header.game_id = "game-002".to_string();
    write_replay_file(&replay2_path, &replay2).expect("Should write second replay");

    // Load both replays
    let loaded1 = load_replay_file(&replay1_path).expect("Should load first replay");
    let loaded2 = load_replay_file(&replay2_path).expect("Should load second replay");

    // Verify they are different games
    assert_ne!(loaded1.header.game_id, loaded2.header.game_id);
    assert_eq!(loaded1.header.game_id, "test-game-001");
    assert_eq!(loaded2.header.game_id, "game-002");
}

#[test]
fn test_replay_roundtrip_with_all_action_types() {
    let temp_dir = TempDir::new().expect("Should create temp directory");
    let replay_path = temp_dir.path().join("full_types.flashback");

    let start_time = Utc::now();

    let replay = ReplayFile {
        header: ReplayHeader {
            game_id: "full-types-test".to_string(),
            players: vec![
                PlayerInfo {
                    player_id: "player1".to_string(),
                    name: "Alice".to_string(),
                    life_total: 20,
                },
            ],
            format: "Standard".to_string(),
            start_time,
            end_time: None,
            result: GameResult::Incomplete,
        },
        actions: vec![
            ReplayAction {
                timestamp: start_time,
                turn: 1,
                phase: "upkeep".to_string(),
                active_player: "player1".to_string(),
                action_type: ActionType::DrawCard {
                    player_id: "player1".to_string(),
                    card_id: "card1".to_string(),
                },
            },
            ReplayAction {
                timestamp: start_time + Duration::seconds(1),
                turn: 1,
                phase: "main".to_string(),
                active_player: "player1".to_string(),
                action_type: ActionType::PlayLand {
                    player_id: "player1".to_string(),
                    card_id: "land1".to_string(),
                },
            },
            ReplayAction {
                timestamp: start_time + Duration::seconds(2),
                turn: 1,
                phase: "main".to_string(),
                active_player: "player1".to_string(),
                action_type: ActionType::CastSpell {
                    player_id: "player1".to_string(),
                    card_id: "spell1".to_string(),
                },
            },
            ReplayAction {
                timestamp: start_time + Duration::seconds(3),
                turn: 1,
                phase: "main".to_string(),
                active_player: "player1".to_string(),
                action_type: ActionType::ActivateAbility {
                    player_id: "player1".to_string(),
                    card_id: "card2".to_string(),
                    ability_id: "tap".to_string(),
                },
            },
            ReplayAction {
                timestamp: start_time + Duration::seconds(4),
                turn: 1,
                phase: "combat".to_string(),
                active_player: "player1".to_string(),
                action_type: ActionType::Attack {
                    attacker_id: "creature1".to_string(),
                    defender_id: "player2".to_string(),
                },
            },
            ReplayAction {
                timestamp: start_time + Duration::seconds(5),
                turn: 1,
                phase: "combat".to_string(),
                active_player: "player2".to_string(),
                action_type: ActionType::Block {
                    attacker_id: "creature1".to_string(),
                    blocker_id: "creature2".to_string(),
                },
            },
            ReplayAction {
                timestamp: start_time + Duration::seconds(6),
                turn: 1,
                phase: "combat".to_string(),
                active_player: "player1".to_string(),
                action_type: ActionType::Resolve {
                    card_id: "spell1".to_string(),
                },
            },
            ReplayAction {
                timestamp: start_time + Duration::seconds(7),
                turn: 1,
                phase: "combat".to_string(),
                active_player: "player2".to_string(),
                action_type: ActionType::LifeChange {
                    player_id: "player2".to_string(),
                    old_life: 20,
                    new_life: 18,
                },
            },
            ReplayAction {
                timestamp: start_time + Duration::seconds(8),
                turn: 1,
                phase: "end".to_string(),
                active_player: "player1".to_string(),
                action_type: ActionType::ZoneTransition {
                    card_id: "spell1".to_string(),
                    from_zone: "stack".to_string(),
                    to_zone: "graveyard".to_string(),
                    player_id: Some("player1".to_string()),
                },
            },
            ReplayAction {
                timestamp: start_time + Duration::seconds(9),
                turn: 1,
                phase: "end".to_string(),
                active_player: "player1".to_string(),
                action_type: ActionType::PassPriority {
                    player_id: "player1".to_string(),
                },
            },
            ReplayAction {
                timestamp: start_time + Duration::seconds(10),
                turn: 2,
                phase: "upkeep".to_string(),
                active_player: "player1".to_string(),
                action_type: ActionType::PhaseChange {
                    phase: "upkeep".to_string(),
                },
            },
            ReplayAction {
                timestamp: start_time + Duration::seconds(11),
                turn: 2,
                phase: "beginning".to_string(),
                active_player: "player1".to_string(),
                action_type: ActionType::TurnChange {
                    turn: 2,
                    player_id: "player1".to_string(),
                },
            },
            ReplayAction {
                timestamp: start_time + Duration::seconds(12),
                turn: 2,
                phase: "beginning".to_string(),
                active_player: "player1".to_string(),
                action_type: ActionType::Unknown {
                    description: "Some unknown event".to_string(),
                },
            },
        ],
        metadata: HashMap::new(),
    };

    // Write and load
    write_replay_file(&replay_path, &replay).expect("Should write replay");
    let loaded = load_replay_file(&replay_path).expect("Should load replay");

    // Verify all actions round-trip correctly
    assert_eq!(loaded.actions.len(), replay.actions.len());
    for (original, loaded) in replay.actions.iter().zip(loaded.actions.iter()) {
        assert_eq!(original.action_type, loaded.action_type);
    }
}
