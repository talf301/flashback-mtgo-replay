# Flashback — MTGO Replay System

A replay system for Magic: The Gathering Online (MTGO) that captures game state during live games using [MTGOSDK](https://github.com/videre-project/MTGOSDK), records structured replay files, and provides a web-based viewer for stepping through replays.

## Overview

**Flashback** consists of two main components:

- **Flashback Recorder** (C# / .NET) — System tray app that attaches to the MTGO process via MTGOSDK memory inspection, captures game events and state snapshots, and writes `.flashback` replay files to disk
- **Flashback Viewer** (TypeScript + React) — Loads replay files, reconstructs board state from snapshots and events, and renders an interactive game view

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

### Goals

- Record MTGO games into shareable `.flashback` replay files using MTGOSDK as the data source
- Produce rich replays with combat pairings, exact zones, mana pools, counter types, deck lists, and sideboard changes
- Provide a web-based replay viewer with zone layout, card images, and playback controls
- System tray app for set-and-forget recording

### Non-Goals

- Live view / WebSocket streaming (future consideration)
- Match-level grouping (Bo3 as a unit) — each game is a separate file
- Chat log capture
- Replay sharing / upload / hosted service
- Mobile support

## Quick Start

### Prerequisites

- **Windows** for the recorder (MTGO + MTGOSDK require Windows)
- **.NET 8+** for building the recorder
- **Node.js** 18+ for building the web viewer
- **npm** for managing web dependencies

### Viewing Replays

1. Start the web viewer:

```bash
cd web
npm install
npm run dev
```

2. Open http://localhost:5173 in your browser
3. Load a `.flashback` file (drag-and-drop or file picker)
4. Use controls to step through the replay:
   - Forward/backward buttons or slider to navigate
   - Play/Pause for automatic playback
   - Speed controls (0.25x to 4x)

## Replay Format (.flashback v3)

Hybrid format: periodic full snapshots (per turn start) with events interleaved between them.

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
      "mainboard": ["Lightning Bolt", "Lightning Bolt", "..."],
      "sideboard": ["Rest in Peace", "..."]
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

### Event Types

| Event | Description |
|-------|-------------|
| `DrawCard` | Card moved from library to hand |
| `PlayLand` | Land played from hand to battlefield |
| `CastSpell` | Spell cast (with exact source zone and ability text) |
| `ActivateAbility` | Ability activated (with ability text) |
| `Resolve` | Spell/ability left the stack |
| `ZoneTransition` | Card moved between zones (exact source and destination) |
| `Attack` | Creature declared as attacker (with attack target) |
| `Block` | Creature declared as blocker (with blocking target) |
| `LifeChange` | Player life total changed (with source) |
| `TapPermanent` / `UntapPermanent` | Tap state changed |
| `DamageMarked` | Damage marked on creature |
| `SummoningSickness` | Summoning sickness state changed |
| `FaceDown` / `FaceUp` | Face-down state changed |
| `Attach` / `Detach` | Equipment/aura attachment changed |
| `CounterUpdate` | Counter count changed (with counter type) |
| `PowerToughnessUpdate` | P/T changed |
| `Discard` | Card discarded from hand |
| `Mill` | Card milled from library to graveyard |
| `CreateToken` | Token created on the battlefield |
| `TurnChange` | New turn started |
| `PhaseChange` | Game phase changed |

## Project Structure

```
flashback/
├── recorder/              # C# / .NET recorder (MTGOSDK-based)
│   └── (in development)
├── web/                   # Web viewer (React + TypeScript + Vite)
│   └── src/
│       ├── components/    # Board, Card, Zone, GameLog, ReplayControls, FileLoader
│       ├── engine/        # State reconstruction from snapshots + events
│       ├── types/         # TypeScript types matching v3 schema
│       └── api/           # Scryfall API integration (card images)
├── tools/
│   ├── capture-hook/      # .NET DLL injection for TLS decryption
│   └── opcode-dump/       # MTGO opcode table extraction
├── docs/
│   └── specs/             # Architecture specs and design documents
└── DEVELOPMENT.md         # Dev guide
```

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for testing, code style, and contribution guidelines.

## Documentation

- [Design Spec](docs/superpowers/specs/2026-03-31-flashback-mtgosdk-redesign.md) — Full MTGOSDK redesign specification
