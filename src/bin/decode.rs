// src/bin/decode.rs

//! CLI tool to parse a decrypted MTGO TCP stream file.
//!
//! Usage: cargo run --bin decode -- <stream_file>
//!
//! Reads raw bytes from a file, parses MTGO message framing,
//! and reports opcode distribution statistics.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::process;

use flashback::protocol::framing;
use flashback::protocol::opcodes;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <stream_file>", args[0]);
        eprintln!();
        eprintln!("Reads a decrypted MTGO TCP stream and parses message framing.");
        eprintln!("The input should be raw application-layer bytes from a decrypted");
        eprintln!("TCP stream to MTGO server port 7770.");
        eprintln!();
        eprintln!("To produce this file, see the capture procedure in the Phase A plan.");
        process::exit(1);
    }

    let path = &args[1];
    let data = fs::read(path).unwrap_or_else(|e| {
        eprintln!("Failed to read {}: {}", path, e);
        process::exit(1);
    });

    println!("Read {} bytes from {}", data.len(), path);

    match framing::parse_messages(&data) {
        Ok(messages) => {
            println!("Parsed {} messages\n", messages.len());
            print_opcode_distribution(&messages);
            print_size_stats(&messages);
        }
        Err(e) => {
            eprintln!("Error parsing messages: {}", e);
            process::exit(1);
        }
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
    println!("  Total: {} bytes across {} messages", total, sizes.len());
    println!("  Min: {} bytes, Max: {} bytes, Avg: {} bytes", min, max, avg);
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
