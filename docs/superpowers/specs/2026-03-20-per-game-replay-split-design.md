# Per-Game Replay Split

Split the replay pipeline output from one flat action list into separate games within a single `.flashback` file. Create a single-game golden test fixture from game 3 of the existing capture.

## Context

The current pipeline concatenates all games in a Bo3 match into one `ReplayFile` with a flat `actions` vec. Turn numbers reset at game boundaries, card IDs change, but the viewer has no way to distinguish games. The golden test fixture (`golden_v1.bin`) contains a 3-game Bo3 match captured mid-stream.

## Schema

### Before

```rust
ReplayFile {
    header: ReplayHeader,          // single header
    actions: Vec<ReplayAction>,    // all games concatenated
    metadata: HashMap,
    card_names: HashMap,
    card_textures: HashMap,
}
```

### After

```rust
ReplayFile {
    header: ReplayHeader,          // match-level
    games: Vec<GameReplay>,        // one per game
    metadata: HashMap,
}

GameReplay {
    game_number: u32,              // 1-indexed
    header: GameHeader,            // per-game: game_id, players, result
    actions: Vec<ReplayAction>,
    card_names: HashMap<String, String>,
    card_textures: HashMap<String, i32>,
}

GameHeader {
    game_id: String,
    players: Vec<PlayerInfo>,      // life totals at game end
    result: GameResult,
}
```

`ReplayHeader` (match-level) keeps: `format`, `start_time`, `end_time`, `players`. Remove `game_id` and `result` from `ReplayHeader` — both move to `GameHeader`. Reuse `PlayerInfo` at the match level with `life_total` set to the starting life (20 for constructed).

The `GameOver` player snapshot currently filters out players with `life <= 0 && library_count <= 0`, which would exclude the losing player. Fix: track which player slots have been populated via a `HashSet<usize>` (`populated_players`) maintained alongside the game state. When `apply_elements` processes a `PlayerStatus` element, add each player index to the set. On `GameOver`, include player `i` if `populated_players.contains(&i)`. Reset the set alongside the other accumulators at game boundaries.

This avoids false negatives from the field-value heuristic (e.g., a decked player with 0 life, 0 hand, 0 library, and 0 graveyard because all cards were exiled).

Bump metadata version to `"2.0"` to distinguish the new schema.

This is a breaking schema change for both the Rust types and the TypeScript viewer types.

## Decode pipeline

`decode_pipeline` in `src/bin/decode.rs` changes from accumulating one `Vec<ReplayAction>` to building a `Vec<GameReplay>`:

1. Maintain per-game accumulators: `current_actions`, `current_card_names`, `current_card_textures`, `current_game_id`.
2. On `GameOver`: snapshot player info from `game_state`, determine `GameResult` (see below), package into `GameReplay` with `game_number = games.len() + 1`, push to `games` vec, reset accumulators and translator/statebuf state.
3. On `GameStatusChange` (status 5 or 7): if `current_actions` is non-empty, package as an incomplete game with `GameResult::Incomplete` and log a warning (this means no `GameOver` was received for this game — possible on disconnect/timeout). Then reset accumulators and state. If `current_actions` is empty, this is a no-op (the preceding `GameOver` already drained the accumulator). Verified in golden_v1.bin: `GameOver` always precedes `GameStatusChange` for normal game endings, but the defensive check handles abnormal cases.
4. After the message loop: if `current_actions` is non-empty, package as an incomplete game with `GameResult::Incomplete` (no `GameOver` was received — capture ended mid-game or stream was truncated).
5. Build `ReplayFile` with the `games` vec.

### Game result detection

The protocol sends game results explicitly via the `GAME_RESULTS` message (opcode 4485, already defined in `opcodes.rs` but currently falling through to `GameMessage::Other`). Analysis of the golden fixture confirms the payload structure:

```
Offset  Type    Field
0..4    u32     game_id
4..8    u32     winner_index    (0-indexed into player entries below)
8       u8      unknown         (always 0 in samples)
9..11   u16     player_count
11..13  u16     unknown         (always 0 in samples)
13..    [player_count × 5-byte entries]:
          u8    seat_id         (MTGO internal player seat, e.g. 2 or 3)
          u16   rating          (player's rating/ELO)
          u16   padding         (always 0)
```

Evidence from golden_v1.bin (Bo3, 3 games):
- Game 1: winner_index=1, entries=[seat 2, seat 3] → seat 3 wins
- Game 2: winner_index=1, entries=[seat 3, seat 2] → seat 2 wins
- Game 3: winner_index=0, entries=[seat 3, seat 2] → seat 3 wins

