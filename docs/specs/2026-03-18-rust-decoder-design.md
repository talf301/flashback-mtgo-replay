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
  opcodes.rs         — the ~15 opcode constants we care about (from 1482 total)
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

Decodes the three outer FLS messages relevant to game replay:

```rust
pub enum FlsMessage {
    GsMessage       { match_token: [u8; 16], match_id: i32, game_id: i32, meta: Vec<u8> },
    GsReplayMessage { game_id: i32, host_gsh_server_id: i32, meta: Vec<u8> },
    GameStatusChange { new_status: u32 },
    Other(RawMessage),
}

pub fn decode_fls(msg: RawMessage) -> Result<FlsMessage>
```

`meta` is the raw `MetaMessage byte[]` field — a complete embedded CSMessage starting with its own 8-byte header.

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
    pub game_id: u32,
    pub flags: u8,
    pub game_state_timestamp: u32,
    pub n_state_elems: u32,
    pub undiffed_buffer_size: u32,   // non-zero → state_buf_raw is a diff
    pub checksum: i32,
    pub last_state_checksum: i32,
    pub priority_player: i32,
    pub time_left: [i32; 8],
}

pub fn decode_game_message(meta: &[u8]) -> Result<GameMessage>
```

`decode_game_message` calls `framing::read_message` on the embedded bytes, then dispatches on opcode.

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

| Bit | Constant              | Meaning                                  |
|-----|-----------------------|------------------------------------------|
| 0   | `GamestateContainsDiffs` | `state_buf_raw` is a diff, not full state |
| 1   | `GamestateHead`       | Start a new assembly buffer              |
| 2   | `GamestateTail`       | Last chunk — process now                 |

**Diff format** (`ApplyDiffs2`): sequence of variable-length opcodes over the previous state buffer:

| Leading byte                               | Meaning                                                                      |
|--------------------------------------------|------------------------------------------------------------------------------|
| `0x00`                                     | Copy: `uint16` count + `int16` seek-from-current; copy from old state        |
| `0x80`, low 7 bits == 0, next byte == 0    | Long literal: 3-byte LE count, then that many literal bytes                  |
| `0x80`, low 7 bits == 0, next byte != 0    | Medium copy: count = next byte; `int16` seek; copy from old state            |
| `0x80..0xFF` (low 7 bits != 0)             | Short copy: count = low 7 bits; `sbyte` seek; copy from old state            |
| `0x01..0x7F`                               | Literal: count = byte value (max 127); read that many literal bytes          |

**Checksum** (validated before and after diff): rolling sum seeded at `826366246`, `checksum = (checksum << 1) + byte` for each byte.

### statebuf/elements.rs

Each element in the assembled buffer:

```
Offset  Size  Type            Description
------  ----  --------------  -----------
0       4     int32           Total element size (including this 8-byte header)
4       4     StateElementType  Element type enum
8+      N-8   byte[]          Element payload
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
    pub active_player: u8,
}

pub struct TurnStepElement {
    pub turn_number: i32,
    pub phase: u8,
    pub prompted_player: u8,
    pub timestamp: u32,
}

pub enum PropertyValue {
    Int(i32),
    Str(String),
    List(HashMap<u32, PropertyValue>),
}
```

**PropertyContainer (ThingElement attribute list):**

Each entry in the attribute list is a `uint32 key_with_type`:
- Bits 31–27 (`0xF8000000`): type tag
- Bits 26–0 (`0x07FFFFFF`): `MagicProperty` key

| Type bits    | Encoding                                                                                  |
|--------------|-------------------------------------------------------------------------------------------|
| `0x20000000` | `int8` value                                                                              |
| `0x28000000` | `int32` value; remap key: `(key & 0xD7FFFFFF) \| 0x20000000`, then strip top bits        |
| `0x40000000` | `uint16` length + ISO-8859-1 bytes; if length == `0xFFFF` → string table lookup (see TODO below) |
| `0x08000000` | Nested attribute list (recurse)                                                           |
| `0x10000000` | No value (function type, ignored)                                                         |

Terminated by `key_with_type == 0x00000000`.

---

### ⚠️ TODO: String Table

When a `String` property has `length == 0xFFFF`, the next 4 bytes are a `uint32 stringTableIndex` referencing a shared string table rather than an inline string. The string table is populated from other messages not yet fully researched.

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
}

pub struct ThingState {
    pub thing_id: u32,
    pub zone: CardZone,
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

`GameState::apply_elements(elements: &[StateElement]) -> Result<()>` updates in place. Each `Thing` element upserts by `THINGNUMBER`. Each `PlayerStatus` element updates all players. `TurnStep` updates turn/phase/active_player.

---

## Section 3: Translator (`src/translator.rs`)

Stateful; diffs consecutive snapshots to emit `ReplayAction` records.

```rust
pub struct ReplayTranslator {
    prev: Option<GameState>,
    player_names: Vec<String>,   // seat index → player name, set at game start
}

impl ReplayTranslator {
    pub fn new(player_names: Vec<String>) -> Self
    pub fn process(&mut self, new_state: GameState, ts: DateTime<Utc>) -> Vec<ReplayAction>
}
```

**Diff rules (in emit order):**

| Condition | Emitted action |
|-----------|---------------|
| `TurnStep.turn_number` increased | `TurnChange` |
| `TurnStep.phase` changed | `PhaseChange` |
| `PlayerState.life` decreased | `LifeChange` |
| Thing appeared, `from_zone == Library (2)`, new zone == `Hand (1)` | `DrawCard` |
| Thing appeared on `Stack (8)` | `CastSpell` |
| Thing moved from `Stack` to `Battlefield (7)` | `ZoneTransition` (stack→battlefield, or `PlayLand` if zone type is land — determined by card type, future work) |
| Thing zone changed (general) | `ZoneTransition` |
| `ThingState.attacking` became true | `Attack` |

Player IDs in emitted actions are looked up from `player_names[seat]`.

---

## Key MagicProperty Constants

Defined in `opcodes.rs` alongside opcode constants:

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
| `POWER` | 538054425 | Creature power |
| `TOUGHNESS` | 538065157 | Creature toughness |

---

## Error Handling

All parsing functions return `Result<T, DecodeError>` using `thiserror`. `DecodeError` variants:

- `Io(std::io::Error)` — stream read failure
- `UnexpectedEof { context: &'static str }` — buffer too short
- `InvalidChecksum { expected: i32, got: i32 }` — StateBuf checksum mismatch
- `UnknownElementType(u32)` — unrecognised StateElementType (non-fatal: stored as `Other`)
- `UnknownPropertyType(u32)` — unrecognised property type tag (non-fatal: skip entry)

Unknown element/property types are treated as non-fatal so a single unrecognised field doesn't abort an entire game decode.

---

## Testing Strategy

- **framing.rs**: unit-test round-trip with hand-crafted byte slices
- **fls.rs**: unit-test with minimal hand-crafted GsMessageMessage payloads
- **statebuf**: unit-test diff (`ApplyDiffs2`) with known input/output pairs; unit-test element parsing with hand-crafted PropertyContainer bytes
- **state.rs**: unit-test `apply_elements` with synthetic element sequences
- **translator.rs**: unit-test diff logic with synthetic before/after `GameState` pairs
- **Integration**: end-to-end test with a real captured (decrypted) dump file once TLS is resolved
