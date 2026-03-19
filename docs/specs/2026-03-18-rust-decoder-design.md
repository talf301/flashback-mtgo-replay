# Rust Protocol Decoder Design

**Date:** 2026-03-18
**Status:** Approved
**Goal:** Implement a full MTGO protocol decoder in Rust, converting decrypted TCP stream bytes into `ReplayFile` / `ReplayAction` records suitable for storage and replay playback.

---

## Context

The MTGO protocol is fully reverse-engineered (see `PROTOCOL_RESEARCH.md`). The existing `src/protocol/decoder.rs` and `src/protocol/raw_analyzer.rs` are placeholders that return `Unknown` for everything. This design replaces them with a real implementation.

The full pipeline:

```
decrypted bytes
      │
      ▼
[protocol layer]   framing → FLS envelope → game message → StateBuf elements
      │
      ▼
[state layer]      GameState — internal board model, updated from each StateBuf
      │
      ▼
[translator]       diffs consecutive GameState snapshots → Vec<ReplayAction>
      │
      ▼
[replay schema]    ReplayFile / ActionType in src/replay/schema.rs
                   (schema.rs gains new ActionType variants — see Section 4)
```

---

## Module Structure

Replace `src/protocol/` contents:

```
src/protocol/
  mod.rs             — public API, re-exports
  opcodes.rs         — opcode constants and MagicProperty constants
  framing.rs         — 8-byte header parsing, stream reading → RawMessage
  fls.rs             — FLS envelope decoding → FlsMessage
  game_messages.rs   — inner game message dispatch → GameMessage
  statebuf.rs        — assembly buffer, element dispatch, element structs

src/state.rs         — new: GameState + ThingState + PlayerState (internal model)
src/translator.rs    — new: diffs GameState snapshots → Vec<ReplayAction>
```

`src/protocol/decoder.rs` and `src/protocol/raw_analyzer.rs` are deleted.
`src/replay/schema.rs` gains new `ActionType` variants (Section 4) but is otherwise untouched.

---

## Section 1: Protocol Layer

### framing.rs

Every message (outer and embedded MetaMessage) shares the same 8-byte header:

```
Offset  Size  Type    Description
------  ----  ------  -----------
0       4     int32   Total buffer length (including header), little-endian
4       2     uint16  OpCode
6       2     uint16  TypeCheckValue (first 2 bytes of MD5 of type layout)
8+      var   bytes   Payload
```

```rust
pub struct RawMessage { pub opcode: u16, pub type_check: u16, pub payload: Vec<u8> }

pub fn read_message(r: &mut impl Read) -> Result<RawMessage>
pub fn parse_messages(data: &[u8]) -> Result<Vec<RawMessage>>
```

`read_message` reads 4 bytes as `i32` for `total_len`. If `total_len < 8` or `total_len < 0`, return `Err(DecodeError::UnexpectedEof { context: "frame too short" })`. Otherwise read `total_len - 4` more bytes and slice out opcode/type_check/payload. The minimum valid frame is 8 bytes (header only, empty payload).

### fls.rs

Decodes the FLS messages relevant to game replay. `GsMessageMessage` (1153) and `GsReplayMessageMessage` (1156) are merged into a single `GsMessage` variant — their envelope metadata (`match_token`, `match_id`, `host_gsh_server_id`) is consumed off the wire but not stored, since nothing downstream uses it.

```rust
pub enum FlsMessage {
    GsMessage   { game_id: i32, meta: Vec<u8> },
    GameStatusChange { new_status: u32 },
    PlayerOrder { seats: Vec<PlayerSeat> },
    Other(RawMessage),
}

pub struct PlayerSeat {
    pub seat_index: u32,
    pub player_name: String,
}

pub fn decode_fls(msg: RawMessage) -> Result<FlsMessage>
```

`meta` is the raw `MetaMessage byte[]` — a complete embedded CSMessage with its own 8-byte header.

**Note on `game_id` types:** `GsMessage.game_id` is `i32` on the wire; `GameState.game_id` is `u32`. Cast via `as u32` at the `apply_elements` call site. Log a warning if the value is negative.

### ⚠️ TODO: GsPlayerOrderMessage Wire Layout

