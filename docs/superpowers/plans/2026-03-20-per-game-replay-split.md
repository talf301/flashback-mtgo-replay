# Per-Game Replay Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the replay pipeline from one flat action list into per-game `GameReplay` objects, decode `GAME_RESULTS` for winner detection, create a single-game golden fixture, and update the web viewer for multi-game support.

**Architecture:** The Rust schema adds a `GameReplay` wrapper and `GameHeader`. The decode pipeline accumulates per-game state and packages on `GameOver`. A new `GameResults` message variant extracts winner info from opcode 4485. The TypeScript viewer adds a game selector and scopes card data per-game.

**Tech Stack:** Rust (serde, chrono), TypeScript/React, cargo test, vitest

---

## File Map

### Rust — Create
- `src/protocol/game_results.rs` — `GAME_RESULTS` message decoder (new module, single responsibility)

### Rust — Modify
- `src/replay/schema.rs` — Add `GameReplay`, `GameHeader`; restructure `ReplayFile` and `ReplayHeader`
- `src/protocol/game_messages.rs` — Add `GameResults` variant, dispatch to new decoder
- `src/protocol/mod.rs` — Add `pub mod game_results;`
- `src/bin/decode.rs` — Per-game accumulator loop, seat-to-player mapping, result packaging
- `tests/golden_pipeline.rs` — Return `Vec<GameReplay>`, add multi-game and single-game tests

### TypeScript — Modify
- `web/src/types/replay.ts` — Add `GameReplay`, `GameHeader`; update `ReplayFile`
- `web/src/engine/reconstructor.ts` — `loadReplay(replay, gameIndex)` signature
- `web/src/App.tsx` — Game selector UI, per-game loading

### Fixtures — Generate (via `#[ignore]` tests)
- `tests/fixtures/golden_game3.bin` — Single-game binary fixture
- `tests/fixtures/golden_game3_replay.json` — Expected actions for game 3
- `tests/fixtures/golden_v1_replay.json` — Updated to multi-game schema

---

## Task 1: Decode GAME_RESULTS message

**Files:**
- Create: `src/protocol/game_results.rs`
- Modify: `src/protocol/game_messages.rs:26-42`
- Modify: `src/protocol/mod.rs`

- [ ] **Step 1: Write the failing test for GAME_RESULTS decoding**

In `src/protocol/game_results.rs`:

