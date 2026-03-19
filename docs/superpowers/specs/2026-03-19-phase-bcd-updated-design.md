# Updated Phase B/C/D Design

**Date:** 2026-03-19
**Status:** Approved
**Supersedes:** Implementation Phases section of `docs/specs/2026-03-18-rust-decoder-design.md`
**Context:** Phase A is complete. We have `golden_v1.bin` (12MB, 10,195 messages, 71 opcodes) and a working framing layer. This document updates the remaining phases based on findings from Phase A and analysis of existing code.

---

## Decisions Made

### Existing code disposition
- **Delete:** `decoder.rs`, `raw_analyzer.rs`, `writer.rs`, `boundary.rs`, `tests/integration.rs` — these are stubs built around a flat event stream model that doesn't match the MTGO protocol's state-snapshot architecture.
- **Keep:** `framing.rs` (done), `opcodes.rs` (extend), `schema.rs` (modify), `bin/decode.rs` (update), `main.rs` and `capture/*` (untouched).

### Architecture rationale
MTGO sends repeated board state snapshots (StateBuf), not discrete events. The decoder diffs consecutive snapshots to produce an action stream. The state model (`GameState`) is internal and transient — never persisted. The replay file stores the action stream (`Vec<ReplayAction>`), which is compact and suitable for playback.

### PlayLand and ActivateAbility are emittable
Contrary to the original spec's assertion:
- **PlayLand:** Lands go Hand(1)→Battlefield(7) without touching Stack(8). Spells go Hand→Stack first. This is distinguishable in state diffs.
- **ActivateAbility:** Activated abilities appear as new Things on Stack(8) with `SRC_THING_ID` pointing to the source permanent. This distinguishes them from cast spells (which come from Hand).

Both variants stay in the schema. `PassPriority` is retained in the schema for JSON deserialization compatibility but is not emitted by this decoder (client→server only, not visible in state snapshots).

### ActivateAbility detection detail
`SRC_THING_ID` must point to a `thing_id` that exists in the current `GameState.things` with `zone == Battlefield`. A bare `SRC_THING_ID` presence is not sufficient — the referenced thing must be a battlefield permanent to distinguish activated abilities from cast spells that happen to reference other things.

### Boundary signal opcodes
In addition to the three core FLS opcodes (1153 GsMessage, 1155 PlayerOrder, 1145 GameStatusChange), decode lightweight boundary signals from:
- 960 `FlsGameCreatedMessage` → `GameCreated { game_id }`
- 967 `FlsGameGameEndedMessage` → `GameEnded { game_id }`
- 2463 `FlsMatchGameStartedMessage` → `MatchStarted { game_id }`

These provide game boundary detection for multi-game sessions. Only the `game_id` field is extracted — no deep field parsing. Wire layouts for these messages are not yet documented; `game_id` byte offset within each payload must be determined during implementation (inspect golden file payloads or decompile). If the offset cannot be determined, decode as `Other` and rely on `GshGameStatusChangeMessage` (1145) for boundary detection.

### Known limitations (accepted, deferred)
1. **GsPlayerOrderMessage (1155):** Wire layout undocumented. Implemented as no-op that logs raw payload. Fallback player naming (`"player_0"`, `"player_1"`) keeps pipeline functional.
2. **GameResultsMessage (4485):** Wire layout undocumented. Decoded as `GameMessage::Other`, emits `GameResult::Incomplete`.
3. **String Table:** `length == 0xFFFF` strings stored as `"<strtable:N>"` placeholders. Card names may be affected; integer-keyed properties (zone, life, tapped) are not.
4. **MiniChange (StateElementType 200):** Payload format undocumented. Decoded as `StateElement::Other` and dropped. Risk: if the server uses MiniChange for life changes or zone transitions between full-state snapshots, those events will be silently missed. On full-state messages this should not affect correctness. On diff messages, log a warning if MiniChange elements appear and treat the state as suspect. To fix: decompile `MiniChangeElement.ReadBuffer()` from `WotC.MtGO.Client.Model.Play.dll`.

---

## Phase B: Decode Pipeline

Working against `golden_v1.bin`. Each step validates against the golden file before proceeding.

### B1: opcodes.rs updates
- Game-level opcode constants: 4652 GamePlayStatus, 4485 GameResults, 4632 GameOver, 4643 GameNextStep, 4645 NewCardAction, 4647 CardAction, 4689 V3ReplayUserOrder
- Boundary signal opcodes: 960, 967, 2463
- MagicProperty constants from the design spec (SRC_THING_ID and any not already present)
- StateElementType constants (0=Invalid, 2=PlayerStatus, 3=TurnStep, 5=Thing, 200=MiniChange)
- PropertyContainer type tags (0x20000000 Int8, 0x28000000 Int32, 0x40000000 String, 0x08000000 List, 0x10000000 Function, 0x48000000 StringConstant)
- Flag bit constants (GamestateContainsDiffs=0x01, GamestateHead=0x02, GamestateTail=0x04)
- Checksum seed: 826366246