`GsPlayerOrderMessage` (opcode 1155) and `V3ReplayUserOrderMessage` (opcode 4689, game-message range, decoded in `game_messages.rs`) both carry seat order but their field layouts are not yet documented in `PROTOCOL_RESEARCH.md`. Before implementing `PlayerOrder` decoding, decompile these message classes using `tools/opcode-dump --reflect`. Until then, implement `PlayerOrder` as a no-op that logs the raw payload. The fallback player naming (`"player_0"`, `"player_1"`) keeps the pipeline functional.

### game_messages.rs

```rust
pub enum GameMessage {
    GamePlayStatus(GamePlayStatusMessage),
    GameOver,
    GameResults { winner_seat: u32 },
    Other { opcode: u16 },
}

pub struct GamePlayStatusMessage {
    pub state_buf_raw: Vec<u8>,
    pub time_left: [i32; 8],          // chess clock ticks per player (64 ticks/second)
    pub player_waiting_for: i32,      // index of player whose clock is running
    pub game_id: u32,
    pub state_size: u32,              // expected decoded size of StateBuf
    pub undiffed_buffer_size: u32,    // non-zero → state_buf_raw is a diff (see diffs TODO)
    pub n_state_elems: u32,
    pub priority_player: i32,
    pub checksum: i32,                // rolling checksum of current state buffer
    pub last_state_checksum: i32,     // rolling checksum of previous state (diff input validation)
    pub flags: u8,
    pub game_state_timestamp: u32,
    // `replaying: u8` is read off the wire for byte-alignment but not stored.
}

pub fn decode_game_message(meta: &[u8]) -> Result<GameMessage>
```

`decode_game_message` calls `framing::read_message` on the embedded bytes, then dispatches on opcode. All fields must be read in wire order to keep the byte cursor aligned.

### ⚠️ TODO: GameResultsMessage Wire Layout

`GameResultsMessage` (opcode 4485) field layout is not documented in `PROTOCOL_RESEARCH.md`. Decompile `GameResultsMessage.UnpackBuffer()` from `MTGOMessage.dll` before implementing. In particular: identify which field is `winner_seat`, confirm whether draws use a special sentinel, and whether multiplayer results include multiple seats. Until then, decode as `GameMessage::Other` and emit `GameResult::Incomplete` in the header.

### statebuf.rs

**Assembly and diff flags** (from `GamePlayStatusMessage.flags`):

| Bit | Constant                 | Meaning                                        |
|-----|--------------------------|------------------------------------------------|
| 0   | `GamestateContainsDiffs` | `state_buf_raw` is a diff against previous state |
| 1   | `GamestateHead`          | Start a new assembly buffer                    |
| 2   | `GamestateTail`          | Last chunk — process now                       |

```rust
pub struct StateBufProcessor {
    assembly_buffer: Vec<u8>,
    previous_state: Vec<u8>,
}

impl StateBufProcessor {
    pub fn new() -> Self
    pub fn process(&mut self, msg: &GamePlayStatusMessage) -> Result<Vec<StateElement>>
    pub fn reset(&mut self)   // called between games — see Section 3
}
```

**Assembly buffer edge cases:**
- On `GamestateHead`: unconditionally reset `assembly_buffer` to empty. If bytes were discarded, log a warning.
- On `GamestateTail` with empty `assembly_buffer` and `GamestateContainsDiffs` clear: treat `state_buf_raw` as a complete single-chunk full state and process it.
- On `GamestateTail` with `GamestateContainsDiffs` set and `previous_state` empty: return `Err(DecodeError::UnexpectedEof { context: "diff tail without prior head and no previous state" })`.

**Checksum:** rolling sum seeded at `826366246`, `checksum = (checksum << 1) + byte` for each byte.
- For non-diff messages: validate `checksum` against the assembled buffer. Do **not** validate `last_state_checksum`.
- For diff messages: validate `last_state_checksum` against `previous_state` before applying, then validate `checksum` against the result after applying.

### ⚠️ TODO: ApplyDiffs2 (Deferred)

The diff algorithm (`GamestateContainsDiffs` bit) is deferred until real captured traffic is available for testing. The five-opcode diff format is complex (signed relative seeks, three copy variants, two literal variants) and difficult to validate against synthetic data alone.

**Current behaviour:** if `GamestateContainsDiffs` is set, log a warning and return an empty `Vec<StateElement>`. The assembly buffer is still updated so subsequent full-state messages are unaffected. This means game events during diff-compressed state updates are missed, but full-state resync (game start, reconnect) works correctly.