```rust
//! GAME_RESULTS message decoder (opcode 4485).

use std::io::{Cursor, Read};
use super::DecodeError;

/// Decoded GAME_RESULTS payload.
#[derive(Debug, Clone)]
pub struct GameResultsMessage {
    pub game_id: u32,
    pub winner_seat: Option<u8>, // None = draw or unparseable
    pub players: Vec<GameResultPlayer>,
}

#[derive(Debug, Clone)]
pub struct GameResultPlayer {
    pub seat_id: u8,
    pub rating: u16,
}

pub fn decode_game_results(payload: &[u8]) -> Result<GameResultsMessage, DecodeError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn golden_game1_payload() -> Vec<u8> {
        vec![
            0x6e, 0x00, 0x5d, 0x38, // game_id
            0x01, 0x00, 0x00, 0x00, // winner_index = 1
            0x00,                   // unknown
            0x02, 0x00,             // player_count = 2
            0x00, 0x00,             // unknown
            0x02, 0x67, 0x04, 0x00, 0x00, // seat 2, rating 1127
            0x03, 0x96, 0x04, 0x00, 0x00, // seat 3, rating 1174
        ]
    }

    #[test]
    fn test_decode_game_results_winner() {
        let msg = decode_game_results(&golden_game1_payload()).unwrap();
        assert_eq!(msg.game_id, 945619054);
        assert_eq!(msg.winner_seat, Some(3)); // seat 3 = entries[1]
        assert_eq!(msg.players.len(), 2);
        assert_eq!(msg.players[0].seat_id, 2);
        assert_eq!(msg.players[0].rating, 1127);
        assert_eq!(msg.players[1].seat_id, 3);
        assert_eq!(msg.players[1].rating, 1174);
    }

    #[test]
    fn test_decode_game_results_draw_sentinel() {
        // winner_index = player_count (out of bounds) → Draw
        let mut payload = golden_game1_payload();
        payload[4] = 0x02; // winner_index = 2, but only 2 players
        let msg = decode_game_results(&payload).unwrap();
        assert_eq!(msg.winner_seat, None);
    }

    #[test]
    fn test_decode_game_results_empty_payload() {
        let result = decode_game_results(&[]);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `~/.cargo/bin/cargo test game_results -- -v 2>&1 | tail -20`
Expected: FAIL with `not yet implemented`

- [ ] **Step 3: Implement the decoder**

Replace the `todo!()` in `decode_game_results`:

```rust
pub fn decode_game_results(payload: &[u8]) -> Result<GameResultsMessage, DecodeError> {
    let mut cursor = Cursor::new(payload);
    let mut buf4 = [0u8; 4];
    let mut buf2 = [0u8; 2];
    let mut buf1 = [0u8; 1];

    // game_id: u32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GameResults: game_id",
    })?;
    let game_id = u32::from_le_bytes(buf4);

    // winner_index: u32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GameResults: winner_index",
    })?;
    let winner_index = u32::from_le_bytes(buf4);

    // unknown byte
    cursor.read_exact(&mut buf1).map_err(|_| DecodeError::UnexpectedEof {
        context: "GameResults: unknown byte",
    })?;

    // player_count: u16
    cursor.read_exact(&mut buf2).map_err(|_| DecodeError::UnexpectedEof {
        context: "GameResults: player_count",
    })?;
    let player_count = u16::from_le_bytes(buf2) as usize;

    // unknown u16
    cursor.read_exact(&mut buf2).map_err(|_| DecodeError::UnexpectedEof {
        context: "GameResults: unknown u16",
    })?;

    // Player entries: 5 bytes each (u8 seat_id, u16 rating, u16 padding)
    let mut players = Vec::with_capacity(player_count);
    for _ in 0..player_count {
        cursor.read_exact(&mut buf1).map_err(|_| DecodeError::UnexpectedEof {
            context: "GameResults: player seat_id",
        })?;
        let seat_id = buf1[0];

        cursor.read_exact(&mut buf2).map_err(|_| DecodeError::UnexpectedEof {
            context: "GameResults: player rating",
        })?;
        let rating = u16::from_le_bytes(buf2);

        // padding
        cursor.read_exact(&mut buf2).map_err(|_| DecodeError::UnexpectedEof {
            context: "GameResults: player padding",
        })?;

        players.push(GameResultPlayer { seat_id, rating });
    }

    let winner_seat = if (winner_index as usize) < player_count {
        Some(players[winner_index as usize].seat_id)
    } else {
        None // Draw or unknown
    };

    Ok(GameResultsMessage {
        game_id,
        winner_seat,
        players,
    })
}
```

- [ ] **Step 4: Wire up the module and GameMessage variant**

In `src/protocol/mod.rs`, add `pub mod game_results;`.

In `src/protocol/game_messages.rs`, add the variant and dispatch:

```rust
// Add to imports at top:
use super::game_results::{self, GameResultsMessage};

// Add to GameMessage enum:
pub enum GameMessage {
    GamePlayStatus(GamePlayStatusMessage),
    GameResults(GameResultsMessage),
    GameOver,
    Other { opcode: u16 },
}

