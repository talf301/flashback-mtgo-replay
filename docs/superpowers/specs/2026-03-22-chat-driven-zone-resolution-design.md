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
    CreateToken { player: String, source_card: ChatCardRef, token_name: String },
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

New fields on `ReplayTranslator`:

```rust
/// Per-thing-id chat context for zone resolution and action classification.
chat_context: HashMap<u32, ChatEvent>,
/// Pending token creation events, keyed by token name (e.g., "Goblin Shaman Token").
/// When a new thing appears on the battlefield with IS_TOKEN set and its card name
/// matches a pending token entry, it is classified as CreateToken.
pending_tokens: VecDeque<(String, ChatEvent)>,
```

New public method:

```rust
pub fn ingest_chat(&mut self, text: &str)
```

Parses the chat text via `chat.rs`. For card-targeted events (Discard, PutIntoGraveyard, Exile, PutOnLibrary), inserts into `chat_context` keyed by the card's thing_id. For `CreateToken` events, pushes onto `pending_tokens` keyed by the token name string.

Called from `decode.rs` on every `UserChat` message, placed **before** the existing turn-ownership parsing.

**Resolution order** when processing a new thing with `from_zone == Some(-1)`:

1. `chat_context.remove(&thing_id)` — authoritative source zone + action type from chat
2. `last_known_zones.get(&thing_id)` — source zone from prior state tracking
3. Fallback — "unknown" (existing behavior, should become rare)

**Token matching** when a new thing appears on the battlefield with `IS_TOKEN` property set:

1. Check `pending_tokens` for an entry whose token name matches the thing's card name
2. If found, emit `ActionType::CreateToken` with the token's thing_id as `card_id` and the token name
3. If not found, fall through to existing logic (generic ZoneTransition or PlayLand)

`chat_context` and `pending_tokens` are cleared on `reset()`.

**Action classification from chat context:**

When `chat_context` provides a `ChatEvent` for a thing_id, the translator uses it to determine both `from_zone` and `ActionType`:

| ChatEvent | Inferred from_zone | ActionType |
|---|---|---|
| `Discard` | hand | `Discard` |
| `PutIntoGraveyard` | library (if `last_known_zones` absent or shows library) | `Mill` |
| `PutIntoGraveyard` | other (if `last_known_zones` shows non-library zone) | `ZoneTransition` with resolved from_zone |
| `Exile` | `last_known_zones` value, or "unknown" | `ZoneTransition` with resolved from_zone |
| `PutOnLibrary` | `last_known_zones` value, or "unknown" | `ZoneTransition` with resolved from_zone |

This means `PutIntoGraveyard` only becomes `Mill` when the source is library. If the card was on the battlefield (e.g., destroyed), it remains a `ZoneTransition` with `from_zone: "battlefield"`. The chat verb "puts X into their graveyard" is ambiguous — the source zone disambiguates.

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

## Notes

- **Exile and PutOnLibrary** chat events resolve `from_zone` on existing `ZoneTransition` actions — they do not produce new ActionType variants. PutOnLibrary may not be observable in state diffs (library contents are hidden), so it primarily serves as from_zone context.
- **Chat resolution only applies to the "new thing" code path** in the translator (things with `from_zone == Some(-1)` not present in prev state). Existing things that change zones already have their old zone tracked directly.
- **Player names** in chat events are raw strings matching the `player_names` vec on the translator. The translator already resolves names to `player_id` strings via `player_name()`.
- **`@P` prefix** is already stripped from UserChat text by `decode_user_chat()` — the chat parser receives clean text.

## What This Doesn't Cover

- **Sacrifice vs Destroy**: Both are battlefield → graveyard. Chat sometimes says "sacrifices" but distinguishing isn't critical yet.
- **Scry**: Chat says "puts N cards on top/bottom" but doesn't reference individual card thing_ids.
- **Counter spell**: Countering is just a spell effect resolving — not a distinct action type.
