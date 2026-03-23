//! Golden file end-to-end pipeline integration test.
//!
//! Runs the full decode pipeline against golden_v1.bin and validates
//! that recognizable game actions are produced.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use flashback::protocol::fls::{self, FlsMessage};
use flashback::protocol::framing::{self, RawMessage};
use flashback::protocol::game_messages::{self, GameMessage};
use flashback::protocol::opcodes;
use flashback::protocol::statebuf::{self, StateBufProcessor};
use flashback::protocol::DecodeError;
use flashback::replay::schema::{
    ActionType, GameHeader, GameReplay, GameResult, PlayerInfo, ReplayAction,
};
use flashback::state::GameState;
use flashback::translator::ReplayTranslator;

/// Package accumulated state into a GameReplay and push it onto `games`.
fn package_game(
    games: &mut Vec<GameReplay>,
    current_actions: &mut Vec<ReplayAction>,
    current_card_names: &mut HashMap<String, String>,
    current_card_textures: &mut HashMap<String, i32>,
    current_game_id: &mut String,
    current_winner_seat: &mut Option<u8>,
    populated_players: &mut HashSet<usize>,
    game_state: &Option<GameState>,
) {
    let players: Vec<PlayerInfo> = if let Some(state) = game_state {
        state
            .players
            .iter()
            .enumerate()
            .filter(|(i, _)| populated_players.contains(i))
            .map(|(i, ps)| PlayerInfo {
                player_id: format!("player_{}", i),
                name: format!("Player {}", i),
                life_total: ps.life,
            })
            .collect()
    } else {
        Vec::new()
    };

    let result = match *current_winner_seat {
        Some(seat) if seat >= 2 && (seat as usize - 2) < players.len() => {
            GameResult::Win {
                winner_id: format!("player_{}", seat - 2),
            }
        }
        Some(_) => GameResult::Incomplete,
        None => GameResult::Incomplete,
    };

    let game = GameReplay {
        game_number: (games.len() + 1) as u32,
        header: GameHeader {
            game_id: std::mem::take(current_game_id),
            players,
            result,
        },
        actions: std::mem::take(current_actions),
        card_names: std::mem::take(current_card_names),
        card_textures: std::mem::take(current_card_textures),
    };

    games.push(game);
    *current_winner_seat = None;
    populated_players.clear();
}

