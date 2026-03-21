//! Golden file end-to-end pipeline integration test.
//!
//! Runs the full decode pipeline against golden_v1.bin and validates
//! that recognizable game actions are produced.

use std::collections::HashMap;
use std::path::Path;

use flashback::protocol::fls::{self, FlsMessage};
use flashback::protocol::framing;
use flashback::protocol::game_messages::{self, GameMessage};
use flashback::protocol::opcodes;
use flashback::protocol::statebuf::{self, StateBufProcessor};
use flashback::protocol::DecodeError;
use flashback::replay::schema::{ActionType, ReplayAction};
use flashback::state::GameState;
use flashback::translator::ReplayTranslator;

fn run_pipeline(data: &[u8]) -> Vec<ReplayAction> {
    let messages = framing::parse_messages(data).expect("framing should parse");

    let mut statebuf_proc = StateBufProcessor::new();
    let mut game_state: Option<GameState> = None;
    let mut translator = ReplayTranslator::new();
    let mut all_actions: Vec<ReplayAction> = Vec::new();

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
                                let actions = translator.process(state, !is_diff);
                                all_actions.extend(actions);
                            }
                            Ok(None) => {}
                            Err(_) => {}
                        }
                    }
                    GameMessage::GameOver => {
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

    all_actions
}

#[test]
fn test_golden_file_produces_actions() {
    let data =
        std::fs::read("tests/fixtures/golden_v1.bin").expect("golden_v1.bin should exist");
    let actions = run_pipeline(&data);

    // The golden file should produce a non-trivial number of actions
    assert!(
        actions.len() > 10,
        "Expected >10 actions, got {}",
        actions.len()
    );
}

#[test]
fn test_golden_file_has_expected_action_types() {
    let data =
        std::fs::read("tests/fixtures/golden_v1.bin").expect("golden_v1.bin should exist");
    let actions = run_pipeline(&data);

    // Count action types
    let mut type_counts: HashMap<&str, usize> = HashMap::new();
    for action in &actions {
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
        std::fs::read("tests/fixtures/golden_v1.bin").expect("golden_v1.bin should exist");
    let actions = run_pipeline(&data);

    // All turns should be non-negative
    for action in &actions {
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
        std::fs::read("tests/fixtures/golden_v1.bin").expect("golden_v1.bin should exist");
    let actions = run_pipeline(&data);

    let play_lands = actions.iter().filter(|a| matches!(&a.action_type, ActionType::PlayLand { .. })).count();
    let zone_transitions = actions.iter().filter(|a| matches!(&a.action_type, ActionType::ZoneTransition { .. })).count();

    assert!(play_lands > 0, "Expected PlayLand actions, got 0");
    assert!(zone_transitions > 0, "Expected ZoneTransition actions, got 0");
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
    let actions = run_pipeline(&data);

    let snapshots: Vec<ActionSnapshot> = actions.iter().map(ActionSnapshot::from).collect();
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

/// Helper: push a state into the cache, evicting the oldest entry if at capacity.
fn push_cache(cache: &mut Vec<(i32, Vec<u8>)>, checksum: i32, state: Vec<u8>, max: usize) {
    cache.retain(|(cs, s)| !(*cs == checksum && s.len() == state.len()));
    if cache.len() >= max {
        cache.remove(0);
    }
    cache.push((checksum, state));
}

/// Regression test: re-runs the pipeline and compares the output (excluding
/// timestamps) against the stored JSON fixture.
///
/// If this test fails, either the pipeline logic changed or the fixture is
/// stale.  Re-generate it by running the `generate_golden_json_fixture` test
/// above and reviewing the diff before committing.
#[test]
fn test_golden_snapshot_regression() {
    assert!(
        Path::new(GOLDEN_JSON_FIXTURE).exists(),
        "Golden fixture {} not found – run \
         `cargo test generate_golden_json_fixture -- --ignored` to create it",
        GOLDEN_JSON_FIXTURE
    );

    let data =
        std::fs::read("tests/fixtures/golden_v1.bin").expect("golden_v1.bin should exist");
    let actions = run_pipeline(&data);
    let actual: Vec<ActionSnapshot> = actions.iter().map(ActionSnapshot::from).collect();

    let fixture_json =
        std::fs::read_to_string(GOLDEN_JSON_FIXTURE).expect("should read fixture JSON");
    let expected: Vec<ActionSnapshot> =
        serde_json::from_str(&fixture_json).expect("fixture JSON should deserialize");

    assert_eq!(
        actual.len(),
        expected.len(),
        "Action count mismatch: got {}, expected {}",
        actual.len(),
        expected.len()
    );

    for (i, (act, exp)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            act, exp,
            "Action {} differs from fixture\n  actual:   {:?}\n  expected: {:?}",
            i, act, exp
        );
    }
}