// Add to decode_game_message match:
opcodes::GAME_RESULTS => {
    game_results::decode_game_results(&inner.payload)
        .map(GameMessage::GameResults)
}
```

- [ ] **Step 5: Run tests to verify everything passes**

Run: `~/.cargo/bin/cargo test game_results -- -v 2>&1 | tail -20`
Expected: 3 tests PASS

- [ ] **Step 6: Add golden file integration test**

Add to the test module in `src/protocol/game_results.rs`:

```rust
#[test]
fn test_golden_file_has_3_game_results() {
    use crate::protocol::{fls, framing, opcodes};
    let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
    let messages = framing::parse_messages(&data).unwrap();

    let mut results = Vec::new();
    for msg in &messages {
        if msg.opcode != 1153 && msg.opcode != 1156 { continue; }
        let fls_msg = fls::decode_fls(msg.clone()).unwrap();
        let meta = match fls_msg {
            fls::FlsMessage::GsMessage { meta, .. } => meta,
            _ => continue,
        };
        let mut cursor = std::io::Cursor::new(&meta);
        let inner = framing::read_message(&mut cursor).unwrap();
        if inner.opcode == opcodes::GAME_RESULTS {
            results.push(decode_game_results(&inner.payload).unwrap());
        }
    }

    assert_eq!(results.len(), 3, "Bo3 should have 3 GAME_RESULTS");
    // Game 1: seat 3 wins
    assert_eq!(results[0].winner_seat, Some(3));
    // Game 2: seat 2 wins
    assert_eq!(results[1].winner_seat, Some(2));
    // Game 3: seat 3 wins
    assert_eq!(results[2].winner_seat, Some(3));
}
```

- [ ] **Step 7: Run and verify**

Run: `~/.cargo/bin/cargo test game_results -- -v 2>&1 | tail -20`
Expected: 4 tests PASS

- [ ] **Step 8: Commit**

```bash
git add src/protocol/game_results.rs src/protocol/game_messages.rs src/protocol/mod.rs
git commit -m "feat: decode GAME_RESULTS message (opcode 4485) for winner detection"
```

---

## Task 2: Update Rust schema for per-game structure

**Files:**
- Modify: `src/replay/schema.rs`

- [ ] **Step 1: Write tests for the new schema types**

Add to the test module in `src/replay/schema.rs`:

```rust
#[test]
fn test_game_replay_serialization_roundtrip() {
    let game = GameReplay {
        game_number: 1,
        header: GameHeader {
            game_id: "12345".to_string(),
            players: vec![
                PlayerInfo { player_id: "player_0".into(), name: "Alice".into(), life_total: 20 },
                PlayerInfo { player_id: "player_1".into(), name: "Bob".into(), life_total: 0 },
            ],
            result: GameResult::Win { winner_id: "player_0".to_string() },
        },
        actions: vec![],
        card_names: HashMap::new(),
        card_textures: HashMap::new(),
    };

    let replay = ReplayFile {
        header: ReplayHeader {
            format: "standard".to_string(),
            start_time: Utc::now(),
            end_time: Some(Utc::now()),
            players: vec![
                PlayerInfo { player_id: "player_0".into(), name: "Alice".into(), life_total: 20 },
                PlayerInfo { player_id: "player_1".into(), name: "Bob".into(), life_total: 20 },
            ],
        },
        games: vec![game],
        metadata: HashMap::new(),
    };

    let json = serde_json::to_string(&replay).unwrap();
    let loaded: ReplayFile = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.games.len(), 1);
    assert_eq!(loaded.games[0].game_number, 1);
    assert_eq!(loaded.games[0].header.game_id, "12345");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `~/.cargo/bin/cargo test game_replay_serialization -- -v 2>&1 | tail -20`
Expected: FAIL — `GameReplay` and `GameHeader` don't exist yet

- [ ] **Step 3: Implement schema changes**

In `src/replay/schema.rs`:

Add new structs:
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameHeader {
    pub game_id: String,
    pub players: Vec<PlayerInfo>,
    pub result: GameResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameReplay {
    pub game_number: u32,
    pub header: GameHeader,
    pub actions: Vec<ReplayAction>,
    pub card_names: HashMap<String, String>,
    pub card_textures: HashMap<String, i32>,
}
```

Update `ReplayHeader` — remove `game_id` and `result`:
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayHeader {
    pub players: Vec<PlayerInfo>,
    pub format: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
}
```

Update `ReplayFile` — replace flat fields with `games`:
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayFile {
    pub header: ReplayHeader,
    pub games: Vec<GameReplay>,
    pub metadata: HashMap<String, String>,
}
```

- [ ] **Step 4: Fix compilation errors in existing code**

The compiler will flag every site that uses `ReplayFile.actions`, `ReplayFile.card_names`, `ReplayFile.card_textures`, `ReplayHeader.game_id`, and `ReplayHeader.result`. Fix these:

- `src/replay/schema.rs`: Update `create_test_replay()` to use new structure. Update existing tests.
- `src/bin/decode.rs`: Will be fixed in Task 4, but needs to compile. Temporarily construct a single-game `ReplayFile` with one `GameReplay` entry so the binary compiles.

- [ ] **Step 5: Run tests**

Run: `~/.cargo/bin/cargo test -p flashback -- -v 2>&1 | tail -30`
Expected: All schema tests PASS. Golden pipeline tests may fail (expected — fixed in Task 4).

- [ ] **Step 6: Commit**

```bash
git add src/replay/schema.rs src/bin/decode.rs
git commit -m "feat: add GameReplay and GameHeader schema types, restructure ReplayFile"
```

---

## Task 3: Extract single-game golden fixture

**Files:**
- Modify: `tests/golden_pipeline.rs`
- Generate: `tests/fixtures/golden_game3.bin`

- [ ] **Step 1: Write the fixture extraction helper**

Add to `tests/golden_pipeline.rs`:

```rust
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
```

- [ ] **Step 2: Generate the fixture**

Run: `~/.cargo/bin/cargo test generate_golden_game3_fixture -- --ignored --nocapture 2>&1 | tail -10`
Expected: Creates `tests/fixtures/golden_game3.bin`

- [ ] **Step 3: Verify the fixture is parseable**

Add a smoke test:

```rust
#[test]
fn test_golden_game3_fixture_parses() {
    let data = std::fs::read("tests/fixtures/golden_game3.bin").unwrap();
    let messages = flashback::protocol::framing::parse_messages(&data).unwrap();
    assert!(messages.len() > 100, "game3 fixture should have many messages");
    eprintln!("golden_game3.bin: {} messages", messages.len());
}
```

- [ ] **Step 4: Run and verify**

Run: `~/.cargo/bin/cargo test golden_game3_fixture_parses -- -v --nocapture 2>&1 | tail -10`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add tests/golden_pipeline.rs tests/fixtures/golden_game3.bin
git commit -m "feat: extract single-game golden_game3.bin fixture from Bo3 capture"
```

---

## Task 4a: Rewrite `run_pipeline` for per-game splitting

**Files:**
- Modify: `tests/golden_pipeline.rs`

- [ ] **Step 1: Write multi-game splitting test**

Add to `tests/golden_pipeline.rs`:

```rust
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
```

- [ ] **Step 2: Write game result test**

```rust
#[test]
fn test_golden_v1_game_results() {
    let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
    let messages = flashback::protocol::framing::parse_messages(&data).unwrap();
    let games = run_pipeline(messages);
    assert_eq!(games.len(), 3);
    // Seat 3 wins game 1, seat 2 wins game 2, seat 3 wins game 3
    // With seat_id - 2 mapping: seat 2 → player_0, seat 3 → player_1
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
```

- [ ] **Step 3: Rewrite `run_pipeline`**

Change signature from `fn run_pipeline(data: &[u8]) -> Vec<ReplayAction>` to `fn run_pipeline(messages: Vec<RawMessage>) -> Vec<GameReplay>`.

Key implementation details for the per-game accumulator pattern:

```rust
use std::collections::{HashMap, HashSet};
use flashback::replay::schema::{GameReplay, GameHeader, GameResult, PlayerInfo};

fn run_pipeline(messages: Vec<RawMessage>) -> Vec<GameReplay> {
    // ... existing setup (statebuf_proc, game_state, translator) ...

    // Per-game accumulators
    let mut games: Vec<GameReplay> = Vec::new();
    let mut current_actions: Vec<ReplayAction> = Vec::new();
    let mut current_card_names: HashMap<String, String> = HashMap::new();
    let mut current_card_textures: HashMap<String, i32> = HashMap::new();
    let mut current_game_id: String = "unknown".to_string();
    let mut current_winner_seat: Option<u8> = None;
    let mut populated_players: HashSet<usize> = HashSet::new();

    // ... message loop ...

    // When processing PlayerStatus elements (inside the state application):
    // After game_state.apply_elements(), update populated_players:
    if let Some(ref state) = game_state {
        for i in 0..state.players.len() {
            populated_players.insert(i);
        }
    }

    // On GameMessage::GameResults(msg):
    current_winner_seat = msg.winner_seat;

    // On GameMessage::GameOver:
    // Snapshot players using populated_players (NOT the old life > 0 filter):
    let players: Vec<PlayerInfo> = if let Some(ref state) = game_state {
        state.players.iter().enumerate()
            .filter(|(i, _)| populated_players.contains(i))
            .map(|(i, p)| PlayerInfo {
                player_id: format!("player_{}", i),
                name: format!("player_{}", i),
                life_total: p.life,
            })
            .collect()
    } else { vec![] };

    // Resolve winner_seat → player_id using seat_id - 2 mapping:
    let result = match current_winner_seat {
        Some(seat) if seat >= 2 && (seat as usize - 2) < players.len() => {
            GameResult::Win { winner_id: format!("player_{}", seat - 2) }
        }
        Some(_) => GameResult::Incomplete, // bad seat mapping
        None => GameResult::Incomplete,     // no GameResults received
    };

    // Package GameReplay:
    games.push(GameReplay {
        game_number: (games.len() + 1) as u32,
        header: GameHeader {
            game_id: current_game_id.clone(),
            players,
            result,
        },
        actions: std::mem::take(&mut current_actions),
        card_names: std::mem::take(&mut current_card_names),
        card_textures: std::mem::take(&mut current_card_textures),
    });
    // Reset all accumulators:
    current_game_id = "unknown".to_string();
    current_winner_seat = None;
    populated_players.clear();

    // On GameStatusChange (5 or 7):
    // If current_actions is non-empty, package as incomplete and warn:
    if !current_actions.is_empty() {
        eprintln!("WARN: GameStatusChange with non-empty actions — no GameOver received");
        // ... package with GameResult::Incomplete, same as above but no winner ...
    }
    // Reset accumulators + translator/statebuf/state

    // After loop: if current_actions non-empty, package as incomplete
    // ... same pattern ...

    games
}
```

Note: `test_commutativity_check` and `test_golden_checksum_audit` manage their own pipeline loops and do NOT call `run_pipeline`, so they are unaffected by this signature change.

- [ ] **Step 4: Run new tests**

Run: `~/.cargo/bin/cargo test golden_v1_produces_3 golden_v1_game_results -- -v 2>&1 | tail -20`
Expected: Both PASS

- [ ] **Step 5: Commit**

```bash
git add tests/golden_pipeline.rs
git commit -m "feat: rewrite run_pipeline for per-game GameReplay splitting"
```

---

## Task 4b: Update existing golden pipeline tests

**Files:**
- Modify: `tests/golden_pipeline.rs`

- [ ] **Step 1: Update tests to new `run_pipeline` signature**

All existing tests that called `run_pipeline(&data)` now need `run_pipeline(messages)`:

```rust
// For single-game tests, switch to golden_game3.bin:
let data = std::fs::read("tests/fixtures/golden_game3.bin").unwrap();
let messages = flashback::protocol::framing::parse_messages(&data).unwrap();
let games = run_pipeline(messages);
assert_eq!(games.len(), 1);
let actions = &games[0].actions;
```

Tests to update:
- `test_golden_file_produces_actions` → switch to `golden_game3.bin`, assert `games.len() == 1`, check `actions.len() > 10`
- `test_golden_file_has_expected_action_types` → switch to `golden_game3.bin`, iterate `games[0].actions`
- `test_golden_file_actions_have_valid_turns` → switch to `golden_game3.bin`, iterate `games[0].actions`
- `test_golden_pipeline_has_zone_transitions` → switch to `golden_game3.bin`, iterate `games[0].actions`
- `test_golden_snapshot_regression` → handled in Task 5

- [ ] **Step 2: Run all golden tests**

Run: `~/.cargo/bin/cargo test golden -- -v 2>&1 | tail -30`
Expected: All PASS except possibly `test_golden_snapshot_regression` (fixture update needed in Task 5)

- [ ] **Step 3: Commit**

```bash
git add tests/golden_pipeline.rs
git commit -m "fix: update golden pipeline tests for per-game run_pipeline signature"
```

---

## Task 4c: Rewrite `decode_pipeline` in `src/bin/decode.rs`

**Files:**
- Modify: `src/bin/decode.rs`

- [ ] **Step 1: Rewrite `decode_pipeline`**

Mirror the per-game accumulator pattern from `run_pipeline` (Task 4a). Key changes:

- Import `GameReplay`, `GameHeader` from schema, `HashSet` from std::collections
- Add per-game accumulators (same as run_pipeline): `current_actions`, `current_card_names`, `current_card_textures`, `current_game_id`, `current_winner_seat`, `populated_players: HashSet<usize>`
- Move card_names/card_textures collection into per-game accumulators (currently match-level)
- Track `populated_players`: after `game_state.apply_elements()`, insert all player indices present in `state.players`
- Handle `GameMessage::GameResults(msg)`: store `current_winner_seat = msg.winner_seat`
- Handle `GameMessage::GameOver`: snapshot players (filter by `populated_players`), resolve winner, package `GameReplay`, reset
- Handle `GameStatusChange`: defensive packaging if `current_actions` non-empty (with `tracing::warn!`)
- After loop: package remaining actions as incomplete
- Return `ReplayFile { header: ReplayHeader { ... }, games, metadata }`

- [ ] **Step 2: Update `main()` output stats**

Change:
```rust
eprintln!(
    "Replay: {} games, {} total actions",
    replay.games.len(),
    replay.games.iter().map(|g| g.actions.len()).sum::<usize>()
);
```

- [ ] **Step 3: Run full test suite**

Run: `~/.cargo/bin/cargo test -- -v 2>&1 | tail -40`
Expected: All tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/bin/decode.rs
git commit -m "feat: rewrite decode_pipeline for per-game splitting with result detection"
```

---

## Task 5: Update golden regression fixtures

**Files:**
- Modify: `tests/golden_pipeline.rs`
- Generate: `tests/fixtures/golden_game3_replay.json`
- Generate: `tests/fixtures/golden_v1_replay.json` (updated schema)

- [ ] **Step 1: Write fixture generation helpers**

Add to `tests/golden_pipeline.rs`:

```rust
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
```

- [ ] **Step 2: Generate fixtures**

Run:
```bash
~/.cargo/bin/cargo test generate_golden_game3_replay_json -- --ignored --nocapture 2>&1 | tail -5
~/.cargo/bin/cargo test generate_golden_v1_replay_json -- --ignored --nocapture 2>&1 | tail -5
```

- [ ] **Step 3: Update snapshot regression test**

Update `test_golden_snapshot_regression` to use `golden_game3.bin` and `golden_game3_replay.json`:

```rust
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
    let expected: Vec<ActionSnapshot> = serde_json::from_str(&expected_json).unwrap();

    assert_eq!(actual.len(), expected.len(), "action count mismatch");
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(a, e, "action {} mismatch", i);
    }
}
```

- [ ] **Step 4: Run all golden tests**

Run: `~/.cargo/bin/cargo test golden -- -v 2>&1 | tail -30`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add tests/golden_pipeline.rs tests/fixtures/golden_game3_replay.json tests/fixtures/golden_v1_replay.json
git commit -m "feat: update golden regression fixtures for per-game schema"
```

