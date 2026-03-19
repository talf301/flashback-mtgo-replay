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
[replay schema]    existing ReplayFile / ActionType in src/replay/schema.rs (unchanged)
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
  statebuf/
    mod.rs           — assembly buffer, diff (ApplyDiffs2), element dispatch
    elements.rs      — StateElement enum + typed structs per element type

src/state.rs         — new: GameState + ThingState + PlayerState (internal model)
src/translator.rs    — new: diffs GameState snapshots → Vec<ReplayAction>
```

`src/protocol/decoder.rs` and `src/protocol/raw_analyzer.rs` are deleted.
`src/replay/` is untouched.

---

## Section 1: Protocol Layer

### framing.rs

Mirrors the wire format exactly. Every message (outer and embedded MetaMessage) has the same 8-byte header.

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

`read_message` reads 4 bytes for `total_len`, then reads `total_len - 4` more bytes, then slices out opcode/type_check/payload.

### fls.rs

Decodes the FLS messages relevant to game replay. `GsPlayerOrderMessage` (opcode 1155) is included here because it carries player seat order required by the translator.

```rust
pub enum FlsMessage {
    GsMessage        { match_token: [u8; 16], match_id: i32, game_id: i32, meta: Vec<u8> },
    GsReplayMessage  { game_id: i32, host_gsh_server_id: i32, meta: Vec<u8> },
    GameStatusChange { new_status: u32 },
    PlayerOrder      { seats: Vec<PlayerSeat> },
    Other(RawMessage),
}

pub struct PlayerSeat {
    pub seat_index: u32,
    pub player_name: String,   // wide string from wire
}

pub fn decode_fls(msg: RawMessage) -> Result<FlsMessage>
```

`meta` is the raw `MetaMessage byte[]` field — a complete embedded CSMessage starting with its own 8-byte header.

**Note on `game_id` types:** `GsMessage.game_id` is `i32` on the wire; `GameState.game_id` is `u32`. Cast via `as u32` at the `apply_elements` call site. Negative values are not expected; log a warning if seen.

### game_messages.rs

Decodes the inner message extracted from a `meta` byte slice:

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
    pub state_size: u32,              // expected decoded size of StateBuf (for diff validation)
    pub undiffed_buffer_size: u32,    // non-zero → state_buf_raw is a diff against previous state
    pub n_state_elems: u32,
    pub priority_player: i32,
    pub checksum: i32,                // rolling checksum of current (post-diff) state buffer
    pub last_state_checksum: i32,     // rolling checksum of previous state buffer (diff input validation)
    pub replaying: u8,                // non-zero during replay playback
    pub flags: u8,
    pub game_state_timestamp: u32,
}

pub fn decode_game_message(meta: &[u8]) -> Result<GameMessage>
```

`decode_game_message` calls `framing::read_message` on the embedded bytes, then dispatches on opcode.

**Field order note:** fields must be read in the order listed above to keep the byte cursor aligned; all are present on the wire regardless of whether the decoder uses them.

### statebuf/mod.rs

Handles multi-chunk assembly and optional diff decompression before element parsing.

```rust
pub struct StateBufProcessor {
    assembly_buffer: Vec<u8>,
    previous_state: Vec<u8>,
}

impl StateBufProcessor {
    pub fn new() -> Self
    pub fn process(&mut self, msg: &GamePlayStatusMessage) -> Result<Vec<StateElement>>
}
```

**Flags bits** (from `GamePlayStatusMessage.flags`):

| Bit | Constant                 | Meaning                                        |
|-----|--------------------------|------------------------------------------------|
| 0   | `GamestateContainsDiffs` | `state_buf_raw` is a diff, not a full state    |
| 1   | `GamestateHead`          | Start a new assembly buffer (first/only chunk) |
| 2   | `GamestateTail`          | Last chunk — process now                       |

**Diff format** (`ApplyDiffs2`): sequence of variable-length opcodes over the previous state buffer:

