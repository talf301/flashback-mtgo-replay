# Flashback MTGOSDK Redesign

## Overview

Redesign Flashback to use [MTGOSDK](https://github.com/videre-project/MTGOSDK) instead of reverse-engineering the MTGO network protocol. MTGOSDK attaches to the live MTGO process via .NET memory inspection, providing direct access to game state, events, card data, and deck lists. This eliminates the entire 6-layer protocol decode pipeline (~5,000 lines of Rust) and replaces it with a C# capture service of ~500-1000 lines.

## Goals

- Record MTGO games into shareable `.flashback` replay files using MTGOSDK as the data source
- Produce richer replays than the wire-protocol approach (combat pairings, exact zones, mana pools, counter types, deck lists, sideboard changes)
- Keep the existing React/TypeScript web viewer, updated for the new format
- System tray app for set-and-forget recording

## Non-Goals

- Live view / WebSocket streaming (future consideration, not this iteration)
- Match-level grouping (Bo3 as a unit) — each game is a separate file
- MTGO's built-in replay integration
- Preserving the Rust codebase or network protocol approach
- Mobile support

## Architecture

Two independent components connected by `.flashback` files on disk:

```
┌─────────────────────┐                          ┌────────────────────┐
│  Flashback Recorder │                          │  Flashback Viewer  │
│  (C# / .NET)        │     .flashback files     │  (React / TS)      │
│  System tray app     │ ──────── on disk ───────▶│  Local dev server  │
│  Windows only        │                          │  Any platform      │
└──────────┬──────────┘                          └────────────────────┘
           │
    MTGOSDK (memory inspection)
           │
      ┌────▼────┐
      │  MTGO   │
      └─────────┘
```

No server, no database, no network communication between components. The `.flashback` file is the only interface.

## Recorder Architecture

### Three Internal Layers

**1. MTGOSDK Integration Layer**
- Attaches to the MTGO process on startup, or waits for it to launch
- Subscribes to event callbacks:
  - `OnZoneChange` — card movement between zones
  - `OnGameAction` — spells cast, abilities activated
  - `OnLifeChange` — life total updates
  - `OnGamePhaseChange` — phase transitions
  - `CurrentTurnChanged` — turn changes
  - `GameStatusChanged` — game start/end
- Polls full game state at each turn start to produce keyframe snapshots

**2. Game Session Manager**
- Listens for `GameStatusChanged` to detect game start/end
- Creates a new recording session per game
- On game start: captures the deck list (mainboard + sideboard)
- Accumulates events between keyframes
- On game end: assembles the final replay and hands it to the file writer
- Handles edge cases:
  - MTGO crash mid-game: saves partial replay with `"complete": false`
  - Player concession: normal game end, recorded as such
  - Disconnect: same as crash handling

**3. File Writer**
- Serializes the replay to JSON (`.flashback` format v3)
- Writes to the configured output directory (default: `%APPDATA%/Flashback/replays/`)
- Output directory is configurable via config file
- File naming: `YYYY-MM-DD_Player1-vs-Player2_GameID_gN.flashback`
  - `gN` suffix (g1, g2, g3) for multi-game matches so files sort together

### System Tray UI

Minimal:
- **Tray icon color**: grey (waiting for MTGO), green (recording), red (error)
- **Right-click menu**: Open replay folder, Settings, Quit
- **Notification on game recorded**: "Game saved: Alice vs Bob"

## Replay Format v3

Hybrid format: periodic full snapshots (per turn start) with events interleaved between them.

### Top-Level Structure

```json
{
  "version": "3.0",
  "header": {
    "game_id": 12345,
    "players": [
      { "name": "Alice", "seat": 0 },
      { "name": "Bob", "seat": 1 }
    ],
    "format": "Modern",
    "start_time": "2026-03-31T10:00:00Z",
    "end_time": "2026-03-31T10:15:00Z",
    "result": { "winner": "Alice", "reason": "concession" },
    "complete": true,
    "decklist": {
      "mainboard": ["Lightning Bolt", "Lightning Bolt", "Snapcaster Mage", "..."],
      "sideboard": ["Rest in Peace", "Wear // Tear", "..."]
    },
    "sideboard_changes": null
  },
  "timeline": [
    { "type": "snapshot", "turn": 1, "phase": "precombat_main", "active_player": "Alice", "state": {} },
    { "type": "event", "turn": 1, "phase": "precombat_main", "active_player": "Alice", "event": {} },
    { "type": "snapshot", "turn": 2, "phase": "untap", "active_player": "Bob", "state": {} }
  ],
  "card_catalog": {
    "12345": { "name": "Lightning Bolt", "mana_cost": "{R}", "type_line": "Instant" }
  }
}
```

### Design Choices

- **Single `timeline` array**: snapshots and events interleaved in chronological order. Viewer walks the array — snapshots set state directly, events apply incrementally. Seeking to turn N means jumping to that turn's snapshot.
- **Card catalog**: card metadata stored once, referenced by ID throughout. MTGOSDK provides names, mana costs, types. Viewer uses Scryfall only for card images.
- **Sideboard changes**: always diffed against the game 1 deck list, not the previous game. Shows "what did I board in for this matchup."
- **Game 1** has `sideboard_changes: null`. Games 2+ include the diff from game 1's list (e.g., `{ "in": ["Rest in Peace"], "out": ["Lightning Bolt"] }`).

### Snapshot Content

Full board state at a point in time:

- Per-player zones: hand, library (count only), battlefield, graveyard, exile
- Per-player: life total, mana pool contents
- Active player, priority player
- Each object includes:
  - Card catalog ID
  - Tapped / flipped / face-down state
  - Power / toughness (current, not base)
  - Damage marked
  - Counters by type (e.g., `{ "+1/+1": 2, "loyalty": 4 }`)
  - Attachments (list of attached object IDs)
  - Combat status (attacking, blocking, attack/block targets)
  - Summoning sickness
  - Controller (if different from owner)

### Event Types

Same action types from v1, enriched with MTGOSDK data:

| Event | New Data from MTGOSDK |
|-------|----------------------|
| `CastSpell` | Exact source zone, ability text |
| `Attack` | Attack target (player or planeswalker) |
| `Block` | Blocking target (which attacker) |
| `ZoneTransition` | Exact source and destination zones (no "unknown") |
| `CounterUpdate` | Counter type, not just count |
| `ActivateAbility` | Ability text |
| `LifeChange` | Source of life change |

All existing event types carry over: `DrawCard`, `PlayLand`, `CastSpell`, `ActivateAbility`, `Resolve`, `ZoneTransition`, `Attack`, `Block`, `LifeChange`, `TapPermanent`, `UntapPermanent`, `DamageMarked`, `SummoningSickness`, `FaceDown`, `FaceUp`, `Attach`, `Detach`, `CounterUpdate`, `PowerToughnessUpdate`, `Discard`, `Mill`, `CreateToken`, `TurnChange`, `PhaseChange`.

### File Size Estimates

| Component | Per Game |
|-----------|----------|
| Snapshots (~15 per game, ~12 KB each) | ~180 KB |
| Events (~60-80 per game, ~200 bytes each) | ~16 KB |
| Header + catalog | ~5 KB |
| **Total** | **~200 KB** |
| **Bo3 match (3 files)** | **~600 KB** |

Small enough to share over Discord or similar.

## Viewer Changes

The existing React/TypeScript/Vite web viewer is updated, not rewritten.

### What Changes

- **TypeScript types** updated to match v3 format
- **Reconstruction engine** simplified:
  - Walk timeline array
  - On snapshot: replace current state wholesale
  - On event: apply incremental update
  - Seeking: binary search for nearest snapshot, then replay events forward
- **New UI elements**:
  - Combat pairings (lines or grouping between attackers and blockers)
  - Mana pool display
  - Counter types on cards
  - Deck list panel (with sideboard changes for games 2+)
- **Removed**:
  - Client-side Scryfall name resolution (card catalog has names)
  - Heuristic state reconstruction logic (snapshots are authoritative)

### What Stays the Same

- File loading (drag-and-drop / file picker)
- Zone layout (battlefield, hand, graveyard, exile, stack per player)
- Scryfall for card images
- Playback controls (forward / back / play / pause / speed)
- Game log sidebar

## Error Handling

| Scenario | Behavior |
|----------|----------|
| MTGO not running | Recorder waits in grey/idle mode, polls for process, attaches when found |
| MTGOSDK attachment failure | Retry with backoff, 3 failures → tray notification with error, retry from menu |
| Missed event callback | Next turn's snapshot corrects state; worst case: lost detail between snapshots, state always correct |
| MTGO crash mid-game | Save partial replay with `"complete": false` |
| Multi-game match | Each game → separate file, `_g1`/`_g2`/`_g3` suffix |
| Sideboarding phase | Ignored (no game state). Recording resumes on next game start. Deck list captured at game start. |

## Testing Strategy

### Recorder
- Unit tests for Game Session Manager: event accumulation, snapshot merging, file assembly using mock MTGOSDK events
- Integration test: scripted event + snapshot sequence → verify output `.flashback` JSON
- No tests requiring a live MTGO client — mock the SDK interface

### Viewer
- Existing test suite updated for v3 schema types
- New tests for simplified reconstruction engine (snapshot + event application)
- Seek behavior tests (jump to snapshot, replay events forward)
- Hand-crafted v3 fixture files

### Format Validation
- JSON schema definition for v3 format
- Validation on both write (recorder) and read (viewer) sides

## What Gets Deleted

The entire Rust codebase (~6,000 lines):

| Module | Lines | Reason |
|--------|-------|--------|
| `capture/` | 350 | Replaced by MTGOSDK process attachment |
| `protocol/framing.rs` | 159 | No wire protocol |
| `protocol/fls.rs` | 280 | No wire protocol |
| `protocol/game_messages.rs` | 352 | No wire protocol |
| `protocol/statebuf.rs` | 1,031 | No wire protocol |
| `protocol/opcodes.rs` | 98 | No wire protocol |
| `state.rs` | 536 | MTGOSDK has richer model |
| `translator.rs` | 1,216 | MTGOSDK fires events directly |
| `chat.rs` | 245 | MTGOSDK has authoritative data |
| `scryfall.rs` | 101 | MTGOSDK provides card metadata |
| `replay/schema.rs` | 389 | Replaced by v3 format |
| `bin/decode.rs` | 960 | No decode pipeline |

The `web/` directory is kept and updated.
