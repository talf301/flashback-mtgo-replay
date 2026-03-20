# Flashback — MTGO Replay System

A passive replay system for Magic: The Gathering Online (MTGO) that captures network packets during live games, decodes them into structured events, and stores compact replay logs. Includes a web-based viewer for stepping through replays with visual board representation.

## Overview

**Flashback** consists of three main components:

- **Capture Agent** (Rust) - Runs on Windows alongside MTGO, sniffing packets and decoding the protocol
- **Decode Pipeline** (Rust) - Converts raw captured streams into structured replay files
- **Web Viewer** (TypeScript + React) - Loads replay files, reconstructs board state, and renders an interactive game view

### Goals

- Passively capture MTGO game data via packet sniffing (no game modification)
- Decode MTGO's network protocol into structured, human-readable game events
- Store replays as compact action logs from which full board state can be reconstructed
- Provide a web-based replay viewer with zone layout and card images
- Capture individual games as separate replay files

### Non-Goals

- Chat log capture
- Match-level grouping (Bo3 as a unit)
- Replay sharing / upload / hosted service
- Format-aware features (legality, archetypes)
- Full graphical fidelity / animations resembling MTGO's UI
- Mobile support

## Quick Start

### Prerequisites

- **Windows** for capture agent (requires npcap)
- **Rust** 1.70+ for building the capture agent and decoder
- **Node.js** 18+ for building the web viewer
- **npm** for managing web dependencies

### Installation

```bash
git clone <repository-url>
cd mtgo-replay-omp

# Rust (capture agent + decoder)
cargo build --release

# Web viewer
cd web
npm install
```

### Usage

#### Capturing Traffic