| Leading byte                               | Meaning                                                                      |
|--------------------------------------------|------------------------------------------------------------------------------|
| `0x00`                                     | Copy: `uint16` count + `int16` seek-from-current; copy from old state        |
| `0x80`, low 7 bits == 0, next byte == 0    | Long literal: 3-byte LE count, then that many literal bytes                  |
| `0x80`, low 7 bits == 0, next byte != 0    | Medium copy: count = next byte; `int16` seek; copy from old state            |
| `0x80..0xFF` (low 7 bits != 0)             | Short copy: count = low 7 bits; `sbyte` seek; copy from old state            |
| `0x01..0x7F`                               | Literal: count = byte value (max 127); read that many literal bytes          |

**Checksum validation:**
- Before diff: validate `last_state_checksum` against the rolling checksum of `previous_state`
- After diff (or directly for non-diff): validate `checksum` against the rolling checksum of the assembled buffer
- Rolling sum: seeded at `826366246`, `checksum = (checksum << 1) + byte` for each byte

### statebuf/elements.rs

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
    pub graveyard_count: Vec<i16>,   // stored in PlayerState.graveyard_count (index by seat)
    pub time_left: Vec<i32>,         // consumed; not currently used by state layer
    // background_image_names: consumed and discarded (asset names, not game-relevant)
    pub active_player: u8,
}