**When implementing:** the copy opcodes use a signed `int16` (or `sbyte` for short-copy) seek relative to the old-state cursor *after* reading the seek field itself. Before each seek, verify `old_cursor + seek` is in `[0, previous_state.len())`. Before each copy, verify `source + count <= previous_state.len()`. Return `Err(DecodeError::UnexpectedEof { context: "diff copy out of bounds" })` on violation. Validate `state_size` == assembled length after applying diffs.

### statebuf.rs — Elements

Each element in the assembled buffer:

```
Offset  Size  Type              Description
------  ----  ----------------  -----------
0       4     int32             Total element size (including this 8-byte header)
4       4     StateElementType  Element type enum
8+      N-8   byte[]            Element payload
```

```rust
pub enum StateElement {
    PlayerStatus(PlayerStatusElement),
    TurnStep(TurnStepElement),
    Thing(ThingElement),
    Other { element_type: u32, raw: Vec<u8> },
}

pub struct ThingElement {
    pub from_zone: i32,
    pub props: HashMap<u32, PropertyValue>,
}

pub struct PlayerStatusElement {
    pub life: Vec<i16>,
    pub hand_count: Vec<i16>,
    pub library_count: Vec<i16>,
    pub graveyard_count: Vec<i16>,
    pub time_left: Vec<i32>,         // consumed; not currently used by state layer
    // background_image_names: consumed and discarded
    pub active_player: u8,
}

pub struct TurnStepElement {
    pub turn_number: i32,
    pub phase: u8,                   // GamePhase enum value
    // PromptText (variable-length string) sits between phase and PromptedPlayer on
    // the wire, making sequential field reads past it impractical. All fields after
    // phase are skipped using the element's total_size header. Add explicit reads
    // here if prompt/button data is needed later.
}

pub enum PropertyValue {
    Int(i32),
    Str(String),
    List(HashMap<u32, PropertyValue>),
}
```

**PropertyContainer (ThingElement attribute list):**

Each entry is a `uint32 key_with_type`:
- Bits 31–27 (`0xF8000000`): type tag
- Bits 26–0 (`0x07FFFFFF`): `MagicProperty` key
- Terminated by `key_with_type == 0x00000000`
- Nested `List` entries use the same sentinel — no separate length prefix; recursion ends on `0x00000000`

| Type bits    | Encoding |
|---|---|
| `0x20000000` | `int8` value |
| `0x28000000` | `int32` value; remap key: apply `(keyWithType & 0xD7FFFFFF) \| 0x20000000` to the full 32-bit field, then mask to `0x07FFFFFF` |
| `0x40000000` | `uint16` length + ISO-8859-1 bytes; if length == `0xFFFF` → string table (see TODO) |
| `0x08000000` | Nested attribute list (recurse) |
| `0x10000000` | No value (function type, skip) |
| `0x48000000` | StringConstant — no defined payload; abort the entire containing `ThingElement` (propagate out of any nested `readAttributes` frames) and store as `Other { element_type: 5, raw }`. Log a warning. The element cursor advances by `total_size` from the element header, not from within the attribute parser. |

Unknown type tags: log `tracing::warn!` and return `Err` to abort the containing element (stored as `Other`).

**`THINGNUMBER` absent:** if `ThingElement.props` does not contain `THINGNUMBER` after parsing, log an error and discard the element — do not upsert into `GameState`.

---

### ⚠️ TODO: String Table

When a `String` property has `length == 0xFFFF`, the next 4 bytes are a `uint32 stringTableIndex` into a shared string table. The table's origin message is not yet identified.

**Current behaviour:** store as `"<strtable:N>"` where N is the index.
**To fix:** identify which message carries the table, decode it, pass into `StateBufProcessor`. Track in `PROTOCOL_RESEARCH.md` under "String Table".
**Impact:** card names may appear as placeholders. Integer-keyed properties (zone, life, tapped, etc.) are unaffected.

### ⚠️ TODO: MiniChange (StateElementType 200)

StateElementType 200 (`MiniChange`) is described as "small incremental state update" in `PROTOCOL_RESEARCH.md` but its payload format is not documented. Currently decoded as `Other` and dropped.

**Risk:** if the server uses `MiniChange` to deliver life changes or zone transitions between full-state snapshots, those events will be silently missed. On a full-state message (`GamestateContainsDiffs` clear), `MiniChange` elements should not be needed for correctness — log a warning if one appears. On a diff message, log a warning and treat the state as suspect.