Implementation:

1. Add `GameResults { game_id: u32, winner_seat: u8, players: Vec<(u8, u16)> }` variant to `GameMessage`.
2. Decode in `decode_game_message`: parse the payload per the layout above, resolve `winner_seat = entries[winner_index].seat_id`.
3. In the decode pipeline, on `GameMessage::GameResults`: store the `winner_seat` in the per-game accumulator. The seat IDs (2, 3) need mapping to our `player_0`/`player_1` naming — this mapping must be established from the `PlayerOrder` message or from the order players appear in `GameState.players`. If no mapping is available, fall back to `GameResult::Incomplete`.
4. On `GameOver`: use the stored `winner_seat` to produce `GameResult::Win { winner_id }`. If no `GameResults` preceded the `GameOver`, fall back to `GameResult::Incomplete`.

This handles all win conditions (life loss, decking, poison, Thassa's Oracle, concession, timeout) because the server resolves the winner — we just read the answer.

**Draws:** The golden data does not contain a draw, so the encoding is unknown. If `winner_index >= player_count` or the payload is shorter than expected, treat it as `GameResult::Draw` (most likely sentinel). If a `GAME_RESULTS` message cannot be parsed at all, fall back to `GameResult::Incomplete`.

**Open question:** The seat-to-player mapping. The `PlayerOrder` message is currently a raw passthrough (`FlsMessage::PlayerOrder { raw: Vec<u8> }`). Its payload likely contains the seat assignment. If decoding it proves difficult, an alternative is to assume `seat_id - 2 == player_index` (seats appear to be 2-indexed in the golden data, mapping to player_0 and player_1). This assumption should include a bounds check: if `seat_id < 2` or `seat_id - 2 >= player_count`, fall back to `GameResult::Incomplete`. Validate against additional captures before relying on it.

The `run_pipeline` helper in `tests/golden_pipeline.rs` follows the same pattern. It currently returns `Vec<ReplayAction>` — change to return `Vec<GameReplay>` (or the full `ReplayFile`).

## Golden fixture changes

### Single-game fixture (`golden_game3.bin`)

Write a `#[test] #[ignore]` helper that:

1. Parses `golden_v1.bin` into raw messages.
2. Walks the FLS stream counting `GameOver` messages.
3. After the 2nd `GameOver`, records all subsequent raw messages as game 3's data. Includes any `GameStatusChange` messages that precede the actual game data (they're part of the inter-game boundary protocol).
4. Re-serializes those messages into `tests/fixtures/golden_game3.bin` using the framing format (length-prefixed messages).

This gives us a ~4MB single-game fixture that tests the full pipeline end-to-end.

### Regression fixtures

- `golden_game3_replay.json`: expected actions for the single-game fixture.
- `golden_v1_replay.json`: updated to the new multi-game schema (array of `GameReplay` objects).
- Both generated by `#[ignore]` test helpers, same as today.

### Test updates

- Most tests switch to `golden_game3.bin` for single-game validation (action types, turn validity, zone transitions).
- Multi-game tests use `golden_v1.bin` to verify game boundary detection and correct splitting.
- Add a test that asserts `golden_v1.bin` produces exactly 3 games.

## Web viewer changes

### Types (`web/src/types/replay.ts`)

- Add `GameReplay` interface with `game_number`, `header: GameHeader`, `actions`, `card_names`, `card_textures`.
- Add `GameHeader` interface.
- `ReplayFile.actions`, `card_names`, `card_textures` removed from top level; all live on `GameReplay` now. No backward-compat optionals (non-goal).

### Reconstructor (`web/src/engine/reconstructor.ts`)

- `loadReplay(replay, gameIndex = 0)`: loads a specific game from the replay file.
- Card names/textures come from `replay.games[gameIndex]`. `resolveCardTextures` is called internally by `loadReplay` using the selected game's textures rather than requiring the caller to extract them.
- No changes to the reconstruction logic itself — it still replays actions sequentially.

### Viewer UI

- When `games.length > 1`, show a game selector (tabs or dropdown).
- Selecting a game calls `reconstructor.loadReplay(replay, gameIndex)` and resets the step counter.
- When `games.length === 1`, no selector shown — behaves exactly as today.

## Non-goals

- Match-level result tracking (which player won the match) — future work.
- Sideboard tracking between games — future work.
- Backward-compatible loading of old single-game `.flashback` files — not worth the complexity since we have no published replays yet.