---

## Task 6: Update TypeScript types

**Files:**
- Modify: `web/src/types/replay.ts`

- [ ] **Step 1: Update the TypeScript interfaces**

Add `GameHeader` and `GameReplay`, update `ReplayFile`:

```typescript
export interface GameHeader {
  game_id: string;
  players: PlayerInfo[];
  result: RawGameResult;
}

export interface GameReplay {
  game_number: number;
  header: GameHeader;
  actions: RawReplayAction[];
  card_names?: Record<string, string>;
  card_textures?: Record<string, number>;
}

// Update ReplayFile — remove actions/card_names/card_textures, add games:
export interface ReplayFile {
  header: ReplayHeader;
  games: GameReplay[];
  metadata: Record<string, string>;
}

// Update ReplayHeader — remove game_id and result:
export interface ReplayHeader {
  format: string;
  start_time: string;
  end_time?: string | null;
  players: PlayerInfo[];
}
```

- [ ] **Step 2: Fix compilation errors**

Update helper functions that reference `ReplayFile.actions` or `ReplayHeader.game_id`. The `getWinnerId` and `getResultLabel` helpers still work since they take `RawGameResult` directly.

- [ ] **Step 3: Run type check**

Run: `cd web && npx tsc --noEmit 2>&1 | head -30`
Expected: Type errors in `reconstructor.ts` and `App.tsx` (fixed in next tasks). Types file itself should be clean.

