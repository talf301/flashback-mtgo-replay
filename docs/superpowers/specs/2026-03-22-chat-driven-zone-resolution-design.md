# Chat-Driven Zone Resolution & Action Type Enrichment

## Problem

Zone transitions for things appearing for the first time in the game state show `from_zone: "unknown"`. The MTGO protocol's `from_zone` field on `ThingElement` is an object reference (not a zone enum), so when a thing hasn't been seen before, we know it moved but not from where.

In the golden file: 37 transitions affected — 32 `unknown → revealed` (surveil/scry reveals) and 5 `unknown → effects` (spells entering an internal zone). Additionally, several action types (Discard, Mill, CreateToken) are documented as missing in KNOWN_ISSUES.md.

## Solution

Parse MTGO chat log messages to build a per-thing-id context that the translator consults when resolving zone transitions and classifying actions. Chat messages arrive **before** the corresponding state diffs and contain structured card references with exact thing IDs in the format `@[CardName@:textureId,thingId:@]`.

This follows the same pattern used successfully for turn ownership resolution via `NEW_USER_CHAT` messages.

## Components

### 1. Chat Parser (`src/chat.rs`)

New module that parses raw chat text into structured events.

**Card reference extraction:** Regex `@\[([^@]+)@:(\d+),(\d+):@\]` extracts `(name, texture_id, thing_id)`.

```rust
pub struct ChatCardRef {
    pub name: String,
    pub texture_id: u32,
    pub thing_id: u32,
}

pub enum LibPos { Top, Bottom }

pub enum ChatEvent {
    Discard { player: String, card: ChatCardRef },
    PutIntoGraveyard { player: String, card: ChatCardRef },
    Exile { player: String, card: ChatCardRef },
    PutOnLibrary { player: String, card: ChatCardRef, position: LibPos },
    CreateToken { player: String, token_name: String },
}
```

**Matched patterns:**

| Chat pattern | ChatEvent |
|---|---|
| `"{player} discards @[Card@:tex,id:@]"` | `Discard` |
| `"{player} puts @[Card@:tex,id:@] into their graveyard"` | `PutIntoGraveyard` |
| `"{player} exiles @[Card@:tex,id:@]"` | `Exile` |
| `"{player} puts @[Card@:tex,id:@] on top/bottom of their library"` | `PutOnLibrary` |
| `"{player}'s @[Card@:tex,id:@] creates a {TokenName}"` | `CreateToken` |

Unrecognized chat is ignored — returns `None`.

### 2. Chat Context in Translator (`src/translator.rs`)

New field on `ReplayTranslator`:

```rust
chat_context: HashMap<u32, ChatEvent>,
```

New public method:

```rust
pub fn ingest_chat(&mut self, text: &str)
```

Parses the chat text via `chat.rs`, inserts result keyed by thing_id. Called from `decode.rs` on every `UserChat` message — single line addition to the existing match arm.

**Resolution order** when processing a new thing with `from_zone == Some(-1)`:

1. `chat_context.remove(&thing_id)` — authoritative source zone + action type from chat
2. `last_known_zones.get(&thing_id)` — source zone from prior state tracking
3. Fallback — "unknown" (existing behavior, should become rare)

`chat_context` is cleared on `reset()`.

### 3. New ActionType Variants (`src/replay/schema.rs`)

```rust
Discard { player_id: String, card_id: String },
Mill { player_id: String, card_id: String },
CreateToken { player_id: String, card_id: String, token_name: String },
```

- **Discard**: chat says "discards", from hand → graveyard
- **Mill**: chat says "puts X into their graveyard" (surveil/mill context), from library → graveyard
- **CreateToken**: chat says "creates a TokenName", new thing on battlefield

These replace what would otherwise be generic `ZoneTransition` actions.

### 4. Integration in decode.rs

Single line addition to the `UserChat` match arm:

```rust
GameMessage::UserChat { ref text } => {
    tracing::debug!("CHAT: {}", text);
    translator.ingest_chat(text);  // <-- new
    // existing turn ownership parsing unchanged
}
```

### 5. Web Viewer Updates

TypeScript types (`web/src/types/`) gain the three new `ActionType` variants:

- `Discard` — `{ player_id: string, card_id: string }`
- `Mill` — `{ player_id: string, card_id: string }`
- `CreateToken` — `{ player_id: string, card_id: string, token_name: string }`

`Reconstructor` gets zone-transition logic for each:
- Discard: hand → graveyard
- Mill: library → graveyard
- CreateToken: add to battlefield

Display component gets labels for each. Existing `ZoneTransition` rendering works as fallback.

### 6. Existing ZoneTransition Improvements

For transitions that don't map to new action types but still have `from_zone: "unknown"`:

- Chat context resolves the source zone (e.g., "exiles @[Card]" with `last_known_zones` showing battlefield → `from_zone: "battlefield"`)
- Surveil-context cards appearing in "revealed" zone → `from_zone: "library"`
- Things previously tracked by `last_known_zones` get their actual last zone

## Data Flow

```
MTGO stream
    │
    ├── UserChat messages ──► translator.ingest_chat(text)
    │                              │
    │                              ▼
    │                         chat.rs parser
    │                              │
    │                              ▼
    │                     chat_context: HashMap<thing_id, ChatEvent>
    │
    ├── GamePlayStatus ──► state.apply_elements() ──► translator.process()
    │                                                       │
    │                                          ┌────────────┤
    │                                          ▼            ▼
    │                                   chat_context   last_known_zones
    │                                          │            │
    │                                          ▼            ▼
    │                                   Resolved from_zone + ActionType
    │
    ▼
  ReplayAction list (enriched)
```

## Test Strategy

- **Unit tests for chat parser**: Each pattern with expected `ChatEvent` output, plus unrecognized input returning `None`
- **Unit tests for translator**: Mock state diffs with chat context pre-loaded, verify correct `from_zone` and `ActionType`
- **Golden file regression**: Re-run pipeline on `golden_v1.bin`, verify zero `from_zone: "unknown"` (or near-zero), verify new action types appear where expected
- **Web viewer**: Existing tests updated for new variants; verify new types render labels

## What This Doesn't Cover

- **Sacrifice vs Destroy**: Both are battlefield → graveyard. Chat sometimes says "sacrifices" but distinguishing isn't critical yet.
- **Scry**: Chat says "puts N cards on top/bottom" but doesn't reference individual card thing_ids.
- **Counter spell**: Countering is just a spell effect resolving — not a distinct action type.