### B2: fls.rs
Decode `FlsMessage` variants from `RawMessage`:

```rust
pub enum FlsMessage {
    GsMessage { game_id: i32, meta: Vec<u8> },
    GameStatusChange { new_status: u32 },
    PlayerOrder { raw: Vec<u8> },  // no-op, log only (wire layout TODO)
    GameCreated { game_id: i32 },
    GameEnded { game_id: i32 },
    MatchStarted { game_id: i32 },
    Other(RawMessage),
}
```

Both opcode 1153 (`GsMessageMessage`) and 1156 (`GsReplayMessageMessage`) map to `FlsMessage::GsMessage`. Their envelope metadata (`match_token`, `match_id`, `host_gsh_server_id`) is consumed off the wire but not stored.

`meta` is the raw `MetaMessage byte[]` — a complete embedded CSMessage with its own 8-byte header. `decode_game_message` in B3 calls `framing::read_message` on these bytes to extract the inner opcode and payload.

**Validation:** Extract 1,202 GsMessage payloads from golden file. All 10,195 messages parse without errors (most as `Other`).

### B3: game_messages.rs
Decode `GameMessage` from the `meta` bytes inside `GsMessage`:

```rust
pub enum GameMessage {
    GamePlayStatus(GamePlayStatusMessage),
    GameOver,
    Other { opcode: u16 },
}
```

Note: `GameResultsMessage` (opcode 4485) is decoded as `Other` until its wire layout is documented. Once decompiled, add a `GameResults { winner_seat: u32 }` variant.

`GamePlayStatusMessage` fields parsed in wire order: state_buf_raw, time_left, player_waiting_for, game_id, state_size, undiffed_buffer_size, n_state_elems, priority_player, checksum, last_state_checksum, flags, game_state_timestamp.

**Validation:** Every meta payload from golden file parses without errors.

### B4: statebuf.rs — assembly + diffs
`StateBufProcessor` with:
- Assembly buffer: Head/Tail chunking with edge case handling (Tail-without-Head, Head-discards-partial, single-chunk full state)
- `apply_diffs` implementing ApplyDiffs2 (5 opcodes: 0x00 copy, 0x80|0 long literal, 0x80|0+next medium copy, 0x80+ short copy, 0x01-0x7F literal)
- Checksum validation: seed 826366246, rolling `(checksum << 1) + byte`
- `previous_state` lifecycle: stored after successful process(), cleared on reset()

**Validation:** Every GamePlayStatusMessage in golden file assembles with valid checksums. Both full-state and diff messages validate.

### B5: statebuf.rs — elements
Parse `StateElement` variants from assembled buffers:

```rust
pub enum StateElement {
    PlayerStatus(PlayerStatusElement),
    TurnStep(TurnStepElement),
    Thing(ThingElement),
    Other { element_type: u32, raw: Vec<u8> },
}
```

`ThingElement` PropertyContainer parsing: type tag dispatch, key remapping for Int32, nested List recursion, StringConstant abort (store containing element as Other), null terminator. Things without THINGNUMBER are discarded with a warning.

**Validation:** Element counts match `n_state_elems` for every message in golden file.

### Phase B exit criterion
Every `GamePlayStatusMessage` in the golden file parses to `Vec<StateElement>` without errors. All checksums pass.

---

## Phase C: State & Translation

### C1: state.rs — GameState

**`game_id` type conversion:** `GsMessage.game_id` is `i32` on the wire; `GameState.game_id` is `u32`. Convert via `u32::try_from(game_id).map_err(...)` at the `apply_elements` call site — do not use `as u32`, which silently wraps negative values. If the value is negative, return `Err` (negative game IDs may be sentinel values).

```rust
pub struct GameState {
    pub game_id: u32,
    pub turn: i32,
    pub phase: GamePhase,
    pub active_player: usize,
    pub players: Vec<PlayerState>,
    pub things: HashMap<u32, ThingState>,
}
```

`apply_elements()`:
- Upsert Things by THINGNUMBER; only update fields whose properties are present
- Update PlayerStatus/TurnStep
- Full-state pruning: when `GamestateContainsDiffs` is clear, remove Things not seen in this update
- `from_zone` set on each update, cleared by translator after processing

**Validation:** Inspect resulting state for sane values — things exist, zones populated, life totals start near 20.

### C2: schema.rs updates
Add variants:
- `TapPermanent { card_id }`, `UntapPermanent { card_id }`
- `DamageMarked { card_id, damage }`
- `SummoningSickness { card_id, has_sickness }`
- `FaceDown { card_id }`, `FaceUp { card_id }`
- `Attach { card_id, attached_to_id }`, `Detach { card_id }`
- `CounterUpdate { card_id, counter_type, count }`
- `PowerToughnessUpdate { card_id, power, toughness }`

Retain but not emitted: `PassPriority` (kept for JSON deserialization compatibility with any previously-saved replay files)