- [ ] **Step 4: Commit**

```bash
git add web/src/types/replay.ts
git commit -m "feat: update TypeScript types for per-game replay schema"
```

---

## Task 7: Update Reconstructor for per-game loading

**Files:**
- Modify: `web/src/engine/reconstructor.ts`

- [ ] **Step 1: Update `loadReplay` signature**

Change `loadReplay(replay: ReplayFile)` to `loadReplay(replay: ReplayFile, gameIndex: number = 0)`:

```typescript
loadReplay(replay: ReplayFile, gameIndex: number = 0) {
  const game = replay.games[gameIndex];
  if (!game) {
    throw new Error(`Game index ${gameIndex} out of range (${replay.games.length} games)`);
  }
  this.actions = game.actions;
  this.cardNames = game.card_names ? { ...game.card_names } : {};
}
```

Note: `cardNames` stays as `Record<string, string>` (plain object) — do NOT convert to `Map`, as the rest of the class uses object-style access (`this.cardNames[cardId]`).

Update `resolveCardTextures` to accept card textures from the game:

```typescript
async resolveCardTextures(cardTextures: Record<string, number>) {
  // existing implementation unchanged, but called with game-level textures
}
```

- [ ] **Step 2: Fix any remaining references to top-level card_names/card_textures**

Ensure `resolveCardTextures` is called by the consumer (App.tsx) with `replay.games[gameIndex].card_textures` rather than `replay.card_textures`.

