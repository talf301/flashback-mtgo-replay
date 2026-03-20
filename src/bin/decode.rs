// src/bin/decode.rs

//! CLI tool to decode an MTGO TCP stream into a replay JSON file.
//!
//! Usage: cargo run --bin decode -- <stream_file> [--stats-only]
//!
//! Full pipeline: framing → fls → game_messages → statebuf → state → translator → ReplayFile

use std::collections::HashMap;
use std::env;
use std::fs;
use std::process;

use chrono::Utc;

use flashback::protocol::fls::{self, FlsMessage};
use flashback::protocol::framing;
use flashback::protocol::game_messages::{self, GameMessage};
use flashback::protocol::opcodes;
use flashback::protocol::statebuf::{self, StateBufProcessor};
use flashback::replay::schema::{
    GameResult, PlayerInfo, ReplayAction, ReplayFile, ReplayHeader,
};
use flashback::state::GameState;
use flashback::translator::ReplayTranslator;

fn main() {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <stream_file> [--stats-only] [--output FILE]", args[0]);
        eprintln!();
        eprintln!("Decodes a decrypted MTGO TCP stream into a replay JSON file.");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --stats-only   Print opcode statistics only (no decode)");
        eprintln!("  --output FILE  Write replay JSON to FILE (default: stdout)");
        process::exit(1);
    }

    let path = &args[1];
    let stats_only = args.iter().any(|a| a == "--stats-only");
    let output_path = args
        .windows(2)
        .find(|w| w[0] == "--output")
        .map(|w| w[1].clone());

    let data = fs::read(path).unwrap_or_else(|e| {
        eprintln!("Failed to read {}: {}", path, e);
        process::exit(1);
    });

    eprintln!("Read {} bytes from {}", data.len(), path);

    let messages = match framing::parse_messages(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error parsing messages: {}", e);
            process::exit(1);
        }
    };

    eprintln!("Parsed {} messages", messages.len());

    if stats_only {
        print_opcode_distribution(&messages);
        print_size_stats(&messages);
        return;
    }

    // Full decode pipeline
    let replay = decode_pipeline(messages);

    let json = serde_json::to_string_pretty(&replay).unwrap_or_else(|e| {
        eprintln!("JSON serialization error: {}", e);
        process::exit(1);
    });

    if let Some(out_path) = output_path {
        fs::write(&out_path, &json).unwrap_or_else(|e| {
            eprintln!("Failed to write {}: {}", out_path, e);
            process::exit(1);
        });
        eprintln!("Wrote replay to {}", out_path);
    } else {
        println!("{}", json);
    }

    eprintln!(
        "Replay: {} actions, {} turns",
        replay.actions.len(),
        replay
            .actions
            .last()
            .map(|a| a.turn)
            .unwrap_or(0)
    );
}