Keep: `PlayLand`, `ActivateAbility` (both emittable — see Decisions above)

### C3: translator.rs — ReplayTranslator

```rust
pub struct ReplayTranslator {
    prev: Option<GameState>,
    player_names: Vec<String>,
    start_time: Option<DateTime<Utc>>,
    things_seen_on_stack: HashSet<u32>,  // tracks stack history for PlayLand detection
}
```

**`things_seen_on_stack` population:** During each `process()` call, before evaluating zone-transition rules, scan the new state for any thing with `zone == Stack(8)` and add its `thing_id` to `things_seen_on_stack`. This ensures that if a thing appears on the stack and resolves to the battlefield within the same state diff, the stack history is recorded before the PlayLand check runs.

Diff rules (emit order within a single `process()` call):

| Condition | ActionType |
|---|---|
| turn_number increased | `TurnChange` |
| phase changed | `PhaseChange` |
| life changed | `LifeChange` |
| Thing appeared, from_zone=Library, zone=Hand | `DrawCard` |
| Thing Hand→Battlefield, never seen on Stack | `PlayLand` |
| Thing appeared on Stack, has SRC_THING_ID to battlefield permanent | `ActivateAbility` (ability_id = stack thing's thing_id as string; card_id = SRC_THING_ID source permanent's thing_id as string) |
| Thing appeared on Stack (other) | `CastSpell` |
| Thing Stack→Battlefield | `ZoneTransition` (a creature/artifact resolving — the transition itself is the interesting event) |
| Thing Stack→non-Battlefield | `Resolve` (means "left the stack to a non-battlefield zone" — fires for actual resolution, bouncing, countering, etc. Not strictly MTG "resolve" semantics) |
| Thing zone changed (other) | `ZoneTransition` |
| attacking became true | `Attack` |
| blocking became true | `Block` |
| tapped changed | `TapPermanent` / `UntapPermanent` |
| damage changed | `DamageMarked` |
| summoning_sickness changed | `SummoningSickness` |
| face_down changed | `FaceDown` / `FaceUp` |
| attached_to_id changed | `Attach` / `Detach` |
| plus/minus counters changed | `CounterUpdate` |
| loyalty changed | `CounterUpdate` (type "loyalty") |
| power or toughness changed | `PowerToughnessUpdate` |

Multi-game: `reset()` triggered by GameOver, GameStatusChange (status 5 or 7), or boundary signals (960/967/2463). Clears prev, start_time, player_names, things_seen_on_stack.

### C4: End-to-end pipeline + golden test
- Wire full pipeline: framing → fls → game_messages → statebuf → state → translator → ReplayFile
- Update `bin/decode.rs` to run full pipeline and output JSON
- Snapshot golden file output as JSON fixture for regression
- New integration tests against real pipeline output

### Phase C exit criterion
Golden file produces a complete `ReplayFile` with recognizable game actions. Manual inspection confirms actions match the captured game.

---

## Phase D: Testing & Hardening

Unit tests are written alongside each module in B and C, not as a separate phase. Phase D covers the cross-cutting test concerns.

### D1: Golden file regression
- Full pipeline JSON snapshot as regression fixture (produced in C4)
- Checksum audit: verify rolling checksum for every GamePlayStatusMessage
- Framing smoke test: every message header parses without panicking

### D2: Commutativity check
Route A vs Route B verification for every diff message in the golden file. Both routes operate on the raw assembled byte buffers:
- Route A: apply wire diff to old_state bytes → parse elements from result → build GameState → translate
- Route B: parse elements from old_state bytes → build old GameState → apply wire diff to old_state bytes → parse elements from result → build new GameState → translate
- Assert identical `Vec<ReplayAction>` output

### D3: Fuzz testing (deferred)
`cargo fuzz` targets for framing, apply_diffs, parse_elements, and full pipeline. Tracked as follow-up, not blocking for initial release.

---

## Module dependency graph

```
framing.rs ──► fls.rs ──► game_messages.rs ──► statebuf.rs ──► state.rs ──► translator.rs ──► schema.rs
                                                                                                  │
opcodes.rs ◄── (constants used by all modules)                                                    ▼
                                                                                            ReplayFile (JSON)
```

## Files changed summary

| Action | File |
|---|---|
| Delete | `src/protocol/decoder.rs` |
| Delete | `src/protocol/raw_analyzer.rs` |
| Delete | `src/replay/writer.rs` |
| Delete | `src/replay/boundary.rs` |
| Delete | `tests/integration.rs` |
| Modify | `src/protocol/mod.rs` |
| Modify | `src/replay/mod.rs` |
| Modify | `src/lib.rs` |
| Modify | `src/protocol/opcodes.rs` |
| Modify | `src/replay/schema.rs` |
| Modify | `src/bin/decode.rs` |
| Create | `src/protocol/fls.rs` |
| Create | `src/protocol/game_messages.rs` |
| Create | `src/protocol/statebuf.rs` |
| Create | `src/state.rs` |
| Create | `src/translator.rs` |