- [ ] **Step 3: Run type check**

Run: `cd web && npx tsc --noEmit 2>&1 | head -20`
Expected: Errors only in `App.tsx` (fixed in Task 8)

- [ ] **Step 4: Commit**

```bash
git add web/src/engine/reconstructor.ts
git commit -m "feat: update Reconstructor.loadReplay for per-game indexing"
```

---

## Task 8: Update viewer UI with game selector

**Files:**
- Modify: `web/src/App.tsx`

- [ ] **Step 1: Add game index state and selector**

Add state:
```typescript
const [gameIndex, setGameIndex] = useState(0);
```

Update the file load handler to pass `gameIndex` to `loadReplay` and scope card textures per-game. Add a game switch handler that reloads the reconstructor with the new game index and resets the step counter.

Note: The current `App.tsx` uses variable names like `setReplayFile`, `setCurrentStep`, etc. — read the actual file and adapt to the existing naming conventions rather than copying this pseudocode literally:

```typescript
// Pseudocode — adapt to actual App.tsx variable names:
// In file load handler:
reconstructor.loadReplay(replay, 0);
resolveCardTextures(replay.games[0].card_textures);

// New game switch handler:
const handleGameSwitch = (newIndex: number) => {
  reconstructor.loadReplay(replay, newIndex);
  resolveCardTextures(replay.games[newIndex].card_textures);
  resetStepToZero();
};
```