fn decode_pipeline(messages: Vec<framing::RawMessage>) -> ReplayFile {
    let mut statebuf_proc = StateBufProcessor::new();
    let mut game_state: Option<GameState> = None;
    let mut translator = ReplayTranslator::new();
    let mut all_actions: Vec<ReplayAction> = Vec::new();
    let start_time = Utc::now();
    translator.set_start_time(start_time);

    let mut gs_message_count = 0u32;
    let mut game_play_status_count = 0u32;
    let mut state_update_count = 0u32;
    let mut decode_errors = 0u32;

    for raw_msg in messages {
        let fls_msg = match fls::decode_fls(raw_msg) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("FLS decode error: {}", e);
                decode_errors += 1;
                continue;
            }
        };

        match fls_msg {
            FlsMessage::GsMessage { game_id, meta } => {
                gs_message_count += 1;

                let game_msg = match game_messages::decode_game_message(&meta) {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!("Game message decode error: {}", e);
                        decode_errors += 1;
                        continue;
                    }
                };

                match game_msg {
                    GameMessage::GamePlayStatus(gps) => {
                        game_play_status_count += 1;

                        let is_diff =
                            gps.flags & opcodes::FLAG_GAMESTATE_CONTAINS_DIFFS != 0;

                        match statebuf_proc.process(&gps) {
                            Ok(Some(assembled)) => {
                                state_update_count += 1;

                                let elements = match statebuf::parse_elements(&assembled)
                                {
                                    Ok(e) => e,
                                    Err(e) => {
                                        tracing::warn!(
                                            "Element parse error: {}",
                                            e
                                        );
                                        decode_errors += 1;
                                        continue;
                                    }
                                };

                                // Initialize or update game state
                                let state = game_state.get_or_insert_with(|| {
                                    let gid =
                                        u32::try_from(game_id).unwrap_or(game_id as u32);
                                    GameState::new(gid)
                                });

                                state.apply_elements(&elements, !is_diff);

                                let actions = translator.process(state);
                                all_actions.extend(actions);
                            }
                            Ok(None) => {
                                // Waiting for more chunks
                            }
                            Err(e) => {
                                tracing::warn!("StateBuf process error: {}", e);
                                decode_errors += 1;
                            }
                        }
                    }
                    GameMessage::GameOver => {
                        tracing::info!("GameOver received");
                        translator.reset();
                        statebuf_proc.reset();
                        game_state = None;
                    }
                    GameMessage::Other { opcode } => {
                        tracing::trace!("Skipping game opcode {}", opcode);
                    }
                }
            }
            FlsMessage::GameStatusChange { new_status } => {
                if new_status == 5 || new_status == 7 {
                    tracing::info!("GameStatusChange {} — resetting", new_status);
                    translator.reset();
                    translator.set_start_time(Utc::now());
                    statebuf_proc.reset();
                    game_state = None;
                }
            }
            // Boundary signals may be for OTHER games in the FLS stream.
            // We rely on GameOver (inside GsMessage) for game-specific resets.
            FlsMessage::GameCreated { .. }
            | FlsMessage::GameEnded { .. }
            | FlsMessage::MatchStarted { .. } => {
                tracing::debug!("Boundary signal (ignored): {:?}", fls_msg);
            }
            FlsMessage::PlayerOrder { .. } => {
                tracing::debug!("PlayerOrder (no-op)");
            }
            FlsMessage::Other(_) => {}
        }
    }

    eprintln!(
        "Pipeline stats: {} GsMessages, {} GamePlayStatus, {} state updates, {} errors",
        gs_message_count, game_play_status_count, state_update_count, decode_errors
    );

    // Build ReplayFile
    let final_state = game_state.as_ref();
    let players: Vec<PlayerInfo> = if let Some(state) = final_state {
        state
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| PlayerInfo {
                player_id: format!("player_{}", i),
                name: format!("player_{}", i),
                life_total: p.life,
            })
            .collect()
    } else {
        Vec::new()
    };

    let game_id_str = final_state
        .map(|s| s.game_id.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let header = ReplayHeader {
        game_id: game_id_str,
        players,
        format: String::new(),
        start_time,
        end_time: Some(Utc::now()),
        result: GameResult::Incomplete,
    };

    let mut metadata = HashMap::new();
    metadata.insert("version".to_string(), "1.0".to_string());
    metadata.insert("decoder".to_string(), "flashback".to_string());
    metadata.insert(
        "decode_errors".to_string(),
        decode_errors.to_string(),
    );

    ReplayFile {
        header,
        actions: all_actions,
        metadata,
    }
}

fn print_opcode_distribution(messages: &[framing::RawMessage]) {
    let mut counts: HashMap<u16, usize> = HashMap::new();
    for msg in messages {
        *counts.entry(msg.opcode).or_default() += 1;
    }

    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by_key(|&(_, count)| std::cmp::Reverse(count));

    println!("Opcode distribution:");
    for (opcode, count) in &sorted {
        println!("  {:>5}  {:<30}  {}", opcode, opcode_name(*opcode), count);
    }
    println!();
}

fn print_size_stats(messages: &[framing::RawMessage]) {
    if messages.is_empty() {
        return;
    }

    let sizes: Vec<usize> = messages.iter().map(|m| m.payload.len()).collect();
    let total: usize = sizes.iter().sum();
    let min = sizes.iter().min().unwrap();
    let max = sizes.iter().max().unwrap();
    let avg = total / sizes.len();

    println!("Payload size stats:");
    println!(
        "  Total: {} bytes across {} messages",
        total,
        sizes.len()
    );
    println!(
        "  Min: {} bytes, Max: {} bytes, Avg: {} bytes",
        min, max, avg
    );
}

fn opcode_name(opcode: u16) -> &'static str {
    match opcode {
        opcodes::GS_CONNECT => "GsConnectMessage",
        opcodes::GSH_GAME_STATUS_CHANGE => "GshGameStatusChangeMessage",
        opcodes::GS_MESSAGE => "GsMessageMessage",
        opcodes::GS_PLAYER_ORDER => "GsPlayerOrderMessage",
        opcodes::GS_REPLAY_MESSAGE => "GsReplayMessageMessage",
        opcodes::GAME_RESULTS => "GameResultsMessage",
        opcodes::GAME_OVER => "GameOverMessage",
        opcodes::GAME_NEXT_STEP => "GameNextStepMessage",
        opcodes::NEW_CARD_ACTION => "NewCardActionMessage",
        opcodes::CARD_ACTION => "CardActionMessage",
        opcodes::GAME_PLAY_STATUS => "GamePlayStatusMessage",
        opcodes::V3_REPLAY_USER_ORDER => "V3ReplayUserOrderMessage",
        _ => "(unknown)",
    }
}