**To fix:** decompile `MiniChangeElement.ReadBuffer()` from `WotC.MtGO.Client.Model.Play.dll`. Track in `PROTOCOL_RESEARCH.md` under "MiniChange".

---

## Section 2: State Layer (`src/state.rs`)

Internal board model — no protocol types leak through this boundary.

```rust
pub struct GameState {
    pub game_id: u32,
    pub turn: i32,
    pub phase: GamePhase,
    pub active_player: usize,
    pub players: Vec<PlayerState>,
    pub things: HashMap<u32, ThingState>,   // keyed by THINGNUMBER (537878017)
}

pub struct PlayerState {
    pub seat: usize,
    pub life: i32,
    pub hand_count: i32,
    pub library_count: i32,
    pub graveyard_count: i32,
}

pub struct ThingState {
    pub thing_id: u32,
    pub zone: CardZone,
    pub from_zone: Option<CardZone>,   // set on each update; cleared by translator after process()
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
}
```

`GameState::apply_elements(elements: &[StateElement]) -> Result<()>` updates in place:

- **`Thing` elements:** upsert by `THINGNUMBER`. Only update fields whose properties are present in `props` — absent properties retain their current values. On first insert, absent fields default to zero/false/None.
- **`PlayerStatus` elements:** update all players by seat index. If `life.len() <= seat`, skip that seat rather than panicking. Apply the same guard to `hand_count`, `library_count`, `graveyard_count`.
- **`TurnStep` elements:** update `turn`, `phase`, `active_player`.
- **Full-state pruning:** when `GamestateContainsDiffs` is clear (full state), after upserting all `Thing` elements, remove from `things` any `thing_id` not seen in this update's `Thing` elements. On a diff message, do not prune (absent entries may simply be unchanged).

After the translator calls `process()`, it must clear `from_zone` on all things (`thing.from_zone = None`) so the field is not re-consumed on the next snapshot.

---

## Section 3: Translator (`src/translator.rs`)

Stateful; diffs consecutive snapshots to emit `ReplayAction` records.

```rust
pub struct ReplayTranslator {
    prev: Option<GameState>,
    player_names: Vec<String>,
    start_time: Option<DateTime<Utc>>,
}

impl ReplayTranslator {
    pub fn new() -> Self
    pub fn set_player_order(&mut self, seats: Vec<PlayerSeat>)
    pub fn process(&mut self, new_state: GameState, ts: DateTime<Utc>) -> Vec<ReplayAction>
    pub fn finish(self, result: GameResult, end_time: DateTime<Utc>) -> ReplayHeader
    pub fn reset(&mut self)   // called between games
}
```

**`ReplayAction.timestamp`:** set to the `ts: DateTime<Utc>` passed into `process()` (wall clock at the time of the call). All actions emitted from a single `process()` call share the same timestamp. Do not use `GameState`'s protocol timestamp counter.

**Seat-to-name lookup:** always use `player_names.get(seat).cloned().unwrap_or_else(|| format!("player_{}", seat))`. Never index `player_names` directly — it may be empty if `GsPlayerOrderMessage` has not yet arrived.

**Multi-game sessions:** MTGO reuses the same TCP connection across games. When `GameOverMessage` or `GshGameStatusChangeMessage` with `new_status == GAME_COMPLETED (7)` or `GAME_TERMINATED (5)` arrives, the caller must:
1. Call `translator.finish()` to close the current replay
2. Call `translator.reset()` and `state_buf_processor.reset()` before processing the next game

`reset()` sets `prev = None`, `start_time = None`, and clears `player_names` to the fallback state. This prevents the last state of game N from being diffed against the first state of game N+1.

### ReplayHeader Assembly

| Field | Source |
|---|---|
| `game_id` | `GameState.game_id` cast to `String` |
| `players` | `player_names` from `set_player_order`; `life_total` = `PlayerStatusElement.life[seat_index]` cast `i16 → i32` from first `PlayerStatus` element |
| `format` | Not in protocol — default to `"Unknown"` |
| `start_time` | Wall clock on first `process()` call |
| `end_time` | Wall clock when `finish()` is called |
| `result` | From `GameResultsMessage.winner_seat` (see TODO) → `player_names.get(winner_seat as usize)` with fallback → `GameResult::Win { winner_id }`. Bounds-safe: use `.get()`, not direct index |

### Diff Rules