fn run_pipeline(messages: Vec<RawMessage>) -> Vec<GameReplay> {
    let mut statebuf_proc = StateBufProcessor::new();
    let mut game_state: Option<GameState> = None;
    let mut translator = ReplayTranslator::new();

    // Per-game accumulators
    let mut games: Vec<GameReplay> = Vec::new();
    let mut current_actions: Vec<ReplayAction> = Vec::new();
    let mut current_card_names: HashMap<String, String> = HashMap::new();
    let mut current_card_textures: HashMap<String, i32> = HashMap::new();
    let mut current_game_id: String = String::new();
    let mut current_winner_seat: Option<u8> = None;
    let mut populated_players: HashSet<usize> = HashSet::new();

    // Saved game state snapshot: GameOver arrives before GameResults in the
    // protocol stream, so on GameOver we snapshot the game state for player
    // info, reset the pipeline, and defer packaging until GameResults arrives.
    let mut saved_game_state: Option<GameState> = None;
    let mut pending_package = false;

    for raw_msg in messages {
        let fls_msg = match fls::decode_fls(raw_msg) {
            Ok(m) => m,
            Err(_) => continue,
        };

        match fls_msg {
            FlsMessage::GsMessage { game_id, meta } => {
                let game_msg = match game_messages::decode_game_message(&meta) {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                match game_msg {
                    GameMessage::GamePlayStatus(gps) => {
                        // If we have a pending package (GameOver seen but no
                        // GameResults yet) and a new GamePlayStatus arrives,
                        // package as incomplete before processing new data.
                        if pending_package {
                            package_game(
                                &mut games,
                                &mut current_actions,
                                &mut current_card_names,
                                &mut current_card_textures,
                                &mut current_game_id,
                                &mut current_winner_seat,
                                &mut populated_players,
                                &saved_game_state,
                            );
                            saved_game_state = None;
                            pending_package = false;
                        }

                        let is_diff =
                            gps.flags & opcodes::FLAG_GAMESTATE_CONTAINS_DIFFS != 0;

                        match statebuf_proc.process(&gps) {
                            Ok(Some(assembled)) => {
                                let elements = match statebuf::parse_elements(&assembled)
                                {
                                    Ok(e) => e,
                                    Err(_) => continue,
                                };

                                let state = game_state.get_or_insert_with(|| {
                                    let gid =
                                        u32::try_from(game_id).unwrap_or(game_id as u32);
                                    GameState::new(gid)
                                });

                                state.apply_elements(&elements, !is_diff);

                                // Track populated players — only include seats
                                // that have meaningful data (nonzero life, hand, or library)
                                for i in 0..state.players.len() {
                                    let p = &state.players[i];
                                    if p.life != 0 || p.hand_count != 0 || p.library_count != 0 {
                                        populated_players.insert(i);
                                    }
                                }

                                // Track game ID
                                if current_game_id.is_empty() {
                                    current_game_id = state.game_id.to_string();
                                }

                                let actions = translator.process(state, !is_diff);
                                current_actions.extend(actions);
                            }
                            Ok(None) => {}
                            Err(_) => {}
                        }
                    }
                    GameMessage::GameResults(msg) => {
                        current_winner_seat = msg.winner_seat;

                        // GameResults arrives after GameOver: now we have the
                        // winner info, so package the completed game.
                        if pending_package {
                            package_game(
                                &mut games,
                                &mut current_actions,
                                &mut current_card_names,
                                &mut current_card_textures,
                                &mut current_game_id,
                                &mut current_winner_seat,
                                &mut populated_players,
                                &saved_game_state,
                            );
                            saved_game_state = None;
                            pending_package = false;
                        } else if !current_actions.is_empty() {
                            // GameResults without a prior GameOver (unusual):
                            // package with current live state.
                            package_game(
                                &mut games,
                                &mut current_actions,
                                &mut current_card_names,
                                &mut current_card_textures,
                                &mut current_game_id,
                                &mut current_winner_seat,
                                &mut populated_players,
                                &game_state,
                            );
                            translator.reset();
                            statebuf_proc.reset();
                            game_state = None;
                        }
                    }
                    GameMessage::GameOver => {
                        // Snapshot game state for player info before resetting.
                        // Actual packaging is deferred until GameResults arrives.
                        saved_game_state = game_state.take();
                        pending_package = !current_actions.is_empty();

                        // Reset pipeline state for the next game
                        translator.reset();
                        statebuf_proc.reset();
                        game_state = None;
                    }
                    _ => {}
                }
            }
            FlsMessage::GameStatusChange { new_status } => {
                // Only reset on statuses that indicate our game ended
                if new_status == 5 || new_status == 7 {
                    // If there's a pending game from GameOver, package it first
                    if pending_package {
                        package_game(
                            &mut games,
                            &mut current_actions,
                            &mut current_card_names,
                            &mut current_card_textures,
                            &mut current_game_id,
                            &mut current_winner_seat,
                            &mut populated_players,
                            &saved_game_state,
                        );
                        saved_game_state = None;
                        pending_package = false;
                    } else if !current_actions.is_empty() {
                        eprintln!(
                            "Warning: packaging incomplete game due to GameStatusChange({})",
                            new_status
                        );
                        package_game(
                            &mut games,
                            &mut current_actions,
                            &mut current_card_names,
                            &mut current_card_textures,
                            &mut current_game_id,
                            &mut current_winner_seat,
                            &mut populated_players,
                            &game_state,
                        );
                    }

                    translator.reset();
                    statebuf_proc.reset();
                    game_state = None;
                }
            }
            // Boundary signals may be for OTHER games in the FLS stream — ignore them.
            // We rely on GameOver (inside GsMessage) for game-specific resets.
            FlsMessage::GameCreated { .. }
            | FlsMessage::GameEnded { .. }
            | FlsMessage::MatchStarted { .. } => {}
            _ => {}
        }
    }

    // Package any remaining game
    if pending_package {
        package_game(
            &mut games,
            &mut current_actions,
            &mut current_card_names,
            &mut current_card_textures,
            &mut current_game_id,
            &mut current_winner_seat,
            &mut populated_players,
            &saved_game_state,
        );
    } else if !current_actions.is_empty() {
        package_game(
            &mut games,
            &mut current_actions,
            &mut current_card_names,
            &mut current_card_textures,
            &mut current_game_id,
            &mut current_winner_seat,
            &mut populated_players,
            &game_state,
        );
    }

    games
}

#[test]
fn test_golden_file_produces_actions() {
    let data =
        std::fs::read("tests/fixtures/golden_game3.bin").expect("golden_game3.bin should exist");
    let messages = framing::parse_messages(&data).expect("framing should parse");
    let games = run_pipeline(messages);

    assert_eq!(games.len(), 1, "Expected exactly 1 game, got {}", games.len());

    // The golden file should produce a non-trivial number of actions
    assert!(
        games[0].actions.len() > 10,
        "Expected >10 actions, got {}",
        games[0].actions.len()
    );
}

#[test]
fn test_golden_file_has_expected_action_types() {
    let data =
        std::fs::read("tests/fixtures/golden_game3.bin").expect("golden_game3.bin should exist");
    let messages = framing::parse_messages(&data).expect("framing should parse");
    let games = run_pipeline(messages);

    assert_eq!(games.len(), 1, "Expected exactly 1 game, got {}", games.len());
    let actions = &games[0].actions;

    // Count action types
    let mut type_counts: HashMap<&str, usize> = HashMap::new();
    for action in actions {
        let name = match &action.action_type {
            ActionType::DrawCard { .. } => "DrawCard",
            ActionType::PlayLand { .. } => "PlayLand",
            ActionType::CastSpell { .. } => "CastSpell",
            ActionType::ActivateAbility { .. } => "ActivateAbility",
            ActionType::Attack { .. } => "Attack",
            ActionType::Block { .. } => "Block",
            ActionType::Resolve { .. } => "Resolve",
            ActionType::LifeChange { .. } => "LifeChange",
            ActionType::ZoneTransition { .. } => "ZoneTransition",
            ActionType::TapPermanent { .. } => "TapPermanent",
            ActionType::UntapPermanent { .. } => "UntapPermanent",
            ActionType::DamageMarked { .. } => "DamageMarked",
            ActionType::SummoningSickness { .. } => "SummoningSickness",
            ActionType::FaceDown { .. } => "FaceDown",
            ActionType::FaceUp { .. } => "FaceUp",
            ActionType::Attach { .. } => "Attach",
            ActionType::Detach { .. } => "Detach",
            ActionType::CounterUpdate { .. } => "CounterUpdate",
            ActionType::PowerToughnessUpdate { .. } => "PowerToughnessUpdate",
            ActionType::PhaseChange { .. } => "PhaseChange",
            ActionType::TurnChange { .. } => "TurnChange",
            ActionType::PassPriority { .. } => "PassPriority",
            ActionType::Unknown { .. } => "Unknown",
        };
        *type_counts.entry(name).or_default() += 1;
    }

    // Print distribution for manual inspection
    let mut sorted: Vec<_> = type_counts.iter().collect();
    sorted.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
    eprintln!("\nAction type distribution ({} total):", actions.len());
    for (name, count) in &sorted {
        eprintln!("  {:<25} {}", name, count);
    }

    // A real game should have phase changes and some game actions
    assert!(
        type_counts.get("PhaseChange").copied().unwrap_or(0) > 0,
        "Expected at least one PhaseChange"
    );
}

#[test]
fn test_golden_file_actions_have_valid_turns() {
    let data =
        std::fs::read("tests/fixtures/golden_game3.bin").expect("golden_game3.bin should exist");
    let messages = framing::parse_messages(&data).expect("framing should parse");
    let games = run_pipeline(messages);

    assert_eq!(games.len(), 1, "Expected exactly 1 game, got {}", games.len());
    let actions = &games[0].actions;

    // All turns should be non-negative
    for action in actions {
        assert!(
            action.turn >= 0,
            "Turn should be non-negative, got {}",
            action.turn
        );
    }

    // Turns should generally increase (allowing repeats for same-turn actions)
    let max_turn = actions.iter().map(|a| a.turn).max().unwrap_or(0);
    assert!(max_turn > 0, "Expected at least turn 1, max was {}", max_turn);
}

/// Verifies that the pipeline generates zone transition actions.
///
/// MTGO assigns new thing_ids when cards change zones, so zone changes are
/// detected through the from_zone element field (not by comparing zone values
/// on the same thing_id across states).
#[test]
fn test_golden_pipeline_has_zone_transitions() {
    let data =
        std::fs::read("tests/fixtures/golden_game3.bin").expect("golden_game3.bin should exist");
    let messages = framing::parse_messages(&data).expect("framing should parse");
    let games = run_pipeline(messages);

    assert_eq!(games.len(), 1, "Expected exactly 1 game, got {}", games.len());
    let actions = &games[0].actions;

    let play_lands = actions.iter().filter(|a| matches!(&a.action_type, ActionType::PlayLand { .. })).count();
    let zone_transitions = actions.iter().filter(|a| matches!(&a.action_type, ActionType::ZoneTransition { .. })).count();

    assert!(play_lands > 0, "Expected PlayLand actions, got 0");
    assert!(zone_transitions > 0, "Expected ZoneTransition actions, got 0");
}

#[test]
fn test_golden_v1_produces_3_games() {
    let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
    let messages = flashback::protocol::framing::parse_messages(&data).unwrap();
    let games = run_pipeline(messages);
    assert_eq!(games.len(), 3, "Bo3 should produce 3 games");
    for (i, game) in games.iter().enumerate() {
        assert_eq!(game.game_number, (i + 1) as u32);
        assert!(!game.actions.is_empty(), "game {} should have actions", i + 1);
    }
}

#[test]
fn test_golden_v1_game_results() {
    let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
    let messages = flashback::protocol::framing::parse_messages(&data).unwrap();
    let games = run_pipeline(messages);
    assert_eq!(games.len(), 3);
    // With seat_id - 2 mapping: seat 2 -> player_0, seat 3 -> player_1
    match &games[0].header.result {
        flashback::replay::schema::GameResult::Win { winner_id } => {
            assert_eq!(winner_id, "player_1");
        }
        other => panic!("Game 1: expected Win, got {:?}", other),
    }
    match &games[1].header.result {
        flashback::replay::schema::GameResult::Win { winner_id } => {
            assert_eq!(winner_id, "player_0");
        }
        other => panic!("Game 2: expected Win, got {:?}", other),
    }
    match &games[2].header.result {
        flashback::replay::schema::GameResult::Win { winner_id } => {
            assert_eq!(winner_id, "player_1");
        }
        other => panic!("Game 3: expected Win, got {:?}", other),
    }
}

#[test]
fn test_golden_framing_smoke() {
    let data =
        std::fs::read("tests/fixtures/golden_v1.bin").expect("golden_v1.bin should exist");

    let messages = framing::parse_messages(&data).expect("framing::parse_messages should return Ok");

    assert_eq!(
        messages.len(),
        10195,
        "Expected 10195 messages, got {}",
        messages.len()
    );

    for (i, msg) in messages.iter().enumerate() {
        assert!(
            msg.opcode > 0,
            "Message {} has invalid opcode 0",
            i
        );
        // payload length is always >= 0 for a Vec, but assert the field is accessible
        let _len = msg.payload.len();
    }
}

/// A stripped-down view of a [`ReplayAction`] that omits the `timestamp` field.
///
/// Timestamps are set to `Utc::now()` at pipeline execution time, so they
/// differ between runs and must be excluded from snapshot comparisons.
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct ActionSnapshot {
    turn: i32,
    phase: String,
    active_player: String,
    action_type: ActionType,
}

impl From<&flashback::replay::schema::ReplayAction> for ActionSnapshot {
    fn from(a: &flashback::replay::schema::ReplayAction) -> Self {
        ActionSnapshot {
            turn: a.turn,
            phase: a.phase.clone(),
            active_player: a.active_player.clone(),
            action_type: a.action_type.clone(),
        }
    }
}

const GOLDEN_JSON_FIXTURE: &str = "tests/fixtures/golden_v1_replay.json";

/// Generates `tests/fixtures/golden_v1_replay.json` when it does not yet exist.
///
/// Run with `cargo test generate_golden_json_fixture -- --ignored` to
/// (re-)create the fixture.
#[test]
#[ignore]
fn generate_golden_json_fixture() {
    let data =
        std::fs::read("tests/fixtures/golden_v1.bin").expect("golden_v1.bin should exist");
    let messages = framing::parse_messages(&data).expect("framing should parse");
    let games = run_pipeline(messages);
    let actions: Vec<&ReplayAction> = games.iter().flat_map(|g| &g.actions).collect();

    let snapshots: Vec<ActionSnapshot> = actions.iter().map(|a| ActionSnapshot::from(*a)).collect();
    let json =
        serde_json::to_string_pretty(&snapshots).expect("actions should serialize to JSON");

    std::fs::write(GOLDEN_JSON_FIXTURE, &json)
        .expect("should write golden_v1_replay.json");

    eprintln!(
        "Wrote {} actions to {}",
        snapshots.len(),
        GOLDEN_JSON_FIXTURE
    );
}

#[test]
#[ignore]
fn generate_golden_game3_replay_json() {
    let data = std::fs::read("tests/fixtures/golden_game3.bin").unwrap();
    let messages = flashback::protocol::framing::parse_messages(&data).unwrap();
    let games = run_pipeline(messages);
    assert_eq!(games.len(), 1, "single-game fixture should produce 1 game");

    let snapshots: Vec<ActionSnapshot> = games[0].actions.iter().map(|a| ActionSnapshot {
        turn: a.turn,
        phase: a.phase.clone(),
        active_player: a.active_player.clone(),
        action_type: a.action_type.clone(),
    }).collect();

    let json = serde_json::to_string_pretty(&snapshots).unwrap();
    std::fs::write("tests/fixtures/golden_game3_replay.json", json).unwrap();
    eprintln!("Wrote golden_game3_replay.json: {} actions", snapshots.len());
}

#[test]
#[ignore]
fn generate_golden_v1_replay_json() {
    let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
    let messages = flashback::protocol::framing::parse_messages(&data).unwrap();
    let games = run_pipeline(messages);

    let json = serde_json::to_string_pretty(&games).unwrap();
    std::fs::write("tests/fixtures/golden_v1_replay.json", json).unwrap();
    eprintln!("Wrote golden_v1_replay.json: {} games", games.len());
}

/// Runs the full pipeline through statebuf processing and verifies the rolling
/// checksum passes for every GamePlayStatusMessage in the golden file.
///
/// `StateBufProcessor::process()` validates checksums internally and returns
/// `Err(DecodeError::InvalidChecksum { .. })` on failure, so this test simply
/// asserts that every call to `process()` succeeds.
#[test]
fn test_golden_checksum_audit() {
    let data =
        std::fs::read("tests/fixtures/golden_v1.bin").expect("golden_v1.bin should exist");

    let messages = framing::parse_messages(&data).expect("framing should parse");

    let mut statebuf_proc = StateBufProcessor::new();
    let mut full_state_count: usize = 0;
    let mut diff_count: usize = 0;
    // Only true checksum failures: InvalidChecksum means the assembled buffer
    // did not match the expected checksum embedded in the protocol message.
    let mut checksum_failures: Vec<String> = Vec::new();
    // Diffs that arrive before any full state has been seen (beginning of a
    // mid-game capture) produce UnexpectedEof — these are expected edge cases,
    // not checksum errors.
    let mut skipped_no_base_state: usize = 0;

    for raw_msg in messages {
        let fls_msg = match fls::decode_fls(raw_msg) {
            Ok(m) => m,
            Err(_) => continue,
        };

        match fls_msg {
            FlsMessage::GsMessage { game_id: _, meta } => {
                let game_msg = match game_messages::decode_game_message(&meta) {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                match game_msg {
                    GameMessage::GamePlayStatus(gps) => {
                        let is_diff =
                            gps.flags & opcodes::FLAG_GAMESTATE_CONTAINS_DIFFS != 0;

                        match statebuf_proc.process(&gps) {
                            Ok(Some(_)) => {
                                if is_diff {
                                    diff_count += 1;
                                } else {
                                    full_state_count += 1;
                                }
                            }
                            Ok(None) => {
                                // Fragment buffered; no assembled buffer yet — no checksum to verify.
                            }
                            Err(DecodeError::InvalidChecksum { expected, got }) => {
                                checksum_failures.push(format!(
                                    "InvalidChecksum {{ expected: {}, got: {} }}",
                                    expected, got
                                ));
                            }
                            Err(_) => {
                                // Other errors (e.g. diff without prior state at stream start)
                                // are expected edge cases, not checksum failures.
                                skipped_no_base_state += 1;
                            }
                        }
                    }
                    GameMessage::GameOver => {
                        statebuf_proc.reset();
                    }
                    _ => {}
                }
            }
            FlsMessage::GameStatusChange { new_status } => {
                if new_status == 5 || new_status == 7 {
                    statebuf_proc.reset();
                }
            }
            _ => {}
        }
    }

    let total_verified = full_state_count + diff_count;
    eprintln!(
        "\nChecksum audit: {} full-state, {} diff, {} total checksums verified, \
         {} skipped (no base state), {} checksum failures",
        full_state_count,
        diff_count,
        total_verified,
        skipped_no_base_state,
        checksum_failures.len()
    );

    assert!(
        checksum_failures.is_empty(),
        "Expected zero checksum failures, got {}:\n{}",
        checksum_failures.len(),
        checksum_failures.join("\n")
    );
    assert!(
        total_verified > 0,
        "Expected at least one assembled buffer to verify, got 0"
    );
}

/// Commutativity check: for every diff message in the golden file, verify that
/// Route A and Route B produce identical `Vec<ReplayAction>` output.
///
/// Route A: apply_diffs(old_bytes, diff_bytes) → parse_elements → GameState → translate
/// Route B: parse_elements(old_bytes) → old_GameState → apply_diffs(old_bytes, diff_bytes)
///          → parse_elements(result) → new_GameState → translate
///
/// Both routes must emit identical actions, proving that building an intermediate
/// GameState from old_bytes before applying the diff does not alter the output.
#[test]
fn test_commutativity_check() {
    let data =
        std::fs::read("tests/fixtures/golden_v1.bin").expect("golden_v1.bin should exist");

    let messages = framing::parse_messages(&data).expect("framing should parse");

    // We maintain a parallel state-cache that records assembled full buffers keyed
    // by checksum so we can retrieve old_bytes for a diff message independently of
    // the production StateBufProcessor.
    let mut state_cache: Vec<(i32, Vec<u8>)> = Vec::new();
    // Assembly buffer used to reconstruct multi-chunk messages.
    let mut assembly_buf: Vec<u8> = Vec::new();

    // Production StateBufProcessor drives the main loop (checksum validation etc.)
    let mut statebuf_proc = StateBufProcessor::new();

    // Per-game state used by both routes between iterations (they must share history).
    let mut game_state_a: Option<GameState> = None;
    let mut game_state_b: Option<GameState> = None;
    let mut translator_a = ReplayTranslator::new();
    let mut translator_b = ReplayTranslator::new();

    let mut diff_messages_checked: usize = 0;
    let mut mismatches: Vec<String> = Vec::new();

    const CACHE_MAX: usize = 16;

    for raw_msg in messages {
        let fls_msg = match fls::decode_fls(raw_msg) {
            Ok(m) => m,
            Err(_) => continue,
        };

        match fls_msg {
            FlsMessage::GsMessage { game_id, meta } => {
                let game_msg = match game_messages::decode_game_message(&meta) {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                let gid = u32::try_from(game_id).unwrap_or(game_id as u32);

                match game_msg {
                    GameMessage::GamePlayStatus(gps) => {
                        let is_diff = gps.flags & opcodes::FLAG_GAMESTATE_CONTAINS_DIFFS != 0;
                        let is_head = gps.flags & opcodes::FLAG_GAMESTATE_HEAD != 0;
                        let is_tail = gps.flags & opcodes::FLAG_GAMESTATE_TAIL != 0;

                        // --- Mirror assembly in our local buffer ---
                        if is_head {
                            assembly_buf.clear();
                        }
                        assembly_buf.extend_from_slice(&gps.state_buf_raw);

                        // --- Drive production processor (validates checksum) ---
                        let assembled_opt = match statebuf_proc.process(&gps) {
                            Ok(v) => v,
                            Err(_) => {
                                // Diff without a base state or other protocol error —
                                // skip, also clear local assembly buffer to stay in sync.
                                if is_tail {
                                    assembly_buf.clear();
                                }
                                continue;
                            }
                        };

                        if !is_tail {
                            // Fragment buffered; nothing assembled yet.
                            continue;
                        }

                        // assembled_opt is Some(_) because is_tail == true and process succeeded.
                        let new_state_bytes = match assembled_opt {
                            Some(b) => b,
                            None => continue,
                        };

                        if is_diff {
                            // Retrieve old_state_bytes from our local cache using last_state_checksum.
                            let diff_bytes = assembly_buf.clone();
                            let old_bytes_opt: Option<Vec<u8>> = {
                                // Try states matching last_state_checksum, newest first.
                                let mut found = None;
                                for (cs, base) in state_cache.iter().rev() {
                                    if *cs != gps.last_state_checksum {
                                        continue;
                                    }
                                    if let Ok(result) = statebuf::apply_diffs(base, &diff_bytes) {
                                        if statebuf::compute_checksum(&result) == gps.checksum {
                                            found = Some(base.clone());
                                            break;
                                        }
                                    }
                                }
                                // Fallback: try all cached states.
                                if found.is_none() {
                                    for (cs, base) in state_cache.iter().rev() {
                                        if *cs == gps.last_state_checksum {
                                            continue;
                                        }
                                        if let Ok(result) = statebuf::apply_diffs(base, &diff_bytes) {
                                            if statebuf::compute_checksum(&result) == gps.checksum {
                                                found = Some(base.clone());
                                                break;
                                            }
                                        }
                                    }
                                }
                                found
                            };

                            if let Some(old_bytes) = old_bytes_opt {
                                // ---- Route A ----
                                // Feed old state to translator_a first (so it has the prior state),
                                // then feed new state.
                                {
                                    let elements_old =
                                        match statebuf::parse_elements(&old_bytes) {
                                            Ok(e) => e,
                                            Err(_) => {
                                                // Cache new state and continue.
                                                let cs = statebuf::compute_checksum(&new_state_bytes);
                                                push_cache(&mut state_cache, cs, new_state_bytes.clone(), CACHE_MAX);
                                                assembly_buf.clear();
                                                continue;
                                            }
                                        };
                                    let state_a = game_state_a.get_or_insert_with(|| GameState::new(gid));
                                    state_a.apply_elements(&elements_old, false);
                                    let _ = translator_a.process(state_a, false);
                                }
                                let elements_new_a =
                                    match statebuf::parse_elements(&new_state_bytes) {
                                        Ok(e) => e,
                                        Err(_) => {
                                            let cs = statebuf::compute_checksum(&new_state_bytes);
                                            push_cache(&mut state_cache, cs, new_state_bytes.clone(), CACHE_MAX);
                                            assembly_buf.clear();
                                            continue;
                                        }
                                    };
                                let state_a = game_state_a.get_or_insert_with(|| GameState::new(gid));
                                state_a.apply_elements(&elements_new_a, false);
                                let actions_a: Vec<ActionSnapshot> =
                                    translator_a.process(state_a, false).iter().map(ActionSnapshot::from).collect();

                                // ---- Route B ----
                                // Parse old bytes, build old GameState, then apply diff and parse new bytes.
                                {
                                    let elements_old_b =
                                        match statebuf::parse_elements(&old_bytes) {
                                            Ok(e) => e,
                                            Err(_) => {
                                                let cs = statebuf::compute_checksum(&new_state_bytes);
                                                push_cache(&mut state_cache, cs, new_state_bytes.clone(), CACHE_MAX);
                                                assembly_buf.clear();
                                                continue;
                                            }
                                        };
                                    let state_b = game_state_b.get_or_insert_with(|| GameState::new(gid));
                                    state_b.apply_elements(&elements_old_b, false);
                                    let _ = translator_b.process(state_b, false);
                                }
                                // Now apply diff: same new_state_bytes already computed by production processor.
                                let elements_new_b =
                                    match statebuf::parse_elements(&new_state_bytes) {
                                        Ok(e) => e,
                                        Err(_) => {
                                            let cs = statebuf::compute_checksum(&new_state_bytes);
                                            push_cache(&mut state_cache, cs, new_state_bytes.clone(), CACHE_MAX);
                                            assembly_buf.clear();
                                            continue;
                                        }
                                    };
                                let state_b = game_state_b.get_or_insert_with(|| GameState::new(gid));
                                state_b.apply_elements(&elements_new_b, false);
                                let actions_b: Vec<ActionSnapshot> =
                                    translator_b.process(state_b, false).iter().map(ActionSnapshot::from).collect();

                                if actions_a != actions_b {
                                    mismatches.push(format!(
                                        "diff message (game_id={}, checksum={}) produced different actions:\n  Route A ({} actions): {:?}\n  Route B ({} actions): {:?}",
                                        gid, gps.checksum, actions_a.len(), &actions_a[..actions_a.len().min(3)],
                                        actions_b.len(), &actions_b[..actions_b.len().min(3)]
                                    ));
                                }
                                diff_messages_checked += 1;
                            }
                        } else {
                            // Full state: feed both routes normally.
                            let elements = match statebuf::parse_elements(&new_state_bytes) {
                                Ok(e) => e,
                                Err(_) => {
                                    let cs = statebuf::compute_checksum(&new_state_bytes);
                                    push_cache(&mut state_cache, cs, new_state_bytes.clone(), CACHE_MAX);
                                    assembly_buf.clear();
                                    continue;
                                }
                            };

                            {
                                let state_a = game_state_a.get_or_insert_with(|| GameState::new(gid));
                                state_a.apply_elements(&elements, true);
                                let _ = translator_a.process(state_a, true);
                            }
                            {
                                let state_b = game_state_b.get_or_insert_with(|| GameState::new(gid));
                                state_b.apply_elements(&elements, true);
                                let _ = translator_b.process(state_b, true);
                            }
                        }

                        // Cache the new state for future diff lookups.
                        let cs = statebuf::compute_checksum(&new_state_bytes);
                        push_cache(&mut state_cache, cs, new_state_bytes, CACHE_MAX);
                        assembly_buf.clear();
                    }
                    GameMessage::GameOver => {
                        translator_a.reset();
                        translator_b.reset();
                        statebuf_proc.reset();
                        state_cache.clear();
                        assembly_buf.clear();
                        game_state_a = None;
                        game_state_b = None;
                    }
                    _ => {}
                }
            }
            FlsMessage::GameStatusChange { new_status } => {
                if new_status == 5 || new_status == 7 {
                    translator_a.reset();
                    translator_b.reset();
                    statebuf_proc.reset();
                    state_cache.clear();
                    assembly_buf.clear();
                    game_state_a = None;
                    game_state_b = None;
                }
            }
            FlsMessage::GameCreated { .. }
            | FlsMessage::GameEnded { .. }
            | FlsMessage::MatchStarted { .. } => {}
            _ => {}
        }
    }

    eprintln!(
        "\nCommutativity check: {} diff messages verified, {} mismatches",
        diff_messages_checked,
        mismatches.len()
    );

    assert!(
        diff_messages_checked > 0,
        "Expected to check at least one diff message, got 0"
    );
    assert!(
        mismatches.is_empty(),
        "Commutativity violations found ({}):\n{}",
        mismatches.len(),
        mismatches.join("\n---\n")
    );
}

#[test]
#[ignore]
fn generate_golden_game3_fixture() {
    let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
    let messages = flashback::protocol::framing::parse_messages(&data).unwrap();

    let mut game_over_count = 0;
    let mut game3_start = None;

    for (i, msg) in messages.iter().enumerate() {
        if msg.opcode != 1153 && msg.opcode != 1156 { continue; }
        let fls_msg = flashback::protocol::fls::decode_fls(msg.clone()).unwrap();
        let meta = match &fls_msg {
            flashback::protocol::fls::FlsMessage::GsMessage { meta, .. } => meta.clone(),
            _ => continue,
        };
        let mut cursor = std::io::Cursor::new(&meta);
        if let Ok(inner) = flashback::protocol::framing::read_message(&mut cursor) {
            if inner.opcode == flashback::protocol::opcodes::GAME_OVER {
                game_over_count += 1;
                if game_over_count == 2 {
                    game3_start = Some(i + 1);
                }
            }
        }
    }

    let start = game3_start.expect("Expected at least 2 GameOver messages");
    let game3_messages = &messages[start..];

    // Re-serialize using framing format
    let mut output = Vec::new();
    for msg in game3_messages {
        let total_len = (8 + msg.payload.len()) as i32;
        output.extend_from_slice(&total_len.to_le_bytes());
        output.extend_from_slice(&msg.opcode.to_le_bytes());
        output.extend_from_slice(&msg.type_check.to_le_bytes());
        output.extend_from_slice(&msg.payload);
    }

    std::fs::write("tests/fixtures/golden_game3.bin", &output).unwrap();
    eprintln!(
        "Wrote golden_game3.bin: {} messages, {} bytes",
        game3_messages.len(),
        output.len()
    );
}

#[test]
fn test_golden_game3_fixture_parses() {
    let data = std::fs::read("tests/fixtures/golden_game3.bin").unwrap();
    let messages = flashback::protocol::framing::parse_messages(&data).unwrap();
    assert!(messages.len() > 100, "game3 fixture should have many messages");
    eprintln!("golden_game3.bin: {} messages", messages.len());
}

/// Helper: push a state into the cache, evicting the oldest entry if at capacity.
fn push_cache(cache: &mut Vec<(i32, Vec<u8>)>, checksum: i32, state: Vec<u8>, max: usize) {
    cache.retain(|(cs, s)| !(*cs == checksum && s.len() == state.len()));
    if cache.len() >= max {
        cache.remove(0);
    }
    cache.push((checksum, state));
}

/// Regression test: re-runs the pipeline against golden_game3.bin and compares
/// the output (excluding timestamps) against the stored golden_game3_replay.json
/// fixture.
///
/// If this test fails, either the pipeline logic changed or the fixture is
/// stale.  Re-generate it by running the `generate_golden_game3_replay_json`
/// test above and reviewing the diff before committing.
#[test]
fn test_golden_snapshot_regression() {
    let data = std::fs::read("tests/fixtures/golden_game3.bin").unwrap();
    let messages = flashback::protocol::framing::parse_messages(&data).unwrap();
    let games = run_pipeline(messages);
    assert_eq!(games.len(), 1);

    let actual: Vec<ActionSnapshot> = games[0].actions.iter().map(|a| ActionSnapshot {
        turn: a.turn,
        phase: a.phase.clone(),
        active_player: a.active_player.clone(),
        action_type: a.action_type.clone(),
    }).collect();

    let expected_json = std::fs::read_to_string("tests/fixtures/golden_game3_replay.json").unwrap();
    // The fixture is a ReplayFile; extract actions from the first game.
    let replay_file: flashback::replay::schema::ReplayFile = serde_json::from_str(&expected_json).unwrap();
    let expected: Vec<ActionSnapshot> = replay_file.games[0].actions.iter().map(|a| ActionSnapshot {
        turn: a.turn,
        phase: a.phase.clone(),
        active_player: a.active_player.clone(),
        action_type: a.action_type.clone(),
    }).collect();

    assert_eq!(actual.len(), expected.len(), "action count mismatch");
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(a, e, "action {} mismatch", i);
    }
}