- [ ] **Step 2: Add game selector UI**

Render a game selector when `games.length > 1`:

```tsx
{replay && replay.games.length > 1 && (
  <div className="game-selector">
    {replay.games.map((game, i) => (
      <button
        key={i}
        className={i === gameIndex ? 'active' : ''}
        onClick={() => handleGameSwitch(i)}
      >
        Game {game.game_number}
      </button>
    ))}
  </div>
)}
```

- [ ] **Step 3: Update game info display**

Change references from `replay.header.game_id` to `replay.games[gameIndex].header.game_id`, and result from `replay.header.result` to `replay.games[gameIndex].header.result`.

- [ ] **Step 4: Update action count and other per-game references**

Any reference to `replay.actions` should become `replay.games[gameIndex].actions`. The reconstructor's `getActionCount()` handles this internally once `loadReplay` is called with the right game index.

- [ ] **Step 5: Run type check and dev server**

Run: `cd web && npx tsc --noEmit 2>&1 | head -20`
Expected: No errors

Run: `cd web && npx vite build 2>&1 | tail -10`
Expected: Build succeeds

- [ ] **Step 6: Commit**

```bash
git add web/src/App.tsx
git commit -m "feat: add game selector UI for multi-game replays"
```

---

## Task 9: Fix existing web component tests

**Files:**
- Modify: `web/src/engine/reconstructor.test.ts` — mock `ReplayFile` uses old flat schema
- Modify: `web/src/components/FileLoader.test.tsx` — mock `ReplayFile` uses old flat schema
- Modify: `web/src/components/Board.test.tsx` — may reference `ReplayFile` fields
- Modify: `web/src/components/GameLog.test.tsx` — may reference action data
- Modify: `web/src/App.test.tsx` — mock `ReplayFile` with `header.game_id`, `actions`, etc.

- [ ] **Step 1: Update test fixtures**

Every test that constructs a `ReplayFile` needs restructuring. The pattern is:

Old: `{ header: { game_id, result, ... }, actions: [...], card_names: {...}, card_textures: {...} }`
New: `{ header: { ... }, games: [{ game_number: 1, header: { game_id, result, ... }, actions: [...], card_names: {...}, card_textures: {...} }] }`

Specifically:
- Move `actions`, `card_names`, `card_textures` from top level into `games[0]`
- Move `game_id` and `result` from `ReplayHeader` into `games[0].header` (as `GameHeader`)
- Add `game_number: 1` to the game entry
- Remove `game_id` and `result` from the top-level `header`

- [ ] **Step 2: Run full web test suite**

Run: `cd web && npx vitest run 2>&1 | tail -30`
Expected: All tests PASS

- [ ] **Step 3: Commit**

```bash
git add web/src/
git commit -m "fix: update web component tests for per-game replay schema"
```

---

## Task 10: Metadata version bump and final verification

**Files:**
- Modify: `src/bin/decode.rs` (metadata version)

- [ ] **Step 1: Bump version to 2.0**

In `decode_pipeline`, change:
```rust
metadata.insert("version".to_string(), "2.0".to_string());
```

- [ ] **Step 2: Run full Rust test suite**

Run: `~/.cargo/bin/cargo test 2>&1 | tail -20`
Expected: All PASS

- [ ] **Step 3: Run full web test suite**

Run: `cd web && npx vitest run 2>&1 | tail -20`
Expected: All PASS

- [ ] **Step 4: End-to-end smoke test**

If a `.flashback` file exists, decode it and verify the JSON has the new structure:

```bash
~/.cargo/bin/cargo run --bin decode -- tests/fixtures/golden_v1.bin 2>/dev/null | python3 -c "import json,sys; d=json.load(sys.stdin); print(f'Games: {len(d[\"games\"])}, Version: {d[\"metadata\"][\"version\"]}')"
```

Expected: `Games: 3, Version: 2.0`

- [ ] **Step 5: Commit**

```bash
git add src/bin/decode.rs
git commit -m "chore: bump replay metadata version to 2.0"
```
