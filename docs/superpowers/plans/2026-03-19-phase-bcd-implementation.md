# Phase B/C/D Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the full MTGO protocol decode pipeline from raw framed messages to `ReplayFile` JSON output, validated against `golden_v1.bin`.

**Architecture:** FLS envelope decoder extracts game messages from the wire protocol. `GamePlayStatusMessage` payloads go through StateBuf assembly/diff processing to produce state elements. A `GameState` model is updated from elements, and a `ReplayTranslator` diffs consecutive states to emit `ReplayAction` records. The replay file stores the action stream.

**Tech Stack:** Rust, `thiserror` for errors, `tracing` for logging, `chrono` for timestamps, `serde`/`serde_json` for serialization. All dependencies already in `Cargo.toml`.

**Spec:** `docs/superpowers/specs/2026-03-19-phase-bcd-updated-design.md`
**Original design:** `docs/specs/2026-03-18-rust-decoder-design.md`
**Protocol reference:** `PROTOCOL_RESEARCH.md`
**Golden file:** `tests/fixtures/golden_v1.bin` (12MB, 10,195 messages)

---

## File Structure

| Action | File | Responsibility |
|--------|------|---------------|
| Delete | `src/protocol/decoder.rs` | Stub, replaced by new pipeline |
| Delete | `src/protocol/raw_analyzer.rs` | Stub, unused |
| Delete | `src/replay/writer.rs` | Flat event model, replaced by translator |
| Delete | `src/replay/boundary.rs` | Replaced by multi-game handling in translator |
| Delete | `tests/integration.rs` | Tests placeholder behavior |
| Modify | `src/protocol/mod.rs` | Remove old exports, add new modules |
| Modify | `src/replay/mod.rs` | Remove old exports |
| Modify | `src/lib.rs` | Add `state` and `translator` modules |
| Modify | `src/protocol/opcodes.rs` | Add boundary signal opcode constants |
| Modify | `src/replay/schema.rs` | Add new ActionType variants |
| Modify | `src/bin/decode.rs` | Use full pipeline |
| Create | `src/protocol/fls.rs` | FLS envelope decoding |
| Create | `src/protocol/game_messages.rs` | Inner game message decoding |
| Create | `src/protocol/statebuf.rs` | StateBuf assembly, diffs, element parsing |
| Create | `src/state.rs` | GameState board model |
| Create | `src/translator.rs` | State-diff to ReplayAction translation |

---

## Task 0: Clean up old code and fix module structure

**Files:**
- Delete: `src/protocol/decoder.rs`
- Delete: `src/protocol/raw_analyzer.rs`
- Delete: `src/replay/writer.rs`
- Delete: `src/replay/boundary.rs`
- Delete: `tests/integration.rs`
- Modify: `src/protocol/mod.rs`
- Modify: `src/replay/mod.rs`
- Modify: `src/lib.rs`
- Modify: `src/protocol/opcodes.rs`

- [ ] **Step 1: Delete old stub files**

```bash
rm src/protocol/decoder.rs src/protocol/raw_analyzer.rs src/replay/writer.rs src/replay/boundary.rs tests/integration.rs
```

- [ ] **Step 2: Update `src/protocol/mod.rs`**

Replace the entire file with:

```rust
// src/protocol/mod.rs

pub mod framing;
pub mod opcodes;
pub mod fls;
pub mod game_messages;
pub mod statebuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("unexpected EOF: {context}")]
    UnexpectedEof { context: &'static str },

    #[error("invalid checksum: expected {expected}, got {got}")]
    InvalidChecksum { expected: i32, got: i32 },

    #[error("diff out of bounds: {context}")]
    DiffOutOfBounds { context: &'static str },

    #[error("diff size mismatch: expected {expected}, got {got}")]
    DiffSizeMismatch { expected: u32, got: u32 },
}

impl DecodeError {
    pub fn is_eof(&self) -> bool {
        match self {
            DecodeError::UnexpectedEof { .. } => true,
            DecodeError::Io(e) => e.kind() == std::io::ErrorKind::UnexpectedEof,
            _ => false,
        }
    }
}
```

- [ ] **Step 3: Update `src/replay/mod.rs`**

Replace the entire file with:

```rust
pub mod schema;

pub use schema::{
    ReplayFile, ReplayHeader, PlayerInfo, GameResult, ReplayAction, ActionType,
    create_test_replay, write_replay_file, load_replay_file, ReplayError,
    Result,
};
```

- [ ] **Step 4: Update `src/lib.rs`**

Replace with:

```rust
pub mod capture;
pub mod protocol;
pub mod replay;
pub mod state;
pub mod translator;
```

- [ ] **Step 5: Add boundary signal opcode constants to `src/protocol/opcodes.rs`**

Add after the existing FLS-level opcodes section (after `pub const GS_REPLAY_MESSAGE: u16 = 1156;`):

```rust
// === Boundary signal opcodes ===

pub const FLS_GAME_CREATED: u16 = 960;
pub const FLS_GAME_ENDED: u16 = 967;
pub const FLS_MATCH_GAME_STARTED: u16 = 2463;
```

- [ ] **Step 6: Create placeholder files so the project compiles**

Create `src/protocol/fls.rs`:
```rust
//! FLS envelope decoding.
```

Create `src/protocol/game_messages.rs`:
```rust
//! Inner game message decoding.
```

Create `src/protocol/statebuf.rs`:
```rust
//! StateBuf assembly, diff processing, and element parsing.
```

Create `src/state.rs`:
```rust
//! Game state board model.
```

Create `src/translator.rs`:
```rust
//! State-diff to ReplayAction translation.
```

- [ ] **Step 7: Verify the project compiles**

Run: `cargo check 2>&1`
Expected: compiles with no errors (warnings are OK)

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor: remove old stubs, set up module structure for decode pipeline"
```

---

## Task 1: FLS envelope decoding (`fls.rs`)

**Files:**
- Create: `src/protocol/fls.rs`

**Reference:** `PROTOCOL_RESEARCH.md` — GsMessageMessage wire format. Opcodes 1153/1156 share the same layout: 16-byte GUID (MatchToken), i32 MatchID, i32 GameID, then i32 count + N bytes MetaMessage. Opcode 1145 (GshGameStatusChangeMessage) has a single i32 NewStatus field.

- [ ] **Step 1: Write test for GsMessage decoding**

In `src/protocol/fls.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::framing::RawMessage;

    fn make_gs_message_payload(game_id: i32, meta: &[u8]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&[0u8; 16]); // MatchToken GUID
        payload.extend_from_slice(&0i32.to_le_bytes()); // MatchID
        payload.extend_from_slice(&game_id.to_le_bytes()); // GameID
        payload.extend_from_slice(&(meta.len() as i32).to_le_bytes()); // byte[] count
        payload.extend_from_slice(meta);
        payload
    }

    #[test]
    fn test_decode_gs_message_1153() {
        let meta = vec![0x01, 0x02, 0x03];
        let raw = RawMessage {
            opcode: 1153,
            type_check: 0,
            payload: make_gs_message_payload(42, &meta),
        };
        let result = decode_fls(raw).unwrap();
        match result {
            FlsMessage::GsMessage { game_id, meta: m } => {
                assert_eq!(game_id, 42);
                assert_eq!(m, vec![0x01, 0x02, 0x03]);
            }
            _ => panic!("expected GsMessage"),
        }
    }

    fn make_gs_replay_message_payload(game_id: i32, meta: &[u8]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&game_id.to_le_bytes()); // GameID
        payload.extend_from_slice(&0i32.to_le_bytes()); // HostGSHServerID
        payload.extend_from_slice(&(meta.len() as i32).to_le_bytes()); // byte[] count
        payload.extend_from_slice(meta);
        payload
    }

    #[test]
    fn test_decode_gs_replay_message_1156() {
        let meta = vec![0xAA];
        let raw = RawMessage {
            opcode: 1156,
            type_check: 0,
            payload: make_gs_replay_message_payload(99, &meta),
        };
        let result = decode_fls(raw).unwrap();
        match result {
            FlsMessage::GsMessage { game_id, .. } => assert_eq!(game_id, 99),
            _ => panic!("expected GsMessage for opcode 1156"),
        }
    }

    #[test]
    fn test_decode_game_status_change() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&7u32.to_le_bytes()); // GAME_COMPLETED
        let raw = RawMessage {
            opcode: 1145,
            type_check: 0,
            payload,
        };
        let result = decode_fls(raw).unwrap();
        match result {
            FlsMessage::GameStatusChange { new_status } => assert_eq!(new_status, 7),
            _ => panic!("expected GameStatusChange"),
        }
    }

    #[test]
    fn test_decode_player_order_is_passthrough() {
        let raw = RawMessage {
            opcode: 1155,
            type_check: 0,
            payload: vec![0x01, 0x02, 0x03],
        };
        let result = decode_fls(raw).unwrap();
        match result {
            FlsMessage::PlayerOrder { raw: r } => assert_eq!(r, vec![0x01, 0x02, 0x03]),
            _ => panic!("expected PlayerOrder"),
        }
    }

    #[test]
    fn test_decode_unknown_opcode() {
        let raw = RawMessage {
            opcode: 9999,
            type_check: 0,
            payload: vec![],
        };
        let result = decode_fls(raw).unwrap();
        assert!(matches!(result, FlsMessage::Other(_)));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib protocol::fls::tests -v 2>&1`
Expected: FAIL — `decode_fls` and `FlsMessage` not defined

- [ ] **Step 3: Implement `decode_fls`**

In `src/protocol/fls.rs`:

```rust
//! FLS envelope decoding.
//!
//! Decodes Level 1 (FLS) messages from the MTGO wire protocol.
//! Both opcode 1153 (GsMessageMessage) and 1156 (GsReplayMessageMessage)
//! map to FlsMessage::GsMessage.

use std::io::{Cursor, Read};

use super::DecodeError;
use super::framing::RawMessage;
use super::opcodes;

#[derive(Debug, Clone)]
pub enum FlsMessage {
    /// Game state message containing an embedded game-level CSMessage.
    /// `meta` is a complete embedded CSMessage with its own 8-byte header.
    GsMessage { game_id: i32, meta: Vec<u8> },
    /// Game lifecycle status change.
    GameStatusChange { new_status: u32 },
    /// Player seat order (wire layout undocumented — raw passthrough).
    PlayerOrder { raw: Vec<u8> },
    /// Game created boundary signal.
    GameCreated { game_id: i32 },
    /// Game ended boundary signal.
    GameEnded { game_id: i32 },
    /// Match started boundary signal.
    MatchStarted { game_id: i32 },
    /// Unrecognized opcode — passed through as-is.
    Other(RawMessage),
}

/// Decode a framed RawMessage into an FLS-level message.
pub fn decode_fls(msg: RawMessage) -> Result<FlsMessage, DecodeError> {
    match msg.opcode {
        opcodes::GS_MESSAGE => decode_gs_message(msg),
        opcodes::GS_REPLAY_MESSAGE => decode_gs_replay_message(msg),
        opcodes::GSH_GAME_STATUS_CHANGE => decode_game_status_change(msg),
        opcodes::GS_PLAYER_ORDER => Ok(FlsMessage::PlayerOrder { raw: msg.payload }),
        opcodes::FLS_GAME_CREATED => decode_boundary_game_id(msg, |gid| FlsMessage::GameCreated { game_id: gid }),
        opcodes::FLS_GAME_ENDED => decode_boundary_game_id(msg, |gid| FlsMessage::GameEnded { game_id: gid }),
        opcodes::FLS_MATCH_GAME_STARTED => decode_boundary_game_id(msg, |gid| FlsMessage::MatchStarted { game_id: gid }),
        _ => Ok(FlsMessage::Other(msg)),
    }
}

fn decode_gs_message(msg: RawMessage) -> Result<FlsMessage, DecodeError> {
    let mut cursor = Cursor::new(&msg.payload);

    // MatchToken: Guid (16 bytes) — consumed, not stored
    let mut guid = [0u8; 16];
    cursor.read_exact(&mut guid).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsMessage: MatchToken",
    })?;

    // MatchID: int32 — consumed, not stored
    let mut buf4 = [0u8; 4];
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsMessage: MatchID",
    })?;

    // GameID: int32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsMessage: GameID",
    })?;
    let game_id = i32::from_le_bytes(buf4);

    // MetaMessage: byte[] — int32 count + N bytes
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsMessage: MetaMessage length",
    })?;
    let meta_len = i32::from_le_bytes(buf4);
    if meta_len < 0 {
        return Err(DecodeError::UnexpectedEof {
            context: "GsMessage: negative MetaMessage length",
        });
    }
    let mut meta = vec![0u8; meta_len as usize];
    cursor.read_exact(&mut meta).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsMessage: MetaMessage bytes",
    })?;

    Ok(FlsMessage::GsMessage { game_id, meta })
}

/// Decode GsReplayMessageMessage (opcode 1156).
/// Wire layout differs from 1153: GameID(int), HostGSHServerID(int), MetaMessage(byte[]).
/// No GUID or MatchID.
fn decode_gs_replay_message(msg: RawMessage) -> Result<FlsMessage, DecodeError> {
    let mut cursor = Cursor::new(&msg.payload);
    let mut buf4 = [0u8; 4];

    // GameID: int32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsReplayMessage: GameID",
    })?;
    let game_id = i32::from_le_bytes(buf4);

    // HostGSHServerID: int32 — consumed, not stored
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsReplayMessage: HostGSHServerID",
    })?;

    // MetaMessage: byte[] — int32 count + N bytes
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsReplayMessage: MetaMessage length",
    })?;
    let meta_len = i32::from_le_bytes(buf4);
    if meta_len < 0 {
        return Err(DecodeError::UnexpectedEof {
            context: "GsReplayMessage: negative MetaMessage length",
        });
    }
    let mut meta = vec![0u8; meta_len as usize];
    cursor.read_exact(&mut meta).map_err(|_| DecodeError::UnexpectedEof {
        context: "GsReplayMessage: MetaMessage bytes",
    })?;

    Ok(FlsMessage::GsMessage { game_id, meta })
}

