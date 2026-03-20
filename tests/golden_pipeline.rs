//! Golden file end-to-end pipeline integration test.
//!
//! Runs the full decode pipeline against golden_v1.bin and validates
//! that recognizable game actions are produced.

use std::collections::HashMap;

use flashback::protocol::fls::{self, FlsMessage};
use flashback::protocol::framing;
use flashback::protocol::game_messages::{self, GameMessage};
use flashback::protocol::opcodes;
use flashback::protocol::statebuf::{self, StateBufProcessor};
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
                                let actions = translator.process(state);
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