**"Appeared" definition:** `thing_id` absent from `prev.things`. "Zone changed" means `thing_id` present in both `prev.things` and `new_state.things` with different `zone`. Evaluate specific cases before the catch-all; rules are not mutually exclusive only where noted.

Emit order within a single `process()` call:

| Condition | Emitted `ActionType` |
|---|---|
| `TurnStep.turn_number` increased | `TurnChange { turn, player_id: active_player_name }` |
| `TurnStep.phase` changed | `PhaseChange { phase: phase_name }` |
| `PlayerState.life` changed | `LifeChange { player_id, old_life, new_life }` |
| Thing appeared, `from_zone == Library(2)`, `zone == Hand(1)` | `DrawCard { player_id: owner_name, card_id: thing_id_str }` |
| Thing appeared, `zone == Stack(8)` | `CastSpell { player_id: controller_name, card_id: thing_id_str }` |
| Thing zone changed from `Stack(8)` to `Battlefield(7)` (known thing) | `ZoneTransition { from_zone: "stack", to_zone: "battlefield", ... }` |
| Thing zone changed from `Stack(8)` to non-Battlefield (known thing) | `Resolve { card_id: thing_id_str }` |
| Thing zone changed (any other transition, known thing) | `ZoneTransition { from_zone, to_zone, card_id, player_id: controller_name }` |
| `ThingState.attacking` became `true` | `Attack { attacker_id: thing_id_str, defender_id: opponent_name }` |
| `ThingState.blocking` became `true` | `Block { blocker_id: thing_id_str, attacker_id: "unknown" }` |
| `ThingState.tapped` changed | `TapPermanent { card_id }` or `UntapPermanent { card_id }` |
| `ThingState.damage` changed | `DamageMarked { card_id, damage }` (absolute value) |
| `ThingState.summoning_sickness` changed | `SummoningSickness { card_id, has_sickness }` |
| `ThingState.face_down` changed | `FaceDown { card_id }` or `FaceUp { card_id }` |
| `ThingState.attached_to_id` changed | `Attach { card_id, attached_to_id }` or `Detach { card_id }` (if None) |
| `ThingState.plus_counters` changed | `CounterUpdate { card_id, counter_type: "+1/+1", count }` |
| `ThingState.minus_counters` changed | `CounterUpdate { card_id, counter_type: "-1/-1", count }` |
| `ThingState.loyalty` changed | `CounterUpdate { card_id, counter_type: "loyalty", count }` |
| `ThingState.power` or `ThingState.toughness` changed | `PowerToughnessUpdate { card_id, power, toughness }` (emit once with both current values) |

**`Attack` / `defender_id`:** `ATTACKING` is a boolean; the wire protocol does not encode which player/planeswalker is targeted. In a 2-player game, `defender_id` defaults to the opponent's name (`seat 1 - controller_seat`). Planeswalker attacks and multiplayer (>2 players) produce the same default — a known limitation.

**`Block` / `attacker_id`:** `BLOCKING` is also boolean; `attacker_id` is `"unknown"`. A future improvement could correlate with `ATTACHED_TO_ID` or combat association data.

**`CastSpell` heuristic:** fires for any new thing appearing on Stack, including triggered and activated abilities and copies. This is a best-effort approximation — the StateBuf model does not distinguish "cast" from "triggered." Document in schema as such.

**`PlayLand`:** not emittable from StateBuf; land plays appear as `ZoneTransition { from_zone: "hand", to_zone: "battlefield" }`. `PlayLand` remains in the schema for compatibility but is unused by this decoder.

**`PassPriority`:** client→server action not reflected in server-sent state snapshots. Not emitted.

**`ActivateAbility`:** not emittable from StateBuf. Deferred.

**Opening hand:** cards dealt to the opening hand emit `DrawCard` (zone: Library→Hand). This is intentional. The viewer groups or suppresses initial-hand draws if desired.

**Deduplication:** do not use `game_state_timestamp` as a deduplication key. Always diff `prev` against `new_state` regardless of equal timestamps — state content may still differ (priority changes, etc.).

---

## Section 4: Schema Changes (`src/replay/schema.rs`)

Add to `ActionType`:

```rust
TapPermanent    { card_id: String },
UntapPermanent  { card_id: String },
DamageMarked    { card_id: String, damage: i32 },
SummoningSickness { card_id: String, has_sickness: bool },
FaceDown        { card_id: String },
FaceUp          { card_id: String },
Attach          { card_id: String, attached_to_id: String },
Detach          { card_id: String },
CounterUpdate   { card_id: String, counter_type: String, count: i32 },
PowerToughnessUpdate { card_id: String, power: i32, toughness: i32 },
```

All other existing variants are unchanged. `PlayLand`, `ActivateAbility`, and `PassPriority` remain in the enum for compatibility but are not emitted by this decoder.

---

## Key MagicProperty Constants (`opcodes.rs`)

| Constant | Value | Description |
|---|---|---|
| `THINGNUMBER` | 537878017 | Unique game-object ID |
| `ZONE` | 537878532 | Current zone |
| `CONTROLLER` | 537875729 | Controller seat index |
| `OWNER` | 537876535 | Owner seat index |
| `CARDTEXTURE_NUMBER` | 537875724 | Card art/definition ID |
| `CARDNAME_STRING` | 1074900499 | Card name |
| `TAPPED` | 538063116 | Tapped state |
| `ATTACKING` | 537874697 | Is attacking |
| `BLOCKING` | 538214407 | Is blocking |
| `POWER` | 538054425 | Creature power |
| `TOUGHNESS` | 538065157 | Creature toughness |
| `DAMAGE` | 537876480 | Damage marked on creature |
| `SUMMONING_SICK` | 537877508 | Has summoning sickness |
| `FACE_DOWN` | 537924114 | Face-down / morphed |
| `ATTACHED_TO_ID` | 538318848 | ID of permanent this is attached to |
| `IS_TOKEN` | 537876520 | Is a token |
| `PLUS_ONE_PLUS_ONE_COUNTERS` | 538054419 | +1/+1 counter count |
| `MINUS_ONE_MINUS_ONE_COUNTERS` | 538285849 | -1/-1 counter count |
| `LOYALTY_COUNTERS` | 538075911 | Loyalty counter count |

---

## Error Handling

All parsing functions return `Result<T, DecodeError>` using `thiserror`:

```rust
pub enum DecodeError {
    Io(#[from] std::io::Error),
    UnexpectedEof { context: &'static str },
    InvalidChecksum { expected: i32, got: i32 },
}
```

Non-fatal conditions use `tracing::warn!` and continue — they do not become `Err` variants:
- Unknown `StateElementType` → store as `Other`, warn
- Unknown property type tag → abort containing element (store as `Other`), warn
- `StringConstantEncountered` → abort containing element (propagating out of any nested `readAttributes` frames), store as `Other`, warn
- `THINGNUMBER` absent → discard element, warn

---

## CardZone and GamePhase Enums

```
CardZone:
  0=Invalid  1=Hand        2=Library    3=Graveyard  4=Exile
  5=LocalExileCanBePlayed  6=OpponentExileCanBePlayed
  7=Battlefield  8=Stack  9=Command  10=Commander  11=Planar
  12=Effects  13=LocalTriggers  14=OpponentTriggers  15=Aside
  16=Revealed  17-19=Pile1-3  20=Nowhere  21=Sideboard
  22=Mutate  23=Companion

GamePhase:
  0=Invalid  1=Untap  2=Upkeep  3=Draw
  4=PreCombatMain  5=BeginCombat
  6=DeclareAttackers  7=DeclareBlockers
  8=CombatDamage  9=EndOfCombat
  10=PostCombatMain  11=EndOfTurn  12=Cleanup
```

---

## Testing Strategy

- **framing.rs**: round-trip with hand-crafted byte slices; test `total_len < 8` guard
- **fls.rs**: hand-crafted `GsMessageMessage` (1153) and `GsReplayMessageMessage` (1156) payloads; verify both produce `GsMessage`
- **statebuf.rs**: element parsing with hand-crafted `PropertyContainer` bytes including nested lists, `StringConstant` abort, and absent `THINGNUMBER`; test assembly buffer edge cases (Tail-without-Head, Head-discards-partial)
- **state.rs**: `apply_elements` with synthetic element sequences; verify `from_zone` set-and-clear; full-state pruning removes absent things; upsert retains absent-property values
- **translator.rs**: synthetic before/after `GameState` pairs covering each diff rule; `set_player_order` + `finish` for header assembly; multi-game `reset()` prevents stale-state bleed; bounds-safe seat lookup with empty `player_names`
- **Integration**: end-to-end test with a real captured (decrypted) dump file once TLS is resolved