fn decode_game_status_change(msg: RawMessage) -> Result<FlsMessage, DecodeError> {
    if msg.payload.len() < 4 {
        return Err(DecodeError::UnexpectedEof {
            context: "GameStatusChange: NewStatus",
        });
    }
    let new_status = u32::from_le_bytes([
        msg.payload[0], msg.payload[1], msg.payload[2], msg.payload[3],
    ]);
    Ok(FlsMessage::GameStatusChange { new_status })
}

/// Attempt to read a game_id (i32) from the start of a boundary message payload.
/// If the payload is too short, fall back to Other.
fn decode_boundary_game_id(
    msg: RawMessage,
    make: impl FnOnce(i32) -> FlsMessage,
) -> Result<FlsMessage, DecodeError> {
    if msg.payload.len() < 4 {
        // Wire layout not confirmed — fall back to Other
        tracing::warn!(
            opcode = msg.opcode,
            payload_len = msg.payload.len(),
            "boundary signal too short for game_id, falling back to Other"
        );
        return Ok(FlsMessage::Other(msg));
    }
    // Try first 4 bytes as game_id — this is a best-effort guess.
    // If this doesn't produce sane values against the golden file,
    // the offset will need adjustment.
    let game_id = i32::from_le_bytes([
        msg.payload[0], msg.payload[1], msg.payload[2], msg.payload[3],
    ]);
    Ok(make(game_id))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib protocol::fls::tests -v 2>&1`
Expected: all 5 tests PASS

- [ ] **Step 5: Write golden file validation test**

Add to `src/protocol/fls.rs` tests:

```rust
    #[test]
    fn test_golden_file_fls_decode() {
        let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
        let messages = crate::protocol::framing::parse_messages(&data).unwrap();

        let mut gs_count = 0;
        let mut status_count = 0;
        let mut other_count = 0;
        let mut errors = 0;

        for msg in messages {
            match decode_fls(msg) {
                Ok(FlsMessage::GsMessage { .. }) => gs_count += 1,
                Ok(FlsMessage::GameStatusChange { .. }) => status_count += 1,
                Ok(FlsMessage::Other(_)) => other_count += 1,
                Ok(_) => {} // other known variants
                Err(e) => {
                    eprintln!("decode error: {e}");
                    errors += 1;
                }
            }
        }

        assert_eq!(errors, 0, "all messages should decode without errors");
        assert!(gs_count > 1000, "expected ~1202 GsMessage, got {gs_count}");
        eprintln!("FLS decode: {gs_count} GsMessage, {status_count} StatusChange, {other_count} Other");
    }
```

- [ ] **Step 6: Run golden file test**

Run: `cargo test --lib protocol::fls::tests::test_golden_file_fls_decode -- --nocapture 2>&1`
Expected: PASS, with output showing ~1202 GsMessage. If the count is wrong, the GsMessage wire format parsing needs adjustment — check byte offsets against PROTOCOL_RESEARCH.md.

- [ ] **Step 7: Commit**

```bash
git add src/protocol/fls.rs src/protocol/opcodes.rs
git commit -m "feat: implement FLS envelope decoding with golden file validation"
```

---

## Task 2: Game message decoding (`game_messages.rs`)

**Files:**
- Create: `src/protocol/game_messages.rs`

**Reference:** `PROTOCOL_RESEARCH.md` — GamePlayStatusMessage (4652) wire format. Fields in wire order: StateBuf (byte[]), TimeLeft[8] (32 bytes), PlayerWaitingFor (i32), GameID (u32), StateSize (u32), UndiffedBufferSize (u32), NStateElems (u32), PriorityPlayer (i32), CheckSum (i32), LastStateChecksum (i32), Replaying (u8), Flags (u8), GameStateTimestamp (u32). **StateBuf is the first field on the wire.**

The `meta` bytes from `FlsMessage::GsMessage` are a complete CSMessage with their own 8-byte framing header. Call `framing::read_message` on the meta bytes to get the inner opcode and payload, then dispatch.

- [ ] **Step 1: Write test for GamePlayStatusMessage**

In `src/protocol/game_messages.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_game_play_status_payload(
        game_id: u32,
        state_size: u32,
        n_state_elems: u32,
        checksum: i32,
        last_checksum: i32,
        flags: u8,
        state_buf: &[u8],
    ) -> Vec<u8> {
        let mut payload = Vec::new();
        // StateBuf: byte[] — FIRST field on the wire (int32 count + data)
        payload.extend_from_slice(&(state_buf.len() as i32).to_le_bytes());
        payload.extend_from_slice(state_buf);
        // TimeLeft[8]: 8 x int32 = 32 bytes
        for _ in 0..8 {
            payload.extend_from_slice(&0i32.to_le_bytes());
        }
        payload.extend_from_slice(&0i32.to_le_bytes()); // PlayerWaitingFor
        payload.extend_from_slice(&game_id.to_le_bytes()); // GameID
        payload.extend_from_slice(&state_size.to_le_bytes()); // StateSize
        payload.extend_from_slice(&0u32.to_le_bytes()); // UndiffedBufferSize
        payload.extend_from_slice(&n_state_elems.to_le_bytes()); // NStateElems
        payload.extend_from_slice(&0i32.to_le_bytes()); // PriorityPlayer
        payload.extend_from_slice(&checksum.to_le_bytes()); // CheckSum
        payload.extend_from_slice(&last_checksum.to_le_bytes()); // LastStateChecksum
        payload.push(0); // Replaying
        payload.push(flags); // Flags
        payload.extend_from_slice(&0u32.to_le_bytes()); // GameStateTimestamp
        payload
    }

    fn wrap_as_meta(opcode: u16, payload: &[u8]) -> Vec<u8> {
        // Build a complete CSMessage (8-byte header + payload)
        let total_len = (8 + payload.len()) as i32;
        let mut meta = Vec::new();
        meta.extend_from_slice(&total_len.to_le_bytes());
        meta.extend_from_slice(&opcode.to_le_bytes());
        meta.extend_from_slice(&0u16.to_le_bytes()); // type_check
        meta.extend_from_slice(payload);
        meta
    }

    #[test]
    fn test_decode_game_play_status() {
        let state_buf = vec![0xAA, 0xBB];
        let inner = make_game_play_status_payload(100, 2, 1, 0, 0, 0x06, &state_buf);
        let meta = wrap_as_meta(4652, &inner);

        let result = decode_game_message(&meta).unwrap();
        match result {
            GameMessage::GamePlayStatus(msg) => {
                assert_eq!(msg.game_id, 100);
                assert_eq!(msg.state_size, 2);
                assert_eq!(msg.n_state_elems, 1);
                assert_eq!(msg.flags, 0x06); // Head + Tail
                assert_eq!(msg.state_buf_raw, vec![0xAA, 0xBB]);
            }
            _ => panic!("expected GamePlayStatus"),
        }
    }

    #[test]
    fn test_decode_game_over() {
        let meta = wrap_as_meta(4632, &[]);
        let result = decode_game_message(&meta).unwrap();
        assert!(matches!(result, GameMessage::GameOver));
    }

    #[test]
    fn test_decode_unknown_game_opcode() {
        let meta = wrap_as_meta(9999, &[0x01]);
        let result = decode_game_message(&meta).unwrap();
        match result {
            GameMessage::Other { opcode } => assert_eq!(opcode, 9999),
            _ => panic!("expected Other"),
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib protocol::game_messages::tests -v 2>&1`
Expected: FAIL

- [ ] **Step 3: Implement game message decoding**

In `src/protocol/game_messages.rs`:

```rust
//! Inner game message decoding (Level 2, inside FLS MetaMessage).

use std::io::{Cursor, Read};

use super::DecodeError;
use super::framing;
use super::opcodes;

#[derive(Debug, Clone)]
pub struct GamePlayStatusMessage {
    pub time_left: [i32; 8],
    pub player_waiting_for: i32,
    pub game_id: u32,
    pub state_size: u32,
    pub undiffed_buffer_size: u32,
    pub n_state_elems: u32,
    pub priority_player: i32,
    pub checksum: i32,
    pub last_state_checksum: i32,
    pub flags: u8,
    pub game_state_timestamp: u32,
    pub state_buf_raw: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum GameMessage {
    GamePlayStatus(GamePlayStatusMessage),
    GameOver,
    Other { opcode: u16 },
}

/// Decode a game-level message from the raw MetaMessage bytes.
/// The `meta` bytes include the inner 8-byte CSMessage header.
pub fn decode_game_message(meta: &[u8]) -> Result<GameMessage, DecodeError> {
    let mut cursor = Cursor::new(meta);
    let inner = framing::read_message(&mut cursor)?;

    match inner.opcode {
        opcodes::GAME_PLAY_STATUS => decode_game_play_status(&inner.payload),
        opcodes::GAME_OVER => Ok(GameMessage::GameOver),
        _ => Ok(GameMessage::Other { opcode: inner.opcode }),
    }
}

fn decode_game_play_status(payload: &[u8]) -> Result<GameMessage, DecodeError> {
    let mut cursor = Cursor::new(payload);
    let mut buf4 = [0u8; 4];

    // StateBuf: byte[] — FIRST field on the wire (int32 count + data)
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: StateBuf length",
    })?;
    let buf_len = i32::from_le_bytes(buf4);
    if buf_len < 0 {
        return Err(DecodeError::UnexpectedEof {
            context: "GamePlayStatus: negative StateBuf length",
        });
    }
    let mut state_buf_raw = vec![0u8; buf_len as usize];
    cursor.read_exact(&mut state_buf_raw).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: StateBuf data",
    })?;

    // TimeLeft[8]: 8 x int32
    let mut time_left = [0i32; 8];
    for slot in &mut time_left {
        cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
            context: "GamePlayStatus: TimeLeft",
        })?;
        *slot = i32::from_le_bytes(buf4);
    }

    // PlayerWaitingFor: int32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: PlayerWaitingFor",
    })?;
    let player_waiting_for = i32::from_le_bytes(buf4);

    // GameID: uint32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: GameID",
    })?;
    let game_id = u32::from_le_bytes(buf4);

    // StateSize: uint32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: StateSize",
    })?;
    let state_size = u32::from_le_bytes(buf4);

    // UndiffedBufferSize: uint32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: UndiffedBufferSize",
    })?;
    let undiffed_buffer_size = u32::from_le_bytes(buf4);

    // NStateElems: uint32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: NStateElems",
    })?;
    let n_state_elems = u32::from_le_bytes(buf4);

    // PriorityPlayer: int32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: PriorityPlayer",
    })?;
    let priority_player = i32::from_le_bytes(buf4);

    // CheckSum: int32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: CheckSum",
    })?;
    let checksum = i32::from_le_bytes(buf4);

    // LastStateChecksum: int32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: LastStateChecksum",
    })?;
    let last_state_checksum = i32::from_le_bytes(buf4);

    // Replaying: byte — consumed, not stored
    let mut buf1 = [0u8; 1];
    cursor.read_exact(&mut buf1).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: Replaying",
    })?;

    // Flags: byte
    cursor.read_exact(&mut buf1).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: Flags",
    })?;
    let flags = buf1[0];

    // GameStateTimestamp: uint32
    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
        context: "GamePlayStatus: GameStateTimestamp",
    })?;
    let game_state_timestamp = u32::from_le_bytes(buf4);

    Ok(GameMessage::GamePlayStatus(GamePlayStatusMessage {
        time_left,
        player_waiting_for,
        game_id,
        state_size,
        undiffed_buffer_size,
        n_state_elems,
        priority_player,
        checksum,
        last_state_checksum,
        flags,
        game_state_timestamp,
        state_buf_raw,
    }))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib protocol::game_messages::tests -v 2>&1`
Expected: all 3 tests PASS

- [ ] **Step 5: Write golden file validation test**

Add to `src/protocol/game_messages.rs` tests:

```rust
    #[test]
    fn test_golden_file_game_messages() {
        let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
        let messages = crate::protocol::framing::parse_messages(&data).unwrap();

        let mut play_status_count = 0;
        let mut game_over_count = 0;
        let mut other_game_count = 0;
        let mut decode_errors = 0;

        for msg in &messages {
            if msg.opcode != 1153 && msg.opcode != 1156 {
                continue;
            }
            let fls = crate::protocol::fls::decode_fls(msg.clone()).unwrap();
            let meta = match fls {
                crate::protocol::fls::FlsMessage::GsMessage { meta, .. } => meta,
                _ => continue,
            };
            match decode_game_message(&meta) {
                Ok(GameMessage::GamePlayStatus(_)) => play_status_count += 1,
                Ok(GameMessage::GameOver) => game_over_count += 1,
                Ok(GameMessage::Other { .. }) => other_game_count += 1,
                Err(e) => {
                    eprintln!("game decode error: {e}");
                    decode_errors += 1;
                }
            }
        }

        assert_eq!(decode_errors, 0, "all game messages should decode");
        assert!(play_status_count > 0, "expected GamePlayStatus messages");
        eprintln!(
            "Game messages: {play_status_count} PlayStatus, {game_over_count} GameOver, {other_game_count} Other"
        );
    }