1. Install [npcap](https://nmap.org/npcap/) on Windows
2. Run the capture agent with administrative privileges:

```bash
flashback <interface> <output_file>
```

3. Play MTGO while the capture agent is running
4. Press Ctrl+C to stop capture

#### Decoding Replays

Convert a captured stream into a `.flashback` replay file:

```bash
cargo run --bin decode -- <stream_file> --output my_game.flashback

# Just show opcode statistics:
cargo run --bin decode -- <stream_file> --stats-only
```

#### Viewing Replays

1. Start the web viewer:

```bash
cd web
npm run dev
```

2. Open http://localhost:5173 in your browser
3. Load a `.flashback` or `.json` file (drag-and-drop or file picker)
4. Use controls to step through the replay:
   - Forward/backward buttons or slider to navigate
   - Play/Pause for automatic playback
   - Speed controls (0.25x to 4x)

## Architecture

```
Capture              Decode Pipeline                    Web Viewer
───────              ───────────────                    ──────────
npcap ──▶ raw stream ──▶ framing ──▶ FLS envelope
                         ──▶ game messages
                         ──▶ StateBuf assembly + diffs
                         ──▶ state elements
                         ──▶ GameState snapshots
                         ──▶ state diff → actions       ◀── .flashback file
                         ──▶ ReplayFile (.flashback)    ──▶ reconstructor
                                                        ──▶ board rendering
                                                        ──▶ Scryfall (card images)
```

### Decode Pipeline

The decoder transforms raw MTGO traffic through six layers:

| Layer | Module | Input | Output |
|-------|--------|-------|--------|
| 1. Framing | `framing.rs` | Raw TCP bytes | `RawMessage` (opcode + payload) |
| 2. FLS Envelope | `fls.rs` | `RawMessage` | `FlsMessage` (GsMessage, GameStatusChange, etc.) |
| 3. Game Messages | `game_messages.rs` | Inner `meta` bytes | `GameMessage` (GamePlayStatus, GameOver, etc.) |
| 4. StateBuf | `statebuf.rs` | Raw state buffer | Assembled bytes (with diff + checksum) |
| 5. State Elements | `statebuf.rs` | Assembled bytes | `Vec<StateElement>` (Things, PlayerStatus, TurnStep) |
| 6. Translation | `state.rs` + `translator.rs` | State diffs | `Vec<ReplayAction>` |

## Project Structure

```
mtgo-replay-omp/
├── src/
│   ├── capture/              # Packet capture (Windows/npcap)
│   │   ├── mtgo.rs           # MTGO-specific BPF filter
│   │   ├── pcap.rs           # Low-level packet capture
│   │   └── dumper.rs         # Binary stream file I/O
│   ├── protocol/             # MTGO protocol decoding
│   │   ├── framing.rs        # Message framing (length-prefix + opcode)
│   │   ├── fls.rs            # FLS envelope decoding
│   │   ├── game_messages.rs  # Inner game message parsing
│   │   ├── statebuf.rs       # StateBuf assembly, diffs, element parsing
│   │   └── opcodes.rs        # Protocol constants
│   ├── replay/
│   │   └── schema.rs         # ReplayFile JSON schema + serialization
│   ├── state.rs              # GameState model (Things, Players, Phases)
│   ├── translator.rs         # State-diff to ReplayAction translation
│   ├── main.rs               # Capture agent CLI
│   └── bin/
│       └── decode.rs         # Decode pipeline CLI
├── web/                      # Web viewer (React + TypeScript + Vite)
│   └── src/
│       ├── components/       # Board, Card, Zone, GameLog, ReplayControls, FileLoader
│       ├── engine/           # State reconstruction from action log
│       ├── types/            # TypeScript types matching Rust schema
│       └── api/              # Scryfall API integration
├── tests/
│   ├── golden_pipeline.rs    # Full pipeline integration tests
│   └── fixtures/
│       ├── golden_v1.bin     # 12MB captured MTGO stream (Bo3 Modern match)
│       └── golden_v1_replay.json  # Expected pipeline output
├── tools/
│   ├── capture-hook/         # .NET DLL injection for TLS decryption
│   └── opcode-dump/          # MTGO opcode table extraction
├── PROTOCOL_RESEARCH.md      # Full protocol reverse-engineering notes
└── DEVELOPMENT.md            # Dev guide, testing, code style
```

## Replay Format (.flashback)

```json
{
  "header": {
    "game_id": "12345",
    "players": [
      { "player_id": "player_0", "name": "Alice", "life_total": 20 },
      { "player_id": "player_1", "name": "Bob", "life_total": 20 }
    ],
    "format": "Modern",
    "start_time": "2024-01-01T10:00:00Z",
    "end_time": "2024-01-01T10:15:00Z",
    "result": { "Win": { "winner_id": "player_0" } }
  },
  "actions": [
    {
      "timestamp": "2024-01-01T10:00:05Z",
      "turn": 1,
      "phase": "precombat_main",
      "active_player": "player_0",
      "action_type": {
        "PlayLand": { "player_id": "player_0", "card_id": "425" }
      }
    }
  ],
  "metadata": { "version": "1.0", "decoder": "flashback" },
  "card_names": { "425": "Scalding Tarn", "430": "Lightning Bolt" }
}
```

### Action Types

| Action | Description |
|--------|-------------|
| `DrawCard` | Card moved from library to hand |
| `PlayLand` | Land played from hand to battlefield (never touched stack) |
| `CastSpell` | Spell cast (appeared on stack from hand) |
| `ActivateAbility` | Ability activated (new object on stack with SRC_THING_ID) |
| `Resolve` | Spell/ability left the stack |
| `ZoneTransition` | Card moved between zones |
| `Attack` | Creature declared as attacker |
| `Block` | Creature declared as blocker |
| `LifeChange` | Player life total changed |
| `TapPermanent` / `UntapPermanent` | Tap state changed |
| `DamageMarked` | Damage marked on creature |
| `SummoningSickness` | Summoning sickness state changed |
| `FaceDown` / `FaceUp` | Face-down state changed |
| `Attach` / `Detach` | Equipment/aura attachment changed |
| `CounterUpdate` | Counter count changed (+1/+1, loyalty, etc.) |
| `PowerToughnessUpdate` | P/T changed |
| `TurnChange` | New turn started |
| `PhaseChange` | Game phase changed |

### Phases

`untap`, `upkeep`, `draw`, `precombat_main`, `begin_combat`, `declare_attackers`, `declare_blockers`, `combat_damage`, `end_of_combat`, `postcombat_main`, `end_of_turn`, `cleanup`

## Current Status

### What Works

- **Full decode pipeline**: raw MTGO stream → framing → FLS → game messages → StateBuf → state → actions → JSON
- **Golden file validation**: 12MB capture (10,195 messages) decodes to 623 actions across a 3-game Modern Bo3 match
- **Web viewer**: loads .flashback files, reconstructs board state, renders zones/cards/life totals, step-through controls
- **20+ action types** decoded from state diffs
- **Card names** extracted from CARDNAME_STRING property where available

### Known Limitations

See [KNOWN_ISSUES.md](KNOWN_ISSUES.md) for detailed analysis.

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for testing, code style, and contribution guidelines.

## Documentation

- [PROTOCOL_RESEARCH.md](PROTOCOL_RESEARCH.md) — Full MTGO protocol reverse-engineering notes
- [KNOWN_ISSUES.md](KNOWN_ISSUES.md) — Current limitations and planned fixes
- [DEVELOPMENT.md](DEVELOPMENT.md) — Dev guide