pub struct TurnStepElement {
    pub turn_number: i32,
    pub phase: u8,          // GamePhase enum value
    // Remaining fields (PromptText, PromptedPlayer, SpecialStepType, TimeStamp, button texts,
    // UIElements, etc.) are skipped using the element's total_size header after reading
    // turn_number and phase. PromptText is a variable-length string that sits between
    // phase and PromptedPlayer on the wire, so sequential field reads cannot reach
    // PromptedPlayer/TimeStamp without also parsing it. Since neither is used in
    // diff rules, we skip the entire remainder. Add explicit field reads here if
    // prompt or timestamp data becomes needed.
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

The list is terminated by `key_with_type == 0x00000000`. Nested `List` entries use the same termination — there is no separate length prefix; recursion ends on the same `0x00000000` sentinel.

| Type bits    | Encoding                                                                                   |
|--------------|--------------------------------------------------------------------------------------------|
| `0x20000000` | `int8` value                                                                               |
| `0x28000000` | `int32` value; remap key: `(key & 0xD7FFFFFF) \| 0x20000000`, then strip top bits         |
| `0x40000000` | `uint16` length + ISO-8859-1 bytes; if length == `0xFFFF` → string table lookup (see TODO)|
| `0x08000000` | Nested attribute list (recurse with same termination logic)                                |
| `0x10000000` | No value (function type, skip)                                                             |
| `0x48000000` | StringConstant — **no defined payload length in the protocol**; abort the containing `ThingElement` and store it as `Other { element_type: 5, raw }`. Log a warning. |

---

### ⚠️ TODO: GsPlayerOrderMessage Wire Layout

`GsPlayerOrderMessage` (opcode 1155) is not fully documented in `PROTOCOL_RESEARCH.md` — the field layout is not yet decompiled. Before implementing `FlsMessage::PlayerOrder` decoding, decompile this message class from `MTGOMessage.dll` (use the `--reflect GsPlayerOrderMessage` mode of `tools/opcode-dump`). The assumed layout follows the standard convention (`int32` element count, then per-element fields) but must be verified. Until then, implement the `PlayerOrder` arm as a no-op that logs the raw payload — the fallback player naming (`"player_0"`, `"player_1"`) keeps the rest of the pipeline functional.

Additionally, `V3ReplayUserOrderMessage` (opcode 4689) serves the same purpose for v3 replay mode. It is in the game-message opcode range and would be decoded in `game_messages.rs`. Its wire layout is also unresearched — track alongside opcode 1155.

---

### ⚠️ TODO: String Table

When a `String` property has `length == 0xFFFF`, the next 4 bytes are a `uint32 stringTableIndex` referencing a shared string table rather than inline bytes. The string table is populated from other messages not yet fully researched.

**Current behaviour:** store as placeholder string `"<strtable:N>"` where N is the index.
**To fix:** identify which FLS/game message carries the string table payload, decode it, and pass the table into the StateBuf processor. Track in `PROTOCOL_RESEARCH.md` under "String Table".

**Impact:** card names and some other string properties may show as placeholders until this is resolved. Zone, zone-transition, and life-change tracking (integers) are unaffected.

---

## Section 2: State Layer (`src/state.rs`)

Internal board model — no protocol types leak through this boundary.

```rust
pub struct GameState {
    pub game_id: u32,
    pub timestamp: u32,
    pub turn: i32,
    pub phase: GamePhase,
    pub active_player: usize,
    pub players: Vec<PlayerState>,
    pub things: HashMap<u32, ThingState>,   // keyed by THINGNUMBER (MagicProperty 537878017)
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
    pub from_zone: Option<CardZone>,   // previous zone from ThingElement.from_zone; Some on each
                                       // update, consumed by translator after each process() call
    pub controller: usize,
    pub owner: usize,
    pub card_texture_id: Option<u32>,
    pub card_name: Option<String>,
    pub tapped: bool,
    pub power: Option<i32>,
    pub toughness: Option<i32>,
    pub attacking: bool,
    pub blocking: bool,
    pub is_token: bool,
}
```

`GameState::apply_elements(elements: &[StateElement]) -> Result<()>` updates in place:
- Each `Thing` element upserts by `THINGNUMBER`; sets `from_zone` on every update
- Each `PlayerStatus` element updates all players
- `TurnStep` updates `turn`, `phase`, `active_player`

After the translator calls `process()` it must clear `from_zone` on all things (`thing.from_zone = None`) so the field is not re-consumed on the next snapshot.

---

## Section 3: Translator (`src/translator.rs`)

Stateful; diffs consecutive snapshots to emit `ReplayAction` records, then finalises the `ReplayHeader`.

```rust
pub struct ReplayTranslator {
    prev: Option<GameState>,
    player_names: Vec<String>,   // seat index → player name; populated from GsPlayerOrderMessage
    start_time: Option<DateTime<Utc>>,
}

impl ReplayTranslator {
    pub fn new() -> Self
    pub fn set_player_order(&mut self, seats: Vec<PlayerSeat>)
    pub fn process(&mut self, new_state: GameState, ts: DateTime<Utc>) -> Vec<ReplayAction>
    pub fn finish(self, result: GameResult, end_time: DateTime<Utc>) -> ReplayHeader
}
```

### ReplayHeader Assembly

| Field | Source |
|---|---|
| `game_id` | `GameState.game_id` cast to `String` |
| `players` | `player_names` from `GsPlayerOrderMessage` (seat order preserved); `life_total` = `PlayerStatusElement.life[seat_index]` cast `i16 → i32` from first `PlayerStatus` element |
| `format` | Not available in protocol — default to `"Unknown"` |
| `start_time` | Wall clock when `process()` is first called |
| `end_time` | Wall clock when `finish()` is called (on `GameOver` message) |
| `result` | `GameResults.winner_seat` → look up `player_names[winner_seat as usize]` → `GameResult::Win { winner_id }` |

If `GsPlayerOrderMessage` is not received before the first `process()` call, seat indices are used as fallback player IDs (`"player_0"`, `"player_1"`, etc.).

### Diff Rules

Emit order within a single `process()` call:

| Condition | Emitted `ActionType` |
|---|---|
| `TurnStep.turn_number` increased | `TurnChange { turn, player_id: active_player_name }` |
| `TurnStep.phase` changed | `PhaseChange { phase: phase_name }` |
| `PlayerState.life` changed | `LifeChange { player_id, old_life, new_life }` |
| `thing_id` absent from `prev.things` AND `from_zone == Library(2)` AND new `zone == Hand(1)` | `DrawCard { player_id: owner_name, card_id: thing_id_str }` |
| `thing_id` absent from `prev.things` AND new `zone == Stack(8)` | `CastSpell { player_id: controller_name, card_id: thing_id_str }` |
| `thing_id` present in `prev.things` AND zone changed from `Stack(8)` to `Battlefield(7)` | `ZoneTransition { from_zone: "stack", to_zone: "battlefield", ... }` |
| `thing_id` present in `prev.things` AND zone changed from `Stack(8)` to any non-Battlefield | `Resolve { card_id: thing_id_str }` |
| `thing_id` present in `prev.things` AND zone changed (any other transition) | `ZoneTransition { from_zone, to_zone, card_id, player_id: controller_name }` |
| `ThingState.attacking` became `true` | `Attack { attacker_id: thing_id_str, defender_id: opponent_name }` |
| `ThingState.blocking` became `true` | `Block { blocker_id: thing_id_str, attacker_id: "unknown" }` |

**"Appeared" definition:** a `thing_id` key that is absent from `prev.things` (newly tracked in this snapshot). If a known thing's zone changes to Stack, emit `ZoneTransition`, not `CastSpell`. The Stack→Battlefield and Stack→non-Battlefield rows take priority over the catch-all `ZoneTransition` row — evaluate specific zone-from/to pairs first.

**`PlayLand` note:** The protocol cannot distinguish land plays from other Hand→Battlefield transitions in the StateBuf — there is no separate "play land" event. `ActionType::PlayLand` will not be emitted; land plays appear as `ZoneTransition { from_zone: "hand", to_zone: "battlefield" }`. `PlayLand` remains in the schema for compatibility but is effectively unused post-implementation.

**Attack / defender_id:** The `ATTACKING` property is a boolean — the wire protocol does not encode which player or planeswalker is being attacked in the state snapshot. `defender_id` is set to the opponent's player name (seat `1 - controller_seat` in a 2-player game). Planeswalker attacks and multiplayer (>2 players) are not supported; both produce the same default opponent assignment.

**Block / attacker_id:** The `BLOCKING` property is also boolean; the attacker being blocked is not encoded in the StateBuf. `attacker_id` is emitted as `"unknown"`. A future improvement could correlate with `ATTACHED_TO_ID` or combat association data if it becomes available.

**PassPriority:** This is a client→server action (`GameNextStepMessage` opcode 4643) not reflected in server-sent state snapshots. It cannot be derived from StateBuf diffs. Deferred — not emitted in this implementation.

Player IDs in emitted actions are looked up from `player_names[seat]`.

---

## Key MagicProperty Constants

Defined in `opcodes.rs`:

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
| `IS_TOKEN` | 537876520 | Is a token |

---

## Error Handling

All parsing functions return `Result<T, DecodeError>` using `thiserror`. `DecodeError` variants:

- `Io(std::io::Error)` — stream read failure
- `UnexpectedEof { context: &'static str }` — buffer too short
- `InvalidChecksum { expected: i32, got: i32 }` — StateBuf checksum mismatch
- `UnknownElementType(u32)` — unrecognised StateElementType (non-fatal: stored as `Other`)
- `UnknownPropertyType(u32)` — unrecognised property type tag (non-fatal: skip entry)
- `StringConstantEncountered { thing_id: Option<u32> }` — `0x48000000` tag found; ThingElement stored as `Other`

Unknown element/property types are non-fatal so a single unrecognised field doesn't abort an entire game decode.

---

## Testing Strategy

- **framing.rs**: unit-test round-trip with hand-crafted byte slices
- **fls.rs**: unit-test with minimal hand-crafted GsMessageMessage and GsPlayerOrderMessage payloads
- **statebuf**: unit-test `ApplyDiffs2` with known input/output pairs; unit-test element parsing with hand-crafted PropertyContainer bytes including nested lists and string-table placeholders
- **state.rs**: unit-test `apply_elements` with synthetic element sequences; verify `from_zone` is set and cleared correctly
- **translator.rs**: unit-test diff logic with synthetic before/after `GameState` pairs covering each diff rule; test `set_player_order` + `finish` for header assembly
- **Integration**: end-to-end test with a real captured (decrypted) dump file once TLS is resolved