```

- [ ] **Step 6: Run golden file test**

Run: `cargo test --lib protocol::game_messages::tests::test_golden_file_game_messages -- --nocapture 2>&1`
Expected: PASS. If decode errors appear, the GamePlayStatusMessage field offsets need adjustment — compare against PROTOCOL_RESEARCH.md byte-by-byte.

- [ ] **Step 7: Commit**

```bash
git add src/protocol/game_messages.rs
git commit -m "feat: implement game message decoding with GamePlayStatusMessage parsing"
```

---

## Task 3: StateBuf assembly and diff processing (`statebuf.rs` — part 1)

**Files:**
- Create: `src/protocol/statebuf.rs`

**Reference:** `PROTOCOL_RESEARCH.md` — ApplyDiffs2 algorithm, checksum, assembly buffer flags. `docs/specs/2026-03-18-rust-decoder-design.md` Section 1 — statebuf.rs.

This task implements the assembly buffer, checksum, and diff algorithm. Element parsing is Task 4.

- [ ] **Step 1: Write checksum test**

In `src/protocol/statebuf.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_empty() {
        assert_eq!(compute_checksum(&[]), opcodes::CHECKSUM_SEED);
    }

    #[test]
    fn test_checksum_deterministic() {
        let data = vec![0x01, 0x02, 0x03];
        let c1 = compute_checksum(&data);
        let c2 = compute_checksum(&data);
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_checksum_differs_for_different_data() {
        let c1 = compute_checksum(&[0x01]);
        let c2 = compute_checksum(&[0x02]);
        assert_ne!(c1, c2);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib protocol::statebuf::tests -v 2>&1`
Expected: FAIL

- [ ] **Step 3: Implement checksum**

At the top of `src/protocol/statebuf.rs`:

```rust
//! StateBuf assembly, diff processing, and element parsing.

use super::DecodeError;
use super::opcodes;
use super::game_messages::GamePlayStatusMessage;

/// Compute the rolling checksum over a byte buffer.
/// Algorithm: seed = 826366246, then for each byte: checksum = (checksum << 1) + byte
pub fn compute_checksum(data: &[u8]) -> i32 {
    let mut checksum = opcodes::CHECKSUM_SEED;
    for &b in data {
        checksum = (checksum << 1).wrapping_add(b as i32);
    }
    checksum
}
```

- [ ] **Step 4: Run checksum tests**

Run: `cargo test --lib protocol::statebuf::tests -v 2>&1`
Expected: all 3 PASS

- [ ] **Step 5: Write diff algorithm tests**

Add to tests in `src/protocol/statebuf.rs`:

```rust
    #[test]
    fn test_apply_diffs_literal() {
        // Short literal: byte 0x03 means "3 literal bytes follow"
        let old_state = vec![0xAA, 0xBB, 0xCC];
        let diff = vec![0x03, 0x11, 0x22, 0x33];
        let result = apply_diffs(&old_state, &diff).unwrap();
        assert_eq!(result, vec![0x11, 0x22, 0x33]);
    }

    #[test]
    fn test_apply_diffs_copy() {
        // 0x00 copy: uint16 count=3, int16 seek=0 (from start)
        let old_state = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let diff = vec![
            0x00,
            0x03, 0x00, // count = 3
            0x00, 0x00, // seek = 0 (absolute from current cursor=0)
        ];
        let result = apply_diffs(&old_state, &diff).unwrap();
        assert_eq!(result, vec![0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn test_apply_diffs_short_copy() {
        // 0x80 | count, sbyte seek
        // 0x83 = 0x80 | 3 → count=3, then sbyte seek
        let old_state = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let diff = vec![0x83, 0x00]; // count=3, seek=0
        let result = apply_diffs(&old_state, &diff).unwrap();
        assert_eq!(result, vec![0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn test_apply_diffs_out_of_bounds() {
        let old_state = vec![0xAA];
        let diff = vec![0x00, 0x05, 0x00, 0x00, 0x00]; // copy 5, seek 0 — out of bounds
        let result = apply_diffs(&old_state, &diff);
        assert!(result.is_err());
    }
```

- [ ] **Step 6: Run diff tests to verify they fail**

Run: `cargo test --lib protocol::statebuf::tests -v 2>&1`
Expected: FAIL — `apply_diffs` not defined

- [ ] **Step 7: Implement `apply_diffs`**

Add to `src/protocol/statebuf.rs`:

```rust
/// Apply a diff to an old state buffer, producing a new state buffer.
/// Implements the ApplyDiffs2 algorithm from PROTOCOL_RESEARCH.md.
pub fn apply_diffs(old_state: &[u8], diff_data: &[u8]) -> Result<Vec<u8>, DecodeError> {
    let mut output = Vec::new();
    let mut diff_cursor = 0usize;
    let mut old_cursor = 0usize;

    while diff_cursor < diff_data.len() {
        let leading = diff_data[diff_cursor];
        diff_cursor += 1;

        if leading == 0x00 {
            // Copy: uint16 count + int16 seek
            if diff_cursor + 4 > diff_data.len() {
                return Err(DecodeError::DiffOutOfBounds {
                    context: "copy opcode truncated",
                });
            }
            let count = u16::from_le_bytes([diff_data[diff_cursor], diff_data[diff_cursor + 1]]) as usize;
            let seek = i16::from_le_bytes([diff_data[diff_cursor + 2], diff_data[diff_cursor + 3]]);
            diff_cursor += 4;

            let new_pos = (old_cursor as isize) + (seek as isize);
            if new_pos < 0 || new_pos as usize + count > old_state.len() {
                return Err(DecodeError::DiffOutOfBounds {
                    context: "copy seek/count out of bounds",
                });
            }
            old_cursor = new_pos as usize;
            output.extend_from_slice(&old_state[old_cursor..old_cursor + count]);
            old_cursor += count;
        } else if leading & 0x80 != 0 {
            let low7 = (leading & 0x7F) as usize;
            if low7 == 0 {
                // 0x80 with low 7 bits == 0
                if diff_cursor >= diff_data.len() {
                    return Err(DecodeError::DiffOutOfBounds {
                        context: "0x80 opcode: missing next byte",
                    });
                }
                let next = diff_data[diff_cursor];
                diff_cursor += 1;

                if next == 0 {
                    // Long literal: 3-byte LE count, then that many literal bytes
                    if diff_cursor + 3 > diff_data.len() {
                        return Err(DecodeError::DiffOutOfBounds {
                            context: "long literal: count truncated",
                        });
                    }
                    let count = diff_data[diff_cursor] as usize
                        | ((diff_data[diff_cursor + 1] as usize) << 8)
                        | ((diff_data[diff_cursor + 2] as usize) << 16);
                    diff_cursor += 3;

                    if diff_cursor + count > diff_data.len() {
                        return Err(DecodeError::DiffOutOfBounds {
                            context: "long literal: data truncated",
                        });
                    }
                    output.extend_from_slice(&diff_data[diff_cursor..diff_cursor + count]);
                    diff_cursor += count;
                } else {
                    // Medium copy: next byte = count, int16 seek
                    let count = next as usize;
                    if diff_cursor + 2 > diff_data.len() {
                        return Err(DecodeError::DiffOutOfBounds {
                            context: "medium copy: seek truncated",
                        });
                    }
                    let seek = i16::from_le_bytes([diff_data[diff_cursor], diff_data[diff_cursor + 1]]);
                    diff_cursor += 2;

                    let new_pos = (old_cursor as isize) + (seek as isize);
                    if new_pos < 0 || new_pos as usize + count > old_state.len() {
                        return Err(DecodeError::DiffOutOfBounds {
                            context: "medium copy seek/count out of bounds",
                        });
                    }
                    old_cursor = new_pos as usize;
                    output.extend_from_slice(&old_state[old_cursor..old_cursor + count]);
                    old_cursor += count;
                }
            } else {
                // Short copy: count = low 7 bits, sbyte seek
                let count = low7;
                if diff_cursor >= diff_data.len() {
                    return Err(DecodeError::DiffOutOfBounds {
                        context: "short copy: seek truncated",
                    });
                }
                let seek = diff_data[diff_cursor] as i8;
                diff_cursor += 1;

                let new_pos = (old_cursor as isize) + (seek as isize);
                if new_pos < 0 || new_pos as usize + count > old_state.len() {
                    return Err(DecodeError::DiffOutOfBounds {
                        context: "short copy seek/count out of bounds",
                    });
                }
                old_cursor = new_pos as usize;
                output.extend_from_slice(&old_state[old_cursor..old_cursor + count]);
                old_cursor += count;
            }
        } else {
            // 0x01..0x7F: literal
            let count = leading as usize;
            if diff_cursor + count > diff_data.len() {
                return Err(DecodeError::DiffOutOfBounds {
                    context: "literal: data truncated",
                });
            }
            output.extend_from_slice(&diff_data[diff_cursor..diff_cursor + count]);
            diff_cursor += count;
        }
    }

    Ok(output)
}
```

- [ ] **Step 8: Run all tests**

Run: `cargo test --lib protocol::statebuf::tests -v 2>&1`
Expected: all 7 tests PASS

- [ ] **Step 9: Implement `StateBufProcessor`**

Add to `src/protocol/statebuf.rs`:

```rust
/// Manages StateBuf assembly across chunked messages and diff application.
pub struct StateBufProcessor {
    assembly_buffer: Vec<u8>,
    previous_state: Vec<u8>,
}

impl StateBufProcessor {
    pub fn new() -> Self {
        Self {
            assembly_buffer: Vec::new(),
            previous_state: Vec::new(),
        }
    }

    /// Process a GamePlayStatusMessage, returning the assembled state bytes
    /// (after assembly and optional diff application, with checksum validation).
    /// Returns `Ok(None)` if waiting for more chunks (non-tail message).
    /// Returns `Ok(Some(bytes))` when assembly is complete.
    pub fn process(&mut self, msg: &GamePlayStatusMessage) -> Result<Option<Vec<u8>>, DecodeError> {
        let is_diff = msg.flags & opcodes::FLAG_GAMESTATE_CONTAINS_DIFFS != 0;
        let is_head = msg.flags & opcodes::FLAG_GAMESTATE_HEAD != 0;
        let is_tail = msg.flags & opcodes::FLAG_GAMESTATE_TAIL != 0;

        // Head: reset assembly buffer
        if is_head {
            if !self.assembly_buffer.is_empty() {
                tracing::warn!("GamestateHead with non-empty assembly buffer, discarding {} bytes", self.assembly_buffer.len());
            }
            self.assembly_buffer.clear();
        }

        // Append chunk
        self.assembly_buffer.extend_from_slice(&msg.state_buf_raw);

        // Not tail yet — wait for more chunks
        if !is_tail {
            return Ok(None);
        }

        // Tail: process the assembled buffer
        let assembled = std::mem::take(&mut self.assembly_buffer);

        let final_state = if is_diff {
            // Validate last_state_checksum against previous_state
            if self.previous_state.is_empty() {
                return Err(DecodeError::UnexpectedEof {
                    context: "diff tail without prior state",
                });
            }
            let prev_checksum = compute_checksum(&self.previous_state);
            if prev_checksum != msg.last_state_checksum {
                return Err(DecodeError::InvalidChecksum {
                    expected: msg.last_state_checksum,
                    got: prev_checksum,
                });
            }

            // Apply diff
            let result = apply_diffs(&self.previous_state, &assembled)?;

            // Validate size
            if msg.state_size != 0 && result.len() != msg.state_size as usize {
                return Err(DecodeError::DiffSizeMismatch {
                    expected: msg.state_size,
                    got: result.len() as u32,
                });
            }

            // Validate checksum of result
            let result_checksum = compute_checksum(&result);
            if result_checksum != msg.checksum {
                return Err(DecodeError::InvalidChecksum {
                    expected: msg.checksum,
                    got: result_checksum,
                });
            }

            result
        } else {
            // Full state — validate checksum
            let checksum = compute_checksum(&assembled);
            if checksum != msg.checksum {
                return Err(DecodeError::InvalidChecksum {
                    expected: msg.checksum,
                    got: checksum,
                });
            }
            assembled
        };

        // Store for next diff
        self.previous_state = final_state.clone();
        Ok(Some(final_state))
    }

    /// Reset between games.
    pub fn reset(&mut self) {
        self.assembly_buffer.clear();
        self.previous_state.clear();
    }
}
```

- [ ] **Step 10: Write StateBufProcessor tests**

Add to tests:

```rust
    #[test]
    fn test_processor_full_state() {
        let mut proc = StateBufProcessor::new();
        let state_data = vec![0x01, 0x02, 0x03];
        let checksum = compute_checksum(&state_data);

        let msg = GamePlayStatusMessage {
            time_left: [0; 8],
            player_waiting_for: 0,
            game_id: 1,
            state_size: state_data.len() as u32,
            undiffed_buffer_size: 0,
            n_state_elems: 0,
            priority_player: 0,
            checksum,
            last_state_checksum: 0,
            flags: opcodes::FLAG_GAMESTATE_HEAD | opcodes::FLAG_GAMESTATE_TAIL, // 0x06
            game_state_timestamp: 0,
            state_buf_raw: state_data.clone(),
        };

        let result = proc.process(&msg).unwrap();
        assert_eq!(result, Some(state_data));
    }

    #[test]
    fn test_processor_bad_checksum() {
        let mut proc = StateBufProcessor::new();
        let msg = GamePlayStatusMessage {
            time_left: [0; 8],
            player_waiting_for: 0,
            game_id: 1,
            state_size: 3,
            undiffed_buffer_size: 0,
            n_state_elems: 0,
            priority_player: 0,
            checksum: 12345, // wrong
            last_state_checksum: 0,
            flags: opcodes::FLAG_GAMESTATE_HEAD | opcodes::FLAG_GAMESTATE_TAIL,
            game_state_timestamp: 0,
            state_buf_raw: vec![0x01, 0x02, 0x03],
        };

        let result = proc.process(&msg);
        assert!(matches!(result, Err(DecodeError::InvalidChecksum { .. })));
    }
```

- [ ] **Step 11: Run all statebuf tests**

Run: `cargo test --lib protocol::statebuf::tests -v 2>&1`
Expected: all PASS

- [ ] **Step 12: Write golden file checksum validation test**

Add to tests:

```rust
    #[test]
    fn test_golden_file_statebuf_assembly() {
        let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
        let messages = crate::protocol::framing::parse_messages(&data).unwrap();

        let mut processor = StateBufProcessor::new();
        let mut success_count = 0;
        let mut pending_count = 0;
        let mut errors = Vec::new();

        for msg in &messages {
            if msg.opcode != 1153 && msg.opcode != 1156 {
                continue;
            }
            let fls = match crate::protocol::fls::decode_fls(msg.clone()) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let meta = match fls {
                crate::protocol::fls::FlsMessage::GsMessage { meta, .. } => meta,
                _ => continue,
            };
            let game_msg = match crate::protocol::game_messages::decode_game_message(&meta) {
                Ok(g) => g,
                Err(_) => continue,
            };
            let play_status = match game_msg {
                crate::protocol::game_messages::GameMessage::GamePlayStatus(ps) => ps,
                _ => continue,
            };

            match processor.process(&play_status) {
                Ok(Some(_)) => success_count += 1,
                Ok(None) => pending_count += 1, // waiting for tail
                Err(e) => errors.push(format!("{e}")),
            }
        }

        eprintln!(
            "StateBuf assembly: {success_count} success, {pending_count} pending, {} errors",
            errors.len()
        );
        for (i, e) in errors.iter().enumerate().take(5) {
            eprintln!("  error {i}: {e}");
        }
        assert!(errors.is_empty(), "expected no assembly errors");
        assert!(success_count > 0, "expected successful assemblies");
    }
```

- [ ] **Step 13: Run golden file test**

Run: `cargo test --lib protocol::statebuf::tests::test_golden_file_statebuf_assembly -- --nocapture 2>&1`
Expected: PASS with all checksums validating. If checksum errors occur, the checksum algorithm or diff implementation needs debugging — compare a single known-good message byte-by-byte.

- [ ] **Step 14: Commit**

```bash
git add src/protocol/statebuf.rs
git commit -m "feat: implement StateBuf assembly, diff algorithm, and checksum validation"
```

---

## Task 4: StateBuf element parsing (`statebuf.rs` — part 2)

**Files:**
- Modify: `src/protocol/statebuf.rs`

**Reference:** `docs/specs/2026-03-18-rust-decoder-design.md` Section 1 — statebuf.rs Elements. `PROTOCOL_RESEARCH.md` — ThingElement, PropertyContainer, PlayerStatusElement, TurnStepElement wire formats.

- [ ] **Step 1: Write element parsing tests**

Add types and tests to `src/protocol/statebuf.rs`:

```rust
    #[test]
    fn test_parse_thing_element_simple() {
        let mut payload = Vec::new();
        // from_zone: i32 = 2 (Library)
        payload.extend_from_slice(&2i32.to_le_bytes());
        // PropertyContainer: one Int8 property (THINGNUMBER), then terminator
        // key_with_type: PROP_TYPE_INT8 | (THINGNUMBER & PROP_KEY_MASK)
        let key = opcodes::PROP_TYPE_INT8 | (opcodes::THINGNUMBER & opcodes::PROP_KEY_MASK);
        payload.extend_from_slice(&key.to_le_bytes());
        payload.push(42); // int8 value
        // terminator
        payload.extend_from_slice(&0u32.to_le_bytes());

        let elem_data = make_element_bytes(opcodes::STATE_ELEM_THING, &payload);
        let elements = parse_elements(&elem_data).unwrap();
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            StateElement::Thing(thing) => {
                assert_eq!(thing.from_zone, 2);
                let key = opcodes::THINGNUMBER & opcodes::PROP_KEY_MASK;
                assert!(thing.props.contains_key(&key));
            }
            _ => panic!("expected Thing"),
        }
    }

    #[test]
    fn test_parse_turn_step_element() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&5i32.to_le_bytes()); // TurnNumber
        payload.extend_from_slice(&4u32.to_le_bytes()); // GamePhase = PreCombatMain
        // Remaining fields are variable-length; we'll use the element size header to skip them

        let elem_data = make_element_bytes(opcodes::STATE_ELEM_TURN_STEP, &payload);
        let elements = parse_elements(&elem_data).unwrap();
        assert_eq!(elements.len(), 1);
        match &elements[0] {
            StateElement::TurnStep(ts) => {
                assert_eq!(ts.turn_number, 5);
                assert_eq!(ts.phase, 4);
            }
            _ => panic!("expected TurnStep"),
        }
    }

    #[test]
    fn test_parse_unknown_element_type() {
        let payload = vec![0x01, 0x02, 0x03];
        let elem_data = make_element_bytes(255, &payload);
        let elements = parse_elements(&elem_data).unwrap();
        assert_eq!(elements.len(), 1);
        assert!(matches!(elements[0], StateElement::Other { element_type: 255, .. }));
    }

    /// Helper: wrap payload bytes in a state element header (4-byte size + 4-byte type).
    fn make_element_bytes(elem_type: u32, payload: &[u8]) -> Vec<u8> {
        let total_size = (8 + payload.len()) as i32;
        let mut data = Vec::new();
        data.extend_from_slice(&total_size.to_le_bytes());
        data.extend_from_slice(&elem_type.to_le_bytes());
        data.extend_from_slice(payload);
        data
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib protocol::statebuf::tests -v 2>&1`
Expected: FAIL — `parse_elements`, `StateElement`, etc. not defined

- [ ] **Step 3: Implement element types and parsing**

Add to `src/protocol/statebuf.rs` (above the tests module):

```rust
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};

/// A parsed state element from the StateBuf.
#[derive(Debug, Clone)]
pub enum StateElement {
    PlayerStatus(PlayerStatusElement),
    TurnStep(TurnStepElement),
    Thing(ThingElement),
    Other { element_type: u32, raw: Vec<u8> },
}

#[derive(Debug, Clone)]
pub struct PlayerStatusElement {
    pub life: Vec<i16>,
    pub hand_count: Vec<i16>,
    pub library_count: Vec<i16>,
    pub graveyard_count: Vec<i16>,
    pub time_left: Vec<i32>,
    pub active_player: u8,
}

#[derive(Debug, Clone)]
pub struct TurnStepElement {
    pub turn_number: i32,
    pub phase: u8,
}

#[derive(Debug, Clone)]
pub struct ThingElement {
    pub from_zone: i32,
    pub props: HashMap<u32, PropertyValue>,
}

#[derive(Debug, Clone)]
pub enum PropertyValue {
    Int(i32),
    Str(String),
    List(HashMap<u32, PropertyValue>),
}

/// Parse state elements from an assembled state buffer.
pub fn parse_elements(data: &[u8]) -> Result<Vec<StateElement>, DecodeError> {
    let mut cursor = Cursor::new(data);
    let mut elements = Vec::new();
    let mut buf4 = [0u8; 4];

    while (cursor.position() as usize) < data.len() {
        let elem_start = cursor.position() as usize;

        // total_size: i32 (includes this 8-byte header)
        if cursor.read_exact(&mut buf4).is_err() {
            break;
        }
        let total_size = i32::from_le_bytes(buf4);
        if total_size < 8 {
            tracing::warn!("element total_size {} < 8, skipping rest", total_size);
            break;
        }

        // element_type: u32
        cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
            context: "element type",
        })?;
        let element_type = u32::from_le_bytes(buf4);

        let payload_size = (total_size as usize) - 8;
        let payload_start = cursor.position() as usize;

        if payload_start + payload_size > data.len() {
            tracing::warn!("element payload extends past buffer, skipping");
            break;
        }

        let payload = &data[payload_start..payload_start + payload_size];

        let element = match element_type {
            opcodes::STATE_ELEM_THING => {
                match parse_thing_element(payload) {
                    Ok(Some(thing)) => StateElement::Thing(thing),
                    Ok(None) => {
                        // THINGNUMBER absent — discard
                        cursor.seek(SeekFrom::Start((elem_start + total_size as usize) as u64)).ok();
                        continue;
                    }
                    Err(_) => {
                        tracing::warn!("failed to parse ThingElement, storing as Other");
                        StateElement::Other { element_type, raw: payload.to_vec() }
                    }
                }
            }
            opcodes::STATE_ELEM_TURN_STEP => {
                match parse_turn_step_element(payload) {
                    Ok(ts) => StateElement::TurnStep(ts),
                    Err(_) => StateElement::Other { element_type, raw: payload.to_vec() },
                }
            }
            opcodes::STATE_ELEM_PLAYER_STATUS => {
                match parse_player_status_element(payload) {
                    Ok(ps) => StateElement::PlayerStatus(ps),
                    Err(_) => StateElement::Other { element_type, raw: payload.to_vec() },
                }
            }
            opcodes::STATE_ELEM_MINI_CHANGE => {
                tracing::warn!("MiniChange element encountered (type 200), storing as Other");
                StateElement::Other { element_type, raw: payload.to_vec() }
            }
            _ => StateElement::Other { element_type, raw: payload.to_vec() },
        };

        elements.push(element);

        // Advance cursor to next element
        cursor.seek(SeekFrom::Start((elem_start + total_size as usize) as u64)).map_err(|_| {
            DecodeError::UnexpectedEof { context: "element seek" }
        })?;
    }

    Ok(elements)
}

fn parse_turn_step_element(payload: &[u8]) -> Result<TurnStepElement, DecodeError> {
    if payload.len() < 8 {
        return Err(DecodeError::UnexpectedEof { context: "TurnStep too short" });
    }
    let turn_number = i32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    // GamePhase is stored as i32 but we only need the low byte
    let phase = payload[4];
    Ok(TurnStepElement { turn_number, phase })
}

fn parse_player_status_element(payload: &[u8]) -> Result<PlayerStatusElement, DecodeError> {
    let mut cursor = Cursor::new(payload);
    let mut buf4 = [0u8; 4];
    let mut buf2 = [0u8; 2];

    let life = read_i16_array(&mut cursor, &mut buf4, &mut buf2)?;
    let hand_count = read_i16_array(&mut cursor, &mut buf4, &mut buf2)?;
    let library_count = read_i16_array(&mut cursor, &mut buf4, &mut buf2)?;
    let graveyard_count = read_i16_array(&mut cursor, &mut buf4, &mut buf2)?;
    let time_left = read_i32_array(&mut cursor, &mut buf4)?;

    // background_image_names: string[] — read and discard
    // Array of wide strings: int32 count, then each string is int32 char_count + chars
    if cursor.read_exact(&mut buf4).is_ok() {
        let str_count = i32::from_le_bytes(buf4);
        for _ in 0..str_count {
            // Each wide string: int32 char_count + char_count*2 bytes
            if cursor.read_exact(&mut buf4).is_err() { break; }
            let char_count = i32::from_le_bytes(buf4);
            if char_count > 0 {
                cursor.seek(SeekFrom::Current(char_count as i64 * 2)).ok();
            }
        }
    }

    // ActivePlayer: byte
    let mut buf1 = [0u8; 1];
    let active_player = if cursor.read_exact(&mut buf1).is_ok() {
        buf1[0]
    } else {
        0
    };

    Ok(PlayerStatusElement {
        life,
        hand_count,
        library_count,
        graveyard_count,
        time_left,
        active_player,
    })
}

fn read_i16_array(cursor: &mut Cursor<&[u8]>, buf4: &mut [u8; 4], buf2: &mut [u8; 2]) -> Result<Vec<i16>, DecodeError> {
    cursor.read_exact(buf4).map_err(|_| DecodeError::UnexpectedEof { context: "i16 array count" })?;
    let count = i32::from_le_bytes(*buf4);
    let mut values = Vec::with_capacity(count.max(0) as usize);
    for _ in 0..count {
        cursor.read_exact(buf2).map_err(|_| DecodeError::UnexpectedEof { context: "i16 array value" })?;
        values.push(i16::from_le_bytes(*buf2));
    }
    Ok(values)
}

fn read_i32_array(cursor: &mut Cursor<&[u8]>, buf4: &mut [u8; 4]) -> Result<Vec<i32>, DecodeError> {
    cursor.read_exact(buf4).map_err(|_| DecodeError::UnexpectedEof { context: "i32 array count" })?;
    let count = i32::from_le_bytes(*buf4);
    let mut values = Vec::with_capacity(count.max(0) as usize);
    for _ in 0..count {
        cursor.read_exact(buf4).map_err(|_| DecodeError::UnexpectedEof { context: "i32 array value" })?;
        values.push(i32::from_le_bytes(*buf4));
    }
    Ok(values)
}

fn parse_thing_element(payload: &[u8]) -> Result<Option<ThingElement>, DecodeError> {
    if payload.len() < 4 {
        return Err(DecodeError::UnexpectedEof { context: "ThingElement: from_zone" });
    }
    let from_zone = i32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let props = parse_property_container(&payload[4..])?;

    // Check for THINGNUMBER
    let thing_key = opcodes::THINGNUMBER & opcodes::PROP_KEY_MASK;
    if !props.contains_key(&thing_key) {
        tracing::warn!("ThingElement without THINGNUMBER, discarding");
        return Ok(None);
    }

    Ok(Some(ThingElement { from_zone, props }))
}

fn parse_property_container(data: &[u8]) -> Result<HashMap<u32, PropertyValue>, DecodeError> {
    let mut props = HashMap::new();
    let mut cursor = Cursor::new(data);
    let mut buf4 = [0u8; 4];

    loop {
        if cursor.read_exact(&mut buf4).is_err() {
            break;
        }
        let key_with_type = u32::from_le_bytes(buf4);
        if key_with_type == 0 {
            break; // terminator
        }

        let type_tag = key_with_type & opcodes::PROP_TYPE_MASK;
        let mut key = key_with_type & opcodes::PROP_KEY_MASK;

        match type_tag {
            opcodes::PROP_TYPE_INT8 => {
                let mut buf1 = [0u8; 1];
                cursor.read_exact(&mut buf1).map_err(|_| DecodeError::UnexpectedEof {
                    context: "PropertyContainer: Int8 value",
                })?;
                props.insert(key, PropertyValue::Int(buf1[0] as i8 as i32));
            }
            opcodes::PROP_TYPE_INT32 => {
                // Key remapping: (keyWithType & 0xD7FFFFFF) | 0x20000000, then mask to key
                let remapped = (key_with_type & 0xD7FFFFFF) | 0x20000000;
                key = remapped & opcodes::PROP_KEY_MASK;
                cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
                    context: "PropertyContainer: Int32 value",
                })?;
                props.insert(key, PropertyValue::Int(i32::from_le_bytes(buf4)));
            }
            opcodes::PROP_TYPE_STRING => {
                let mut buf2 = [0u8; 2];
                cursor.read_exact(&mut buf2).map_err(|_| DecodeError::UnexpectedEof {
                    context: "PropertyContainer: String length",
                })?;
                let length = u16::from_le_bytes(buf2);

                if length == 0xFFFF {
                    // String table reference
                    cursor.read_exact(&mut buf4).map_err(|_| DecodeError::UnexpectedEof {
                        context: "PropertyContainer: StringTable index",
                    })?;
                    let idx = u32::from_le_bytes(buf4);
                    props.insert(key, PropertyValue::Str(format!("<strtable:{idx}>")));
                } else {
                    let mut str_bytes = vec![0u8; length as usize];
                    cursor.read_exact(&mut str_bytes).map_err(|_| DecodeError::UnexpectedEof {
                        context: "PropertyContainer: String data",
                    })?;
                    // ISO-8859-1: each byte maps directly to a Unicode code point
                    let s: String = str_bytes.iter().map(|&b| b as char).collect();
                    props.insert(key, PropertyValue::Str(s));
                }
            }
            opcodes::PROP_TYPE_LIST => {
                // Nested attribute list — recursively parse until 0x00000000 terminator
                // We need to parse from the current cursor position
                let remaining_start = cursor.position() as usize;
                let remaining = &data[remaining_start..];
                let nested = parse_property_container(remaining)?;
                // Advance cursor past the nested data + terminator
                // We can't easily know how many bytes were consumed, so re-scan
                let consumed = count_property_container_bytes(remaining);
                cursor.seek(SeekFrom::Current(consumed as i64)).ok();
                props.insert(key, PropertyValue::List(nested));
            }
            opcodes::PROP_TYPE_FUNCTION => {
                // No value, skip
            }
            opcodes::PROP_TYPE_STRING_CONSTANT => {
                // Abort the containing ThingElement
                tracing::warn!("StringConstant encountered in PropertyContainer, aborting element");
                return Err(DecodeError::UnexpectedEof {
                    context: "StringConstant in PropertyContainer",
                });
            }
            _ => {
                tracing::warn!(type_tag = type_tag, "unknown PropertyContainer type tag, aborting element");
                return Err(DecodeError::UnexpectedEof {
                    context: "unknown PropertyContainer type tag",
                });
            }
        }
    }

    Ok(props)
}

/// Count the number of bytes consumed by a property container (including the 0x00000000 terminator).
fn count_property_container_bytes(data: &[u8]) -> usize {
    let mut cursor = Cursor::new(data);
    let mut buf4 = [0u8; 4];

    loop {
        if cursor.read_exact(&mut buf4).is_err() {
            return cursor.position() as usize;
        }
        let key_with_type = u32::from_le_bytes(buf4);
        if key_with_type == 0 {
            return cursor.position() as usize;
        }

        let type_tag = key_with_type & opcodes::PROP_TYPE_MASK;
        match type_tag {
            opcodes::PROP_TYPE_INT8 => { cursor.seek(SeekFrom::Current(1)).ok(); }
            opcodes::PROP_TYPE_INT32 => { cursor.seek(SeekFrom::Current(4)).ok(); }
            opcodes::PROP_TYPE_STRING => {
                let mut buf2 = [0u8; 2];
                if cursor.read_exact(&mut buf2).is_err() { return cursor.position() as usize; }
                let length = u16::from_le_bytes(buf2);
                if length == 0xFFFF {
                    cursor.seek(SeekFrom::Current(4)).ok();
                } else {
                    cursor.seek(SeekFrom::Current(length as i64)).ok();
                }
            }
            opcodes::PROP_TYPE_LIST => {
                let pos = cursor.position() as usize;
                let remaining = &data[pos..];
                let consumed = count_property_container_bytes(remaining);
                cursor.seek(SeekFrom::Current(consumed as i64)).ok();
            }
            opcodes::PROP_TYPE_FUNCTION => { /* no value */ }
            _ => { return cursor.position() as usize; }
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib protocol::statebuf::tests -v 2>&1`
Expected: all tests PASS (both new element tests and previous assembly/diff tests)

- [ ] **Step 5: Write golden file element count validation test**

Add to tests:

```rust
    #[test]
    fn test_golden_file_element_parsing() {
        let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
        let messages = crate::protocol::framing::parse_messages(&data).unwrap();

        let mut processor = StateBufProcessor::new();
        let mut total_elements = 0;
        let mut count_mismatches = 0;
        let mut parse_errors = 0;

        for msg in &messages {
            if msg.opcode != 1153 && msg.opcode != 1156 { continue; }
            let fls = match crate::protocol::fls::decode_fls(msg.clone()) {
                Ok(f) => f, Err(_) => continue,
            };
            let meta = match fls {
                crate::protocol::fls::FlsMessage::GsMessage { meta, .. } => meta,
                _ => continue,
            };
            let game_msg = match crate::protocol::game_messages::decode_game_message(&meta) {
                Ok(g) => g, Err(_) => continue,
            };
            let ps = match game_msg {
                crate::protocol::game_messages::GameMessage::GamePlayStatus(ps) => ps,
                _ => continue,
            };

            let expected_elems = ps.n_state_elems;
            let assembled = match processor.process(&ps) {
                Ok(Some(a)) => a,
                Ok(None) => continue, // waiting for tail
                Err(_) => continue,
            };

            match parse_elements(&assembled) {
                Ok(elems) => {
                    total_elements += elems.len();
                    if elems.len() != expected_elems as usize {
                        count_mismatches += 1;
                    }
                }
                Err(e) => {
                    eprintln!("element parse error: {e}");
                    parse_errors += 1;
                }
            }
        }

        eprintln!(
            "Element parsing: {total_elements} total, {count_mismatches} count mismatches, {parse_errors} errors"
        );
        assert_eq!(parse_errors, 0, "no element parse errors");
    }
```

- [ ] **Step 6: Run golden file element test**

Run: `cargo test --lib protocol::statebuf::tests::test_golden_file_element_parsing -- --nocapture 2>&1`
Expected: PASS. Some count mismatches are tolerable (MiniChange elements counted in n_state_elems but parsed as Other, or THINGNUMBER-less things discarded). Zero parse errors is the goal. If mismatches are high, investigate element header parsing.

- [ ] **Step 7: Commit**

```bash
git add src/protocol/statebuf.rs
git commit -m "feat: implement StateBuf element parsing with PropertyContainer support"
```

---

## Task 5: GameState board model (`state.rs`)

**Files:**
- Create: `src/state.rs`

**Reference:** `docs/specs/2026-03-18-rust-decoder-design.md` Section 2. `docs/superpowers/specs/2026-03-19-phase-bcd-updated-design.md` C1.

- [ ] **Step 1: Write tests for apply_elements**

In `src/state.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::statebuf::*;
    use crate::protocol::opcodes;
    use std::collections::HashMap;

    fn make_thing_props(thing_id: u32, zone: i32) -> HashMap<u32, PropertyValue> {
        let mut props = HashMap::new();
        let tn_key = opcodes::THINGNUMBER & opcodes::PROP_KEY_MASK;
        let zone_key = opcodes::ZONE & opcodes::PROP_KEY_MASK;
        props.insert(tn_key, PropertyValue::Int(thing_id as i32));
        props.insert(zone_key, PropertyValue::Int(zone));
        props
    }

    #[test]
    fn test_apply_thing_element() {
        let mut state = GameState::new(1);
        let elements = vec![
            StateElement::Thing(ThingElement {
                from_zone: 2,
                props: make_thing_props(100, 1), // zone=Hand
            }),
        ];
        state.apply_elements(&elements, false).unwrap();
        assert!(state.things.contains_key(&100));
        assert_eq!(state.things[&100].zone, 1);
    }

    #[test]
    fn test_apply_turn_step() {
        let mut state = GameState::new(1);
        let elements = vec![
            StateElement::TurnStep(TurnStepElement {
                turn_number: 3,
                phase: 4,
            }),
        ];
        state.apply_elements(&elements, false).unwrap();
        assert_eq!(state.turn, 3);
        assert_eq!(state.phase, 4);
    }

    #[test]
    fn test_full_state_pruning() {
        let mut state = GameState::new(1);
        // Add two things
        let elements = vec![
            StateElement::Thing(ThingElement {
                from_zone: 0,
                props: make_thing_props(100, 7),
            }),
            StateElement::Thing(ThingElement {
                from_zone: 0,
                props: make_thing_props(200, 7),
            }),
        ];
        state.apply_elements(&elements, false).unwrap();
        assert_eq!(state.things.len(), 2);

        // Full state with only thing 100
        let elements2 = vec![
            StateElement::Thing(ThingElement {
                from_zone: 0,
                props: make_thing_props(100, 7),
            }),
        ];
        state.apply_elements(&elements2, false).unwrap(); // is_diff=false → prune
        assert_eq!(state.things.len(), 1);
        assert!(state.things.contains_key(&100));
    }

    #[test]
    fn test_diff_no_pruning() {
        let mut state = GameState::new(1);
        let elements = vec![
            StateElement::Thing(ThingElement {
                from_zone: 0,
                props: make_thing_props(100, 7),
            }),
            StateElement::Thing(ThingElement {
                from_zone: 0,
                props: make_thing_props(200, 7),
            }),
        ];
        state.apply_elements(&elements, false).unwrap();

        // Diff update with only thing 100
        let elements2 = vec![
            StateElement::Thing(ThingElement {
                from_zone: 0,
                props: make_thing_props(100, 1), // zone changed
            }),
        ];
        state.apply_elements(&elements2, true).unwrap(); // is_diff=true → no prune
        assert_eq!(state.things.len(), 2); // 200 still present
    }

    #[test]
    fn test_upsert_retains_absent_properties() {
        let mut state = GameState::new(1);
        // First: thing with zone and tapped
        let mut props1 = make_thing_props(100, 7);
        let tapped_key = opcodes::TAPPED & opcodes::PROP_KEY_MASK;
        props1.insert(tapped_key, PropertyValue::Int(1));
        state.apply_elements(&[StateElement::Thing(ThingElement {
            from_zone: 0,
            props: props1,
        })], false).unwrap();

        assert!(state.things[&100].tapped);

        // Update: only zone changes, tapped not in props
        let props2 = make_thing_props(100, 1);
        state.apply_elements(&[StateElement::Thing(ThingElement {
            from_zone: 7,
            props: props2,
        })], true).unwrap();

        // tapped should still be true
        assert!(state.things[&100].tapped);
        assert_eq!(state.things[&100].zone, 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib state::tests -v 2>&1`
Expected: FAIL

- [ ] **Step 3: Implement GameState**

In `src/state.rs`:

```rust
//! Game state board model.
//!
//! Updated from StateBuf elements. Internal and transient — never persisted.

use std::collections::{HashMap, HashSet};

use crate::protocol::opcodes;
use crate::protocol::statebuf::{StateElement, ThingElement, PlayerStatusElement, TurnStepElement, PropertyValue};
use crate::protocol::DecodeError;

#[derive(Debug, Clone)]
pub struct GameState {
    pub game_id: u32,
    pub turn: i32,
    pub phase: u8,
    pub active_player: usize,
    pub players: Vec<PlayerState>,
    pub things: HashMap<u32, ThingState>,
}

#[derive(Debug, Clone)]
pub struct PlayerState {
    pub seat: usize,
    pub life: i32,
    pub hand_count: i32,
    pub library_count: i32,
    pub graveyard_count: i32,
}

#[derive(Debug, Clone)]
pub struct ThingState {
    pub thing_id: u32,
    pub zone: i32,
    pub from_zone: Option<i32>,
    pub controller: usize,
    pub owner: usize,
    pub card_texture_id: Option<u32>,
    pub card_name: Option<String>,
    pub tapped: bool,
    pub power: Option<i32>,
    pub toughness: Option<i32>,
    pub damage: i32,
    pub summoning_sickness: bool,
    pub face_down: bool,
    pub attached_to_id: Option<u32>,
    pub plus_counters: i32,
    pub minus_counters: i32,
    pub loyalty: i32,
    pub attacking: bool,
    pub blocking: bool,
    pub is_token: bool,
    pub src_thing_id: Option<u32>,
}

impl ThingState {
    pub fn new(thing_id: u32) -> Self {
        Self {
            thing_id,
            zone: 0,
            from_zone: None,
            controller: 0,
            owner: 0,
            card_texture_id: None,
            card_name: None,
            tapped: false,
            power: None,
            toughness: None,
            damage: 0,
            summoning_sickness: false,
            face_down: false,
            attached_to_id: None,
            plus_counters: 0,
            minus_counters: 0,
            loyalty: 0,
            attacking: false,
            blocking: false,
            is_token: false,
            src_thing_id: None,
        }
    }

    fn apply_props(&mut self, props: &HashMap<u32, PropertyValue>, from_zone: i32) {
        self.from_zone = Some(from_zone);

        for (&key, value) in props {
            let raw_key = key; // already masked to PROP_KEY_MASK
            let int_val = match value {
                PropertyValue::Int(v) => Some(*v),
                _ => None,
            };

            // Match against known property keys (masked values)
            let tn_key = opcodes::THINGNUMBER & opcodes::PROP_KEY_MASK;
            let zone_key = opcodes::ZONE & opcodes::PROP_KEY_MASK;
            let controller_key = opcodes::CONTROLLER & opcodes::PROP_KEY_MASK;
            let owner_key = opcodes::OWNER & opcodes::PROP_KEY_MASK;
            let texture_key = opcodes::CARDTEXTURE_NUMBER & opcodes::PROP_KEY_MASK;
            let name_key = opcodes::CARDNAME_STRING & opcodes::PROP_KEY_MASK;
            let tapped_key = opcodes::TAPPED & opcodes::PROP_KEY_MASK;
            let attacking_key = opcodes::ATTACKING & opcodes::PROP_KEY_MASK;
            let blocking_key = opcodes::BLOCKING & opcodes::PROP_KEY_MASK;
            let power_key = opcodes::POWER & opcodes::PROP_KEY_MASK;
            let toughness_key = opcodes::TOUGHNESS & opcodes::PROP_KEY_MASK;
            let damage_key = opcodes::DAMAGE & opcodes::PROP_KEY_MASK;
            let ss_key = opcodes::SUMMONING_SICK & opcodes::PROP_KEY_MASK;
            let fd_key = opcodes::FACE_DOWN & opcodes::PROP_KEY_MASK;
            let attached_key = opcodes::ATTACHED_TO_ID & opcodes::PROP_KEY_MASK;
            let token_key = opcodes::IS_TOKEN & opcodes::PROP_KEY_MASK;
            let plus_key = opcodes::PLUS_ONE_PLUS_ONE_COUNTERS & opcodes::PROP_KEY_MASK;
            let minus_key = opcodes::MINUS_ONE_MINUS_ONE_COUNTERS & opcodes::PROP_KEY_MASK;
            let loyalty_key = opcodes::LOYALTY_COUNTERS & opcodes::PROP_KEY_MASK;
            let src_key = opcodes::SRC_THING_ID & opcodes::PROP_KEY_MASK;

            if raw_key == zone_key { if let Some(v) = int_val { self.zone = v; } }
            else if raw_key == controller_key { if let Some(v) = int_val { self.controller = v as usize; } }
            else if raw_key == owner_key { if let Some(v) = int_val { self.owner = v as usize; } }
            else if raw_key == texture_key { if let Some(v) = int_val { self.card_texture_id = Some(v as u32); } }
            else if raw_key == name_key {
                if let PropertyValue::Str(s) = value { self.card_name = Some(s.clone()); }
            }
            else if raw_key == tapped_key { if let Some(v) = int_val { self.tapped = v != 0; } }
            else if raw_key == attacking_key { if let Some(v) = int_val { self.attacking = v != 0; } }
            else if raw_key == blocking_key { if let Some(v) = int_val { self.blocking = v != 0; } }
            else if raw_key == power_key { if let Some(v) = int_val { self.power = Some(v); } }
            else if raw_key == toughness_key { if let Some(v) = int_val { self.toughness = Some(v); } }
            else if raw_key == damage_key { if let Some(v) = int_val { self.damage = v; } }
            else if raw_key == ss_key { if let Some(v) = int_val { self.summoning_sickness = v != 0; } }
            else if raw_key == fd_key { if let Some(v) = int_val { self.face_down = v != 0; } }
            else if raw_key == attached_key {
                if let Some(v) = int_val {
                    self.attached_to_id = if v == 0 { None } else { Some(v as u32) };
                }
            }
            else if raw_key == token_key { if let Some(v) = int_val { self.is_token = v != 0; } }
            else if raw_key == plus_key { if let Some(v) = int_val { self.plus_counters = v; } }
            else if raw_key == minus_key { if let Some(v) = int_val { self.minus_counters = v; } }
            else if raw_key == loyalty_key { if let Some(v) = int_val { self.loyalty = v; } }
            else if raw_key == src_key { if let Some(v) = int_val { self.src_thing_id = Some(v as u32); } }
            // tn_key (THINGNUMBER) is handled by the caller — not stored as a field update
        }
    }
}

impl GameState {
    pub fn new(game_id: u32) -> Self {
        Self {
            game_id,
            turn: 0,
            phase: 0,
            active_player: 0,
            players: Vec::new(),
            things: HashMap::new(),
        }
    }

    /// Apply state elements. `is_diff` controls whether absent things are pruned.
    pub fn apply_elements(&mut self, elements: &[StateElement], is_diff: bool) -> Result<(), DecodeError> {
        let mut seen_things = HashSet::new();

        for element in elements {
            match element {
                StateElement::Thing(thing) => {
                    let tn_key = opcodes::THINGNUMBER & opcodes::PROP_KEY_MASK;
                    if let Some(PropertyValue::Int(id)) = thing.props.get(&tn_key) {
                        let thing_id = *id as u32;
                        seen_things.insert(thing_id);

                        let state = self.things.entry(thing_id).or_insert_with(|| ThingState::new(thing_id));
                        state.apply_props(&thing.props, thing.from_zone);
                    }
                }
                StateElement::TurnStep(ts) => {
                    self.turn = ts.turn_number;
                    self.phase = ts.phase;
                }
                StateElement::PlayerStatus(ps) => {
                    // Resize players vec if needed
                    let max_seats = ps.life.len()
                        .max(ps.hand_count.len())
                        .max(ps.library_count.len())
                        .max(ps.graveyard_count.len());
                    while self.players.len() < max_seats {
                        let seat = self.players.len();
                        self.players.push(PlayerState {
                            seat,
                            life: 20,
                            hand_count: 0,
                            library_count: 0,
                            graveyard_count: 0,
                        });
                    }
                    for (i, player) in self.players.iter_mut().enumerate() {
                        if let Some(&v) = ps.life.get(i) { player.life = v as i32; }
                        if let Some(&v) = ps.hand_count.get(i) { player.hand_count = v as i32; }
                        if let Some(&v) = ps.library_count.get(i) { player.library_count = v as i32; }
                        if let Some(&v) = ps.graveyard_count.get(i) { player.graveyard_count = v as i32; }
                    }
                    self.active_player = ps.active_player as usize;
                }
                StateElement::Other { .. } => {
                    // Skip unknown element types
                }
            }
        }

        // Full-state pruning: remove things not seen in this update
        if !is_diff {
            self.things.retain(|id, _| seen_things.contains(id));
        }

        Ok(())
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib state::tests -v 2>&1`
Expected: all 5 tests PASS

- [ ] **Step 5: Write golden file state sanity test**

Add to tests:

```rust
    #[test]
    fn test_golden_file_state_sanity() {
        use crate::protocol::fls;
        use crate::protocol::game_messages;
        use crate::protocol::statebuf::StateBufProcessor;

        let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
        let messages = crate::protocol::framing::parse_messages(&data).unwrap();

        let mut processor = StateBufProcessor::new();
        let mut state = GameState::new(0);
        let mut update_count = 0;

        for msg in &messages {
            if msg.opcode != 1153 && msg.opcode != 1156 { continue; }
            let fls_msg = match fls::decode_fls(msg.clone()) { Ok(f) => f, Err(_) => continue };
            let meta = match fls_msg {
                fls::FlsMessage::GsMessage { game_id, meta } => {
                    if state.game_id == 0 {
                        if let Ok(gid) = u32::try_from(game_id) { state.game_id = gid; }
                    }
                    meta
                }
                _ => continue,
            };
            let game_msg = match game_messages::decode_game_message(&meta) { Ok(g) => g, Err(_) => continue };
            let ps = match game_msg {
                game_messages::GameMessage::GamePlayStatus(ps) => ps,
                _ => continue,
            };

            let is_diff = ps.flags & opcodes::FLAG_GAMESTATE_CONTAINS_DIFFS != 0;
            let assembled = match processor.process(&ps) { Ok(a) => a, Err(_) => continue };
            let elements = match crate::protocol::statebuf::parse_elements(&assembled) { Ok(e) => e, Err(_) => continue };

            state.apply_elements(&elements, is_diff).ok();
            update_count += 1;
        }

        eprintln!("State after {update_count} updates:");
        eprintln!("  game_id: {}", state.game_id);
        eprintln!("  turn: {}", state.turn);
        eprintln!("  phase: {}", state.phase);
        eprintln!("  players: {}", state.players.len());
        for (i, p) in state.players.iter().enumerate() {
            eprintln!("    player {i}: life={}", p.life);
        }
        eprintln!("  things: {}", state.things.len());

        assert!(update_count > 0, "expected state updates");
        assert!(state.things.len() > 0, "expected things in state");
        assert!(state.players.len() >= 2, "expected at least 2 players");
    }
```

- [ ] **Step 6: Run golden file state test**

Run: `cargo test --lib state::tests::test_golden_file_state_sanity -- --nocapture 2>&1`
Expected: PASS. Output should show 2 players with life totals, a reasonable number of things, and a turn > 0.

- [ ] **Step 7: Commit**

```bash
git add src/state.rs
git commit -m "feat: implement GameState board model with element application"
```

---

## Task 6: Schema updates (`schema.rs`)

**Files:**
- Modify: `src/replay/schema.rs`

- [ ] **Step 1: Add new ActionType variants**

Add after `TurnChange` variant in the `ActionType` enum in `src/replay/schema.rs`:

```rust
    TapPermanent { card_id: String },
    UntapPermanent { card_id: String },
    DamageMarked { card_id: String, damage: i32 },
    SummoningSickness { card_id: String, has_sickness: bool },
    FaceDown { card_id: String },
    FaceUp { card_id: String },
    Attach { card_id: String, attached_to_id: String },
    Detach { card_id: String },
    CounterUpdate { card_id: String, counter_type: String, count: i32 },
    PowerToughnessUpdate { card_id: String, power: i32, toughness: i32 },
```

- [ ] **Step 2: Update the `test_action_types` test**

Add the new variants to the test vector in `src/replay/schema.rs`:

```rust
            ActionType::TapPermanent { card_id: "c1".to_string() },
            ActionType::UntapPermanent { card_id: "c1".to_string() },
            ActionType::DamageMarked { card_id: "c1".to_string(), damage: 3 },
            ActionType::SummoningSickness { card_id: "c1".to_string(), has_sickness: true },
            ActionType::FaceDown { card_id: "c1".to_string() },
            ActionType::FaceUp { card_id: "c1".to_string() },
            ActionType::Attach { card_id: "c1".to_string(), attached_to_id: "c2".to_string() },
            ActionType::Detach { card_id: "c1".to_string() },
            ActionType::CounterUpdate { card_id: "c1".to_string(), counter_type: "+1/+1".to_string(), count: 2 },
            ActionType::PowerToughnessUpdate { card_id: "c1".to_string(), power: 3, toughness: 4 },
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib replay::schema::tests -v 2>&1`
Expected: all PASS

- [ ] **Step 4: Commit**

```bash
git add src/replay/schema.rs
git commit -m "feat: add new ActionType variants for state-diff events"
```

---

## Task 7: ReplayTranslator (`translator.rs`)

**Files:**
- Create: `src/translator.rs`

**Reference:** `docs/superpowers/specs/2026-03-19-phase-bcd-updated-design.md` C3 — diff rules table.

This is the largest and most complex task. The translator diffs consecutive GameState snapshots to produce ReplayAction records.

- [ ] **Step 1: Write core diff tests**

In `src/translator.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{GameState, ThingState, PlayerState};
    use std::collections::HashMap;

    fn make_state(turn: i32, phase: u8) -> GameState {
        GameState {
            game_id: 1,
            turn,
            phase,
            active_player: 0,
            players: vec![
                PlayerState { seat: 0, life: 20, hand_count: 7, library_count: 53, graveyard_count: 0 },
                PlayerState { seat: 1, life: 20, hand_count: 7, library_count: 53, graveyard_count: 0 },
            ],
            things: HashMap::new(),
        }
    }

    fn make_thing(thing_id: u32, zone: i32) -> ThingState {
        let mut t = ThingState::new(thing_id);
        t.zone = zone;
        t
    }

    #[test]
    fn test_turn_change() {
        let mut translator = ReplayTranslator::new();
        let ts = Utc::now();
        let state1 = make_state(1, 1);
        let _ = translator.process(state1, ts);

        let state2 = make_state(2, 1);
        let actions = translator.process(state2, ts);

        assert!(actions.iter().any(|a| matches!(&a.action_type, ActionType::TurnChange { turn: 2, .. })));
    }

    #[test]
    fn test_phase_change() {
        let mut translator = ReplayTranslator::new();
        let ts = Utc::now();
        let state1 = make_state(1, 4); // PreCombatMain
        let _ = translator.process(state1, ts);

        let state2 = make_state(1, 5); // BeginCombat
        let actions = translator.process(state2, ts);

        assert!(actions.iter().any(|a| matches!(&a.action_type, ActionType::PhaseChange { .. })));
    }

    #[test]
    fn test_life_change() {
        let mut translator = ReplayTranslator::new();
        let ts = Utc::now();
        let state1 = make_state(1, 4);
        let _ = translator.process(state1, ts);

        let mut state2 = make_state(1, 4);
        state2.players[1].life = 17;
        let actions = translator.process(state2, ts);

        assert!(actions.iter().any(|a| matches!(&a.action_type,
            ActionType::LifeChange { old_life: 20, new_life: 17, .. }
        )));
    }

    #[test]
    fn test_draw_card() {
        let mut translator = ReplayTranslator::new();
        let ts = Utc::now();
        let state1 = make_state(1, 3); // Draw phase
        let _ = translator.process(state1, ts);

        let mut state2 = make_state(1, 3);
        let mut thing = make_thing(100, 1); // zone=Hand
        thing.from_zone = Some(2); // from Library
        state2.things.insert(100, thing);
        let actions = translator.process(state2, ts);

        assert!(actions.iter().any(|a| matches!(&a.action_type, ActionType::DrawCard { .. })));
    }

    #[test]
    fn test_play_land() {
        let mut translator = ReplayTranslator::new();
        let ts = Utc::now();

        // State 1: thing in hand
        let mut state1 = make_state(1, 4);
        let mut thing = make_thing(100, 1); // Hand
        thing.from_zone = None;
        state1.things.insert(100, thing);
        let _ = translator.process(state1, ts);

        // State 2: thing on battlefield (never seen on stack)
        let mut state2 = make_state(1, 4);
        let mut thing2 = make_thing(100, 7); // Battlefield
        thing2.from_zone = Some(1); // from Hand
        state2.things.insert(100, thing2);
        let actions = translator.process(state2, ts);

        assert!(actions.iter().any(|a| matches!(&a.action_type, ActionType::PlayLand { .. })));
    }

    #[test]
    fn test_cast_spell() {
        let mut translator = ReplayTranslator::new();
        let ts = Utc::now();
        let state1 = make_state(1, 4);
        let _ = translator.process(state1, ts);

        // New thing appears on Stack
        let mut state2 = make_state(1, 4);
        let mut thing = make_thing(100, 8); // Stack
        thing.from_zone = Some(1); // from Hand
        state2.things.insert(100, thing);
        let actions = translator.process(state2, ts);

        assert!(actions.iter().any(|a| matches!(&a.action_type, ActionType::CastSpell { .. })));
    }

    #[test]
    fn test_tap_untap() {
        let mut translator = ReplayTranslator::new();
        let ts = Utc::now();

        let mut state1 = make_state(1, 4);
        let mut thing = make_thing(100, 7);
        thing.tapped = false;
        state1.things.insert(100, thing);
        let _ = translator.process(state1, ts);

        let mut state2 = make_state(1, 4);
        let mut thing2 = make_thing(100, 7);
        thing2.tapped = true;
        state2.things.insert(100, thing2);
        let actions = translator.process(state2, ts);

        assert!(actions.iter().any(|a| matches!(&a.action_type, ActionType::TapPermanent { .. })));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib translator::tests -v 2>&1`
Expected: FAIL

- [ ] **Step 3: Implement ReplayTranslator**

In `src/translator.rs`:

```rust
//! State-diff to ReplayAction translation.
//!
//! Diffs consecutive GameState snapshots to produce an action stream.

use std::collections::HashSet;

use chrono::{DateTime, Utc};

use crate::protocol::opcodes;
use crate::replay::schema::{ActionType, ReplayAction, ReplayHeader, ReplayFile, PlayerInfo, GameResult};
use crate::state::{GameState, ThingState};

pub struct ReplayTranslator {
    prev: Option<GameState>,
    player_names: Vec<String>,
    start_time: Option<DateTime<Utc>>,
    things_seen_on_stack: HashSet<u32>,
}

impl ReplayTranslator {
    pub fn new() -> Self {
        Self {
            prev: None,
            player_names: Vec::new(),
            start_time: None,
            things_seen_on_stack: HashSet::new(),
        }
    }

    pub fn set_player_order(&mut self, names: Vec<String>) {
        self.player_names = names;
    }

    fn player_name(&self, seat: usize) -> String {
        self.player_names
            .get(seat)
            .cloned()
            .unwrap_or_else(|| format!("player_{seat}"))
    }

    fn phase_name(phase: u8) -> &'static str {
        match phase {
            0 => "invalid",
            1 => "untap",
            2 => "upkeep",
            3 => "draw",
            4 => "precombat_main",
            5 => "begin_combat",
            6 => "declare_attackers",
            7 => "declare_blockers",
            8 => "combat_damage",
            9 => "end_of_combat",
            10 => "postcombat_main",
            11 => "end_of_turn",
            12 => "cleanup",
            _ => "unknown",
        }
    }

    pub fn process(&mut self, new_state: GameState, ts: DateTime<Utc>) -> Vec<ReplayAction> {
        if self.start_time.is_none() {
            self.start_time = Some(ts);
        }

        // Update things_seen_on_stack before evaluating transitions
        for (&thing_id, thing) in &new_state.things {
            if thing.zone == 8 { // Stack
                self.things_seen_on_stack.insert(thing_id);
            }
        }

        let mut actions = Vec::new();

        if let Some(ref prev) = self.prev {
            let turn = new_state.turn;
            let phase = new_state.phase;
            let active = self.player_name(new_state.active_player);

            // Turn change
            if new_state.turn > prev.turn {
                actions.push(make_action(ts, turn, phase, &active, ActionType::TurnChange {
                    turn: new_state.turn,
                    player_id: active.clone(),
                }));
            }

            // Phase change
            if new_state.phase != prev.phase {
                actions.push(make_action(ts, turn, phase, &active, ActionType::PhaseChange {
                    phase: Self::phase_name(new_state.phase).to_string(),
                }));
            }

            // Life changes
            for (i, player) in new_state.players.iter().enumerate() {
                if let Some(prev_player) = prev.players.get(i) {
                    if player.life != prev_player.life {
                        actions.push(make_action(ts, turn, phase, &active, ActionType::LifeChange {
                            player_id: self.player_name(i),
                            old_life: prev_player.life,
                            new_life: player.life,
                        }));
                    }
                }
            }

            // Thing changes
            for (&thing_id, thing) in &new_state.things {
                let tid = thing_id.to_string();
                let prev_thing = prev.things.get(&thing_id);

                if prev_thing.is_none() {
                    // New thing appeared
                    self.emit_new_thing(&mut actions, ts, turn, phase, &active, thing, &tid, &new_state);
                } else {
                    let pt = prev_thing.unwrap();
                    // Zone change
                    if thing.zone != pt.zone {
                        self.emit_zone_change(&mut actions, ts, turn, phase, &active, thing, pt, &tid);
                    }
                    // Property changes
                    self.emit_property_changes(&mut actions, ts, turn, phase, &active, thing, pt, &tid);
                }
            }
        }

        // Clear from_zone on all things (consumed by this process call)
        let mut owned_state = new_state;
        for thing in owned_state.things.values_mut() {
            thing.from_zone = None;
        }
        self.prev = Some(owned_state);

        actions
    }

    fn emit_new_thing(
        &self,
        actions: &mut Vec<ReplayAction>,
        ts: DateTime<Utc>,
        turn: i32,
        phase: u8,
        active: &str,
        thing: &ThingState,
        tid: &str,
        new_state: &GameState,
    ) {
        let from = thing.from_zone.unwrap_or(0);

        // DrawCard: from Library(2) to Hand(1)
        if from == 2 && thing.zone == 1 {
            actions.push(make_action(ts, turn, phase, active, ActionType::DrawCard {
                player_id: self.player_name(thing.owner),
                card_id: tid.to_string(),
            }));
            return;
        }

        // Thing appeared on Stack(8)
        if thing.zone == 8 {
            // Check ActivateAbility: SRC_THING_ID points to a battlefield permanent
            if let Some(src_id) = thing.src_thing_id {
                if let Some(src_thing) = new_state.things.get(&src_id) {
                    if src_thing.zone == 7 { // Battlefield
                        actions.push(make_action(ts, turn, phase, active, ActionType::ActivateAbility {
                            player_id: self.player_name(thing.controller),
                            card_id: src_id.to_string(),
                            ability_id: tid.to_string(),
                        }));
                        return;
                    }
                }
            }
            // Otherwise: CastSpell
            actions.push(make_action(ts, turn, phase, active, ActionType::CastSpell {
                player_id: self.player_name(thing.controller),
                card_id: tid.to_string(),
            }));
            return;
        }

        // PlayLand: Hand(1) → Battlefield(7), never seen on stack
        if from == 1 && thing.zone == 7 && !self.things_seen_on_stack.contains(&thing.thing_id) {
            actions.push(make_action(ts, turn, phase, active, ActionType::PlayLand {
                player_id: self.player_name(thing.controller),
                card_id: tid.to_string(),
            }));
            return;
        }

        // Generic zone transition for any other new thing
        if from != 0 && from != thing.zone {
            actions.push(make_action(ts, turn, phase, active, ActionType::ZoneTransition {
                card_id: tid.to_string(),
                from_zone: zone_name(from).to_string(),
                to_zone: zone_name(thing.zone).to_string(),
                player_id: Some(self.player_name(thing.controller)),
            }));
        }
    }

    fn emit_zone_change(
        &self,
        actions: &mut Vec<ReplayAction>,
        ts: DateTime<Utc>,
        turn: i32,
        phase: u8,
        active: &str,
        thing: &ThingState,
        prev_thing: &ThingState,
        tid: &str,
    ) {
        let from = prev_thing.zone;
        let to = thing.zone;

        // Hand → Battlefield, never on stack → PlayLand
        if from == 1 && to == 7 && !self.things_seen_on_stack.contains(&thing.thing_id) {
            actions.push(make_action(ts, turn, phase, active, ActionType::PlayLand {
                player_id: self.player_name(thing.controller),
                card_id: tid.to_string(),
            }));
            return;
        }

        // Stack → Battlefield → ZoneTransition
        if from == 8 && to == 7 {
            actions.push(make_action(ts, turn, phase, active, ActionType::ZoneTransition {
                card_id: tid.to_string(),
                from_zone: "stack".to_string(),
                to_zone: "battlefield".to_string(),
                player_id: Some(self.player_name(thing.controller)),
            }));
            return;
        }

        // Stack → non-Battlefield → Resolve
        if from == 8 {
            actions.push(make_action(ts, turn, phase, active, ActionType::Resolve {
                card_id: tid.to_string(),
            }));
            return;
        }

        // Generic zone transition
        actions.push(make_action(ts, turn, phase, active, ActionType::ZoneTransition {
            card_id: tid.to_string(),
            from_zone: zone_name(from).to_string(),
            to_zone: zone_name(to).to_string(),
            player_id: Some(self.player_name(thing.controller)),
        }));
    }

    fn emit_property_changes(
        &self,
        actions: &mut Vec<ReplayAction>,
        ts: DateTime<Utc>,
        turn: i32,
        phase: u8,
        active: &str,
        thing: &ThingState,
        prev: &ThingState,
        tid: &str,
    ) {
        if thing.attacking && !prev.attacking {
            let opponent = if thing.controller == 0 { 1 } else { 0 };
            actions.push(make_action(ts, turn, phase, active, ActionType::Attack {
                attacker_id: tid.to_string(),
                defender_id: self.player_name(opponent),
            }));
        }
        if thing.blocking && !prev.blocking {
            actions.push(make_action(ts, turn, phase, active, ActionType::Block {
                attacker_id: "unknown".to_string(),
                blocker_id: tid.to_string(),
            }));
        }
        if thing.tapped != prev.tapped {
            if thing.tapped {
                actions.push(make_action(ts, turn, phase, active, ActionType::TapPermanent { card_id: tid.to_string() }));
            } else {
                actions.push(make_action(ts, turn, phase, active, ActionType::UntapPermanent { card_id: tid.to_string() }));
            }
        }
        if thing.damage != prev.damage {
            actions.push(make_action(ts, turn, phase, active, ActionType::DamageMarked { card_id: tid.to_string(), damage: thing.damage }));
        }
        if thing.summoning_sickness != prev.summoning_sickness {
            actions.push(make_action(ts, turn, phase, active, ActionType::SummoningSickness { card_id: tid.to_string(), has_sickness: thing.summoning_sickness }));
        }
        if thing.face_down != prev.face_down {
            if thing.face_down {
                actions.push(make_action(ts, turn, phase, active, ActionType::FaceDown { card_id: tid.to_string() }));
            } else {
                actions.push(make_action(ts, turn, phase, active, ActionType::FaceUp { card_id: tid.to_string() }));
            }
        }
        if thing.attached_to_id != prev.attached_to_id {
            if let Some(target) = thing.attached_to_id {
                actions.push(make_action(ts, turn, phase, active, ActionType::Attach {
                    card_id: tid.to_string(),
                    attached_to_id: target.to_string(),
                }));
            } else {
                actions.push(make_action(ts, turn, phase, active, ActionType::Detach { card_id: tid.to_string() }));
            }
        }
        if thing.plus_counters != prev.plus_counters {
            actions.push(make_action(ts, turn, phase, active, ActionType::CounterUpdate {
                card_id: tid.to_string(), counter_type: "+1/+1".to_string(), count: thing.plus_counters,
            }));
        }
        if thing.minus_counters != prev.minus_counters {
            actions.push(make_action(ts, turn, phase, active, ActionType::CounterUpdate {
                card_id: tid.to_string(), counter_type: "-1/-1".to_string(), count: thing.minus_counters,
            }));
        }
        if thing.loyalty != prev.loyalty {
            actions.push(make_action(ts, turn, phase, active, ActionType::CounterUpdate {
                card_id: tid.to_string(), counter_type: "loyalty".to_string(), count: thing.loyalty,
            }));
        }
        if thing.power != prev.power || thing.toughness != prev.toughness {
            if thing.power.is_some() || thing.toughness.is_some() {
                actions.push(make_action(ts, turn, phase, active, ActionType::PowerToughnessUpdate {
                    card_id: tid.to_string(),
                    power: thing.power.unwrap_or(0),
                    toughness: thing.toughness.unwrap_or(0),
                }));
            }
        }
    }

    pub fn finish(self, result: GameResult, end_time: DateTime<Utc>) -> ReplayHeader {
        let players: Vec<PlayerInfo> = if let Some(ref prev) = self.prev {
            prev.players.iter().enumerate().map(|(i, p)| PlayerInfo {
                player_id: self.player_name(i),
                name: self.player_name(i),
                life_total: p.life,
            }).collect()
        } else {
            Vec::new()
        };

        ReplayHeader {
            game_id: self.prev.as_ref().map(|s| s.game_id.to_string()).unwrap_or_default(),
            players,
            format: "Unknown".to_string(),
            start_time: self.start_time.unwrap_or_else(Utc::now),
            end_time: Some(end_time),
            result,
        }
    }

    pub fn reset(&mut self) {
        self.prev = None;
        self.start_time = None;
        self.player_names.clear();
        self.things_seen_on_stack.clear();
    }
}

fn make_action(ts: DateTime<Utc>, turn: i32, phase: u8, active: &str, action_type: ActionType) -> ReplayAction {
    ReplayAction {
        timestamp: ts,
        turn,
        phase: ReplayTranslator::phase_name(phase).to_string(),
        active_player: active.to_string(),
        action_type,
    }
}

fn zone_name(zone: i32) -> &'static str {
    match zone {
        0 => "invalid",
        1 => "hand",
        2 => "library",
        3 => "graveyard",
        4 => "exile",
        7 => "battlefield",
        8 => "stack",
        9 => "command",
        21 => "sideboard",
        _ => "other",
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib translator::tests -v 2>&1`
Expected: all 7 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/translator.rs
git commit -m "feat: implement ReplayTranslator with state-diff action emission"
```

---

## Task 8: End-to-end pipeline and golden file test

**Files:**
- Modify: `src/bin/decode.rs`

- [ ] **Step 1: Update `bin/decode.rs` to run the full pipeline**

Replace `src/bin/decode.rs` with:

```rust
//! CLI tool: decode a captured MTGO stream file into replay JSON.

use std::env;
use std::fs;
use std::collections::HashMap;

use chrono::Utc;
use flashback::protocol::framing;
use flashback::protocol::fls::{self, FlsMessage};
use flashback::protocol::game_messages::{self, GameMessage};
use flashback::protocol::statebuf::{self, StateBufProcessor, parse_elements};
use flashback::protocol::opcodes;
use flashback::state::GameState;
use flashback::translator::ReplayTranslator;
use flashback::replay::schema::{GameResult, ReplayFile};

fn main() {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: decode <stream_file> [--json]");
        std::process::exit(1);
    }

    let path = &args[1];
    let json_output = args.iter().any(|a| a == "--json");

    let data = fs::read(path).unwrap_or_else(|e| {
        eprintln!("Failed to read {path}: {e}");
        std::process::exit(1);
    });

    eprintln!("Read {} bytes from {path}", data.len());

    let messages = framing::parse_messages(&data).unwrap_or_else(|e| {
        eprintln!("Framing error: {e}");
        std::process::exit(1);
    });
    eprintln!("Parsed {} messages", messages.len());

    let mut processor = StateBufProcessor::new();
    let mut state = GameState::new(0);
    let mut translator = ReplayTranslator::new();
    let mut all_actions = Vec::new();
    let mut stats = PipelineStats::default();
    let now = Utc::now();

    for msg in &messages {
        let fls_msg = match fls::decode_fls(msg.clone()) {
            Ok(m) => m,
            Err(e) => { stats.fls_errors += 1; tracing::warn!("FLS decode error: {e}"); continue; }
        };

        match &fls_msg {
            FlsMessage::GsMessage { game_id, meta } => {
                stats.gs_messages += 1;
                if state.game_id == 0 {
                    if let Ok(gid) = u32::try_from(*game_id) {
                        state = GameState::new(gid);
                    }
                }

                let game_msg = match game_messages::decode_game_message(meta) {
                    Ok(m) => m,
                    Err(e) => { stats.game_errors += 1; tracing::warn!("Game decode error: {e}"); continue; }
                };

                match game_msg {
                    GameMessage::GamePlayStatus(ps) => {
                        stats.play_status += 1;
                        let is_diff = ps.flags & opcodes::FLAG_GAMESTATE_CONTAINS_DIFFS != 0;

                        let assembled = match processor.process(&ps) {
                            Ok(Some(a)) => a,
                            Ok(None) => { stats.pending_chunks += 1; continue; }
                            Err(e) => { stats.assembly_errors += 1; tracing::warn!("Assembly error: {e}"); continue; }
                        };

                        let elements = match parse_elements(&assembled) {
                            Ok(e) => e,
                            Err(e) => { stats.element_errors += 1; tracing::warn!("Element error: {e}"); continue; }
                        };

                        if let Err(e) = state.apply_elements(&elements, is_diff) {
                            stats.state_errors += 1;
                            tracing::warn!("State error: {e}");
                            continue;
                        }

                        let actions = translator.process(state.clone(), now);
                        stats.actions += actions.len();
                        all_actions.extend(actions);
                    }
                    GameMessage::GameOver => {
                        stats.game_over += 1;
                    }
                    GameMessage::Other { opcode } => {
                        stats.other_game += 1;
                        let _ = opcode;
                    }
                }
            }
            FlsMessage::GameStatusChange { new_status } => {
                stats.status_changes += 1;
                if *new_status == 5 || *new_status == 7 {
                    // Game terminated or completed — reset for next game
                    processor.reset();
                    translator.reset();
                }
            }
            _ => { stats.other_fls += 1; }
        }
    }

    eprintln!("\nPipeline stats:");
    eprintln!("  GsMessages: {}", stats.gs_messages);
    eprintln!("  GamePlayStatus: {}", stats.play_status);
    eprintln!("  Actions emitted: {}", stats.actions);
    eprintln!("  GameOver: {}", stats.game_over);
    eprintln!("  Status changes: {}", stats.status_changes);
    eprintln!("  Errors: fls={} game={} assembly={} element={} state={}",
        stats.fls_errors, stats.game_errors, stats.assembly_errors,
        stats.element_errors, stats.state_errors);

    if json_output {
        let header = translator.finish(GameResult::Incomplete, now);
        let replay = ReplayFile {
            header,
            actions: all_actions,
            metadata: {
                let mut m = HashMap::new();
                m.insert("version".to_string(), "1.0".to_string());
                m.insert("recorder".to_string(), "flashback".to_string());
                m
            },
        };
        let json = serde_json::to_string_pretty(&replay).unwrap();
        println!("{json}");
    } else {
        eprintln!("\nFirst 20 actions:");
        for (i, action) in all_actions.iter().take(20).enumerate() {
            eprintln!("  {i}: turn={} phase={} {:?}", action.turn, action.phase, action.action_type);
        }
        eprintln!("\nTotal actions: {}", all_actions.len());
    }
}

#[derive(Default)]
struct PipelineStats {
    gs_messages: usize,
    play_status: usize,
    actions: usize,
    game_over: usize,
    status_changes: usize,
    other_fls: usize,
    other_game: usize,
    fls_errors: usize,
    game_errors: usize,
    assembly_errors: usize,
    element_errors: usize,
    state_errors: usize,
    pending_chunks: usize,
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check --bin decode 2>&1`
Expected: compiles with no errors

- [ ] **Step 3: Run against golden file**

Run: `cargo run --bin decode -- tests/fixtures/golden_v1.bin 2>&1`
Expected: output showing pipeline stats — GsMessages ~1202, some GamePlayStatus messages parsed, actions emitted > 0, zero or near-zero errors. Inspect the first 20 actions for sanity (turn changes, phase changes, draws, etc.).

- [ ] **Step 4: Generate JSON fixture for regression**

Run: `cargo run --bin decode -- tests/fixtures/golden_v1.bin --json > tests/fixtures/golden_v1_replay.json 2>/dev/null`

Inspect the output:
Run: `head -50 tests/fixtures/golden_v1_replay.json`
Expected: valid JSON with header (game_id, players, format) and actions array.

- [ ] **Step 5: Commit**

```bash
git add src/bin/decode.rs tests/fixtures/golden_v1_replay.json
git commit -m "feat: full decode pipeline with golden file JSON output"
```

---

## Task 9: Regression test against golden JSON fixture

**Files:**
- Create: `tests/pipeline.rs`

- [ ] **Step 1: Write regression test**

Create `tests/pipeline.rs`:

```rust
//! End-to-end pipeline regression test against golden file.

use flashback::protocol::{framing, fls, game_messages, statebuf, opcodes};
use flashback::protocol::fls::FlsMessage;
use flashback::protocol::game_messages::GameMessage;
use flashback::protocol::statebuf::{StateBufProcessor, parse_elements};
use flashback::state::GameState;
use flashback::translator::ReplayTranslator;
use flashback::replay::schema::GameResult;
use chrono::Utc;

#[test]
fn test_golden_file_full_pipeline() {
    let data = std::fs::read("tests/fixtures/golden_v1.bin").unwrap();
    let messages = framing::parse_messages(&data).unwrap();

    let mut processor = StateBufProcessor::new();
    let mut state = GameState::new(0);
    let mut translator = ReplayTranslator::new();
    let mut all_actions = Vec::new();
    let now = Utc::now();
    let mut errors = 0;

    for msg in &messages {
        let fls_msg = match fls::decode_fls(msg.clone()) { Ok(m) => m, Err(_) => { errors += 1; continue; } };

        match &fls_msg {
            FlsMessage::GsMessage { game_id, meta } => {
                if state.game_id == 0 {
                    if let Ok(gid) = u32::try_from(*game_id) {
                        state = GameState::new(gid);
                    }
                }
                let game_msg = match game_messages::decode_game_message(meta) { Ok(m) => m, Err(_) => { errors += 1; continue; } };
                if let GameMessage::GamePlayStatus(ps) = game_msg {
                    let is_diff = ps.flags & opcodes::FLAG_GAMESTATE_CONTAINS_DIFFS != 0;
                    let assembled = match processor.process(&ps) { Ok(Some(a)) => a, Ok(None) => continue, Err(_) => { errors += 1; continue; } };
                    let elements = match parse_elements(&assembled) { Ok(e) => e, Err(_) => { errors += 1; continue; } };
                    state.apply_elements(&elements, is_diff).ok();
                    let actions = translator.process(state.clone(), now);
                    all_actions.extend(actions);
                }
            }
            FlsMessage::GameStatusChange { new_status } => {
                if *new_status == 5 || *new_status == 7 {
                    processor.reset();
                    translator.reset();
                }
            }
            _ => {}
        }
    }

    // Basic sanity checks
    assert_eq!(errors, 0, "pipeline should have zero errors");
    assert!(all_actions.len() > 10, "expected meaningful actions, got {}", all_actions.len());

    // Should contain recognizable game actions
    let has_turn = all_actions.iter().any(|a| matches!(&a.action_type, flashback::replay::schema::ActionType::TurnChange { .. }));
    let has_draw = all_actions.iter().any(|a| matches!(&a.action_type, flashback::replay::schema::ActionType::DrawCard { .. }));
    assert!(has_turn, "expected at least one TurnChange action");
    assert!(has_draw, "expected at least one DrawCard action");
}
```

- [ ] **Step 2: Run regression test**

Run: `cargo test --test pipeline test_golden_file_full_pipeline -- --nocapture 2>&1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add tests/pipeline.rs
git commit -m "test: add end-to-end pipeline regression test against golden file"
```
