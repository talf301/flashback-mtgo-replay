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
use flashback::protocol::statebuf::{self, StateElement, StateBufProcessor};
use flashback::replay::schema::{
    ActionType, GameHeader, GameReplay, GameResult, PlayerInfo, ReplayAction, ReplayFile,
    ReplayHeader,
};
use flashback::scryfall;
use flashback::state::GameState;
use flashback::translator::ReplayTranslator;

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <stream_file> [--stats-only] [--output FILE] [--text-log]", args[0]);
        eprintln!();
        eprintln!("Decodes a decrypted MTGO TCP stream into a replay JSON file.");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  --stats-only   Print opcode statistics only (no decode)");
        eprintln!("  --output FILE  Write replay JSON to FILE (default: stdout)");
        eprintln!("  --text-log     Print human-readable game log instead of JSON");
        eprintln!("  --no-resolve   Skip Scryfall card name lookups (offline mode)");
        process::exit(1);
    }

    let path = &args[1];
    let stats_only = args.iter().any(|a| a == "--stats-only");
    let text_log = args.iter().any(|a| a == "--text-log");
    let no_resolve = args.iter().any(|a| a == "--no-resolve");
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
    let mut replay = decode_pipeline(messages);

    if !no_resolve {
        resolve_card_names(&mut replay);
    }

    if text_log {
        print_text_log(&replay);
        return;
    }

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

    // Player name → seat index mapping, built from MASTER_USER_LIST messages.
    let mut player_seat_map: HashMap<String, usize> = HashMap::new();

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

                        tracing::debug!(
                            "GamePlayStatus: waiting_for={} priority={}",
                            gps.player_waiting_for, gps.priority_player
                        );

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

                                // Debug: log each element type and key data
                                for elem in &elements {
                                    match elem {
                                        StateElement::PlayerStatus(ps) => {
                                            // Show only slots with nonzero life or hand
                                            let active_seats: Vec<_> = (0..ps.life.len())
                                                .filter(|&i| ps.life[i] != 0 || ps.hand_count[i] != 0 || ps.library_count[i] != 0)
                                                .map(|i| format!(
                                                    "seat{}(life={} hand={} lib={} gy={})",
                                                    i, ps.life[i], ps.hand_count[i],
                                                    ps.library_count[i], ps.graveyard_count[i]
                                                ))
                                                .collect();
                                            tracing::debug!(
                                                "PlayerStatus: active_player={} seats=[{}]",
                                                ps.active_player,
                                                active_seats.join(", ")
                                            );
                                        }
                                        StateElement::TurnStep(ts) => {
                                            tracing::debug!(
                                                "TurnStep: turn={} phase={} prompted={:?}",
                                                ts.turn_number, ts.phase, ts.prompted_player
                                            );
                                        }
                                        StateElement::Thing(te) => {
                                            let thing_id = te.props.get(&opcodes::THINGNUMBER);
                                            let zone = te.props.get(&opcodes::ZONE);
                                            let controller = te.props.get(&opcodes::CONTROLLER);
                                            let name = te.props.get(&opcodes::CARDNAME_STRING);
                                            let texture = te.props.get(&opcodes::CARDTEXTURE_NUMBER);
                                            tracing::debug!(
                                                "Thing: id={:?} zone={:?} ctrl={:?} name={:?} tex={:?} from_zone={} props_count={}",
                                                thing_id, zone, controller, name, texture,
                                                te.from_zone, te.props.len()
                                            );
                                        }
                                        StateElement::Other { element_type, raw } => {
                                            tracing::debug!(
                                                "Other element: type={} len={}",
                                                element_type, raw.len()
                                            );
                                        }
                                    }
                                }

                                // Initialize or update game state
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
                    GameMessage::UserList { ref players } => {
                        if players.len() >= 2 && player_seat_map.is_empty() {
                            let names: Vec<String> = players.iter()
                                .map(|p| p.name.clone())
                                .collect();
                            for (i, p) in players.iter().enumerate() {
                                player_seat_map.insert(p.name.clone(), i);
                            }
                            translator.set_player_names(names);
                            tracing::info!(
                                "Player seats: {}",
                                players.iter().enumerate()
                                    .map(|(i, p)| format!("seat{}={}", i, p.name))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                        }
                    }
                    GameMessage::UserChat { ref text } => {
                        tracing::debug!("CHAT: {}", text);
                        translator.ingest_chat(text);
                        // Parse "Turn N: PlayerName" to set active player
                        if let Some(rest) = text.strip_prefix("Turn ") {
                            if let Some(colon_pos) = rest.find(": ") {
                                let player_name = &rest[colon_pos + 2..];
                                if let Some(&seat) = player_seat_map.get(player_name) {
                                    if let Some(ref mut state) = game_state {
                                        state.active_player = seat;
                                    }
                                    tracing::debug!(
                                        "Turn owner: {} (seat {})", player_name, seat
                                    );
                                }
                            }
                        }
                    }
                    GameMessage::Other { opcode, .. } => {
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
            FlsMessage::PlayerOrder { ref raw } => {
                tracing::debug!("PlayerOrder: raw={:?} (len={})", raw, raw.len());
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

/// Resolve missing card names from Scryfall using texture IDs.
fn resolve_card_names(replay: &mut ReplayFile) {
    // Collect all unique MTGO IDs that lack names across all games
    let mut ids_to_resolve: Vec<i32> = Vec::new();
    for game in &replay.games {
        for (thing_id, &mtgo_id) in &game.card_textures {
            if !game.card_names.contains_key(thing_id) {
                ids_to_resolve.push(mtgo_id);
            }
        }
    }
    ids_to_resolve.sort_unstable();
    ids_to_resolve.dedup();

    if ids_to_resolve.is_empty() {
        return;
    }

    eprintln!(
        "Resolving {} unique card IDs via Scryfall...",
        ids_to_resolve.len()
    );

    let mut cache = std::collections::HashMap::new();
    let resolved = scryfall::resolve_mtgo_ids(&ids_to_resolve, &mut cache);

    eprintln!(
        "Resolved {}/{} card names",
        resolved.len(),
        ids_to_resolve.len()
    );

    // Fill in card_names for each game
    for game in &mut replay.games {
        for (thing_id, &mtgo_id) in &game.card_textures {
            if !game.card_names.contains_key(thing_id) {
                if let Some(name) = resolved.get(&mtgo_id) {
                    game.card_names.insert(thing_id.clone(), name.clone());
                }
            }
        }
    }
}

fn card_label(card_id: &str, names: &HashMap<String, String>) -> String {
    match names.get(card_id) {
        Some(name) => format!("{} ({})", name, card_id),
        None => format!("#{}", card_id),
    }
}

fn print_text_log(replay: &ReplayFile) {
    for game in &replay.games {
        let names = &game.card_names;
        let players = &game.header.players;

        // Header
        println!("{}", "=".repeat(60));
        println!(
            "  GAME {} — {}",
            game.game_number,
            match &game.header.result {
                GameResult::Win { winner_id } => {
                    let name = players
                        .iter()
                        .find(|p| &p.player_id == winner_id)
                        .map(|p| p.name.as_str())
                        .unwrap_or(winner_id);
                    format!("{} wins", name)
                }
                GameResult::Draw => "Draw".to_string(),
                GameResult::Incomplete => "Incomplete".to_string(),
            }
        );
        println!(
            "  Players: {}",
            players
                .iter()
                .map(|p| format!("{} ({}hp)", p.name, p.life_total))
                .collect::<Vec<_>>()
                .join(" vs ")
        );
        println!("  Actions: {}", game.actions.len());
        println!("  Cards identified: {}/{} names, {} textures",
            names.len(),
            game.card_textures.len() + names.len() - names.len(), // total unique cards
            game.card_textures.len(),
        );
        println!("{:=<60}", "");
        println!();

        let mut last_turn: i32 = -1;
        let mut last_phase = String::new();

        for action in &game.actions {
            // Skip pre-game setup noise
            if action.turn == 0 && action.phase.starts_with("unknown") {
                continue;
            }

            // Turn header
            if action.turn != last_turn {
                last_turn = action.turn;
                last_phase.clear();
                println!("--- Turn {} ({}) ---", action.turn, action.active_player);
            }

            // Phase header (deduplicated)
            if action.phase != last_phase {
                if !matches!(&action.action_type, ActionType::PhaseChange { .. } | ActionType::TurnChange { .. }) {
                    // Phase changed implicitly — show it
                    last_phase = action.phase.clone();
                    println!("  [{}]", action.phase);
                }
            }

            match &action.action_type {
                ActionType::PhaseChange { phase } => {
                    last_phase = phase.clone();
                    println!("  [{}]", phase);
                }
                ActionType::TurnChange { .. } => {
                    // Already handled by the turn header above
                }
                ActionType::DrawCard {
                    player_id,
                    card_id,
                } => {
                    println!("    {} draws {}", player_id, card_label(card_id, names));
                }
                ActionType::PlayLand {
                    player_id,
                    card_id,
                } => {
                    println!(
                        "    {} plays land {}",
                        player_id,
                        card_label(card_id, names)
                    );
                }
                ActionType::CastSpell {
                    player_id,
                    card_id,
                } => {
                    println!(
                        "    {} casts {}",
                        player_id,
                        card_label(card_id, names)
                    );
                }
                ActionType::ActivateAbility {
                    player_id,
                    card_id,
                    ability_id,
                } => {
                    println!(
                        "    {} activates {} (ability {})",
                        player_id,
                        card_label(card_id, names),
                        ability_id
                    );
                }
                ActionType::Attack {
                    attacker_id,
                    defender_id,
                } => {
                    println!(
                        "    {} attacks {}",
                        card_label(attacker_id, names),
                        defender_id
                    );
                }
                ActionType::Block {
                    attacker_id,
                    blocker_id,
                } => {
                    let atk = if attacker_id.is_empty() {
                        "?".to_string()
                    } else {
                        card_label(attacker_id, names)
                    };
                    println!(
                        "    {} blocks {}",
                        card_label(blocker_id, names),
                        atk
                    );
                }
                ActionType::Resolve { card_id } => {
                    println!("    {} resolves", card_label(card_id, names));
                }
                ActionType::LifeChange {
                    player_id,
                    old_life,
                    new_life,
                } => {
                    let delta = new_life - old_life;
                    let sign = if delta > 0 { "+" } else { "" };
                    println!(
                        "    {} life: {} -> {} ({}{})",
                        player_id, old_life, new_life, sign, delta
                    );
                }
                ActionType::Discard {
                    player_id,
                    card_id,
                } => {
                    println!(
                        "    {} discards {}",
                        player_id,
                        card_label(card_id, names)
                    );
                }
                ActionType::Mill {
                    player_id,
                    card_id,
                } => {
                    println!(
                        "    {} mills {}",
                        player_id,
                        card_label(card_id, names)
                    );
                }
                ActionType::CreateToken {
                    player_id,
                    card_id,
                    token_name,
                } => {
                    println!(
                        "    {} creates {} ({})",
                        player_id,
                        token_name,
                        card_label(card_id, names)
                    );
                }
                ActionType::ZoneTransition {
                    card_id,
                    from_zone,
                    to_zone,
                    player_id,
                } => {
                    let who = player_id.as_deref().unwrap_or("?");
                    println!(
                        "    [{}] {} moves {} -> {}",
                        who,
                        card_label(card_id, names),
                        from_zone,
                        to_zone
                    );
                }
                ActionType::TapPermanent { card_id } => {
                    println!("    tap {}", card_label(card_id, names));
                }
                ActionType::UntapPermanent { card_id } => {
                    println!("    untap {}", card_label(card_id, names));
                }
                ActionType::DamageMarked { card_id, damage } => {
                    println!(
                        "    {} takes {} damage",
                        card_label(card_id, names),
                        damage
                    );
                }
                ActionType::SummoningSickness {
                    card_id,
                    has_sickness,
                } => {
                    if *has_sickness {
                        println!("    {} enters (summoning sick)", card_label(card_id, names));
                    }
                    // Don't log sickness wearing off — it's noise
                }
                ActionType::FaceDown { card_id } => {
                    println!("    {} turned face down", card_label(card_id, names));
                }
                ActionType::FaceUp { card_id } => {
                    println!("    {} turned face up", card_label(card_id, names));
                }
                ActionType::Attach {
                    card_id,
                    attached_to_id,
                } => {
                    println!(
                        "    {} attached to {}",
                        card_label(card_id, names),
                        card_label(attached_to_id, names)
                    );
                }
                ActionType::Detach { card_id } => {
                    println!("    {} detached", card_label(card_id, names));
                }
                ActionType::CounterUpdate {
                    card_id,
                    counter_type,
                    count,
                } => {
                    println!(
                        "    {} now has {} {} counter(s)",
                        card_label(card_id, names),
                        count,
                        counter_type
                    );
                }
                ActionType::PowerToughnessUpdate {
                    card_id,
                    power,
                    toughness,
                } => {
                    println!(
                        "    {} is now {}/{}",
                        card_label(card_id, names),
                        power,
                        toughness
                    );
                }
                ActionType::PassPriority { player_id } => {
                    println!("    {} passes priority", player_id);
                }
                ActionType::Unknown { description } => {
                    println!("    ??? {}", description);
                }
            }
        }
        println!();
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
