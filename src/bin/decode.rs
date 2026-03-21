// src/bin/decode.rs

//! CLI tool to decode an MTGO TCP stream into a replay JSON file.
//!
//! Usage: cargo run --bin decode -- <stream_file> [--stats-only]
//!
//! Full pipeline: framing → fls → game_messages → statebuf → state → translator → ReplayFile

use std::collections::{HashMap, HashSet};
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
    GameHeader, GameReplay, GameResult, PlayerInfo, ReplayAction, ReplayFile, ReplayHeader,
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
        "Replay: {} games, {} total actions",
        replay.games.len(),
        replay.games.iter().map(|g| g.actions.len()).sum::<usize>()
    );
}

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
        Some(seat) if seat >= 2 && (seat as usize - 2) < players.len() => GameResult::Win {
            winner_id: format!("player_{}", seat - 2),
        },
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

    tracing::info!(
        "Packaged game {} with {} actions",
        game.game_number,
        game.actions.len()
    );

    games.push(game);
    *current_winner_seat = None;
    populated_players.clear();
}

fn decode_pipeline(messages: Vec<framing::RawMessage>) -> ReplayFile {
    let mut statebuf_proc = StateBufProcessor::new();
    let mut game_state: Option<GameState> = None;
    let mut translator = ReplayTranslator::new();
    let start_time = Utc::now();
    translator.set_start_time(start_time);

    // Per-game accumulators
    let mut games: Vec<GameReplay> = Vec::new();
    let mut current_actions: Vec<ReplayAction> = Vec::new();
    let mut current_card_names: HashMap<String, String> = HashMap::new();
    let mut current_card_textures: HashMap<String, i32> = HashMap::new();
    let mut current_game_id: String = String::new();
    let mut current_winner_seat: Option<u8> = None;
    let mut populated_players: HashSet<usize> = HashSet::new();

    // Deferred packaging: GameOver arrives before GameResults in the protocol
    // stream, so on GameOver we snapshot the game state, reset the pipeline,
    // and defer packaging until GameResults arrives with winner info.
    let mut saved_game_state: Option<GameState> = None;
    let mut pending_package = false;

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

                                // Track populated players
                                for i in 0..state.players.len() {
                                    populated_players.insert(i);
                                }

                                // Track game ID
                                if current_game_id.is_empty() {
                                    current_game_id = state.game_id.to_string();
                                }

                                // Collect card names and texture IDs
                                for (thing_id, thing) in &state.things {
                                    let tid = thing_id.to_string();
                                    if let Some(ref name) = thing.card_name {
                                        current_card_names
                                            .entry(tid.clone())
                                            .or_insert_with(|| name.clone());
                                    }
                                    if let Some(tex) = thing.card_texture_number {
                                        let mtgo_id = tex / 2;
                                        current_card_textures
                                            .entry(tid)
                                            .or_insert(mtgo_id);
                                    }
                                }

                                let actions = translator.process(state, !is_diff);
                                current_actions.extend(actions);
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
                        // Snapshot game state for player info before resetting.
                        // Actual packaging is deferred until GameResults arrives.
                        saved_game_state = game_state.take();
                        pending_package = !current_actions.is_empty();

                        // Reset pipeline state for the next game
                        translator.reset();
                        statebuf_proc.reset();
                        game_state = None;
                    }
                    GameMessage::GameResults(gr) => {
                        tracing::info!(
                            "GameResults: game_id={} winner_seat={:?}",
                            gr.game_id,
                            gr.winner_seat
                        );
                        current_winner_seat = gr.winner_seat;

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
                            tracing::warn!(
                                "GameResults without prior GameOver — packaging with live state"
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
                            translator.reset();
                            statebuf_proc.reset();
                            game_state = None;
                        }
                    }
                    GameMessage::Other { opcode } => {
                        tracing::trace!("Skipping game opcode {}", opcode);
                    }
                }
            }
            FlsMessage::GameStatusChange { new_status } => {
                if new_status == 5 || new_status == 7 {
                    tracing::info!("GameStatusChange {} — resetting", new_status);

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
                        tracing::warn!(
                            "Packaging incomplete game due to GameStatusChange({})",
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

    eprintln!(
        "Pipeline stats: {} GsMessages, {} GamePlayStatus, {} state updates, {} errors",
        gs_message_count, game_play_status_count, state_update_count, decode_errors
    );

    // Build header from the first game's players (or empty)
    let players = games
        .first()
        .map(|g| g.header.players.clone())
        .unwrap_or_default();

    let header = ReplayHeader {
        players,
        format: String::new(),
        start_time,
        end_time: Some(Utc::now()),
    };

    let mut metadata = HashMap::new();
    metadata.insert("version".to_string(), "2.0".to_string());
    metadata.insert("decoder".to_string(), "flashback".to_string());
    metadata.insert(
        "decode_errors".to_string(),
        decode_errors.to_string(),
    );

    ReplayFile {
        header,
        games,
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
