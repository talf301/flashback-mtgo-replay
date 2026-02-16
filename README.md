# Flashback — MTGO Replay System

A passive replay system for Magic: The Gathering Online (MTGO) that captures network packets during live games, decodes them into structured events, and stores compact replay logs. Includes a web-based viewer for stepping through replays with visual board representation.

## Overview

**Flashback** consists of three main components:

- **Capture Agent** (Rust) - Runs on Windows alongside MTGO, sniffing packets and decoding the protocol
- **Replay Store** (JSON) - Compact replay format storing ordered game actions
- **Web Viewer** (TypeScript + React) - Reads replay files and reconstructs board state

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
- **Rust** 1.70+ for building the capture agent
- **Node.js** 18+ for building the web viewer
- **npm** for managing web dependencies

### Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd mtgo-replay-omp
```

2. Install dependencies:

```bash
# Rust (capture agent)
cargo build --release

# Web viewer
cd web
npm install
npm run build
```

### Usage

#### Capturing Replays

1. Install [npCap](https://nmap.org/npcap/) on Windows
2. Run the capture agent with administrative privileges:

```bash
# List available network interfaces
flashback

# Start capture on specific interface
flashback <interface> <output_file>

# Example
flashback "\\Device\\NPF_{GUID}" my_replay.pcap
```

3. Play MTGO while the capture agent is running
4. Press Ctrl+C to stop capture

#### Viewing Replays

1. Start the web viewer:

```bash
cd web
npm run dev
```

2. Open http://localhost:5173 in your browser

3. Load a replay file (drag-and-drop or use the file picker)
4. Use controls to step through the replay:
   - Arrow keys or buttons to step forward/backward
   - Slider to jump to any point in the replay
   - Play/Pause button for automatic playback
   - Speed controls (0.5x, 1x, 2x)

## Architecture

```
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│  Capture Agent   │ ──▶  │  Replay Store    │ ◀──  │  Web Viewer      │
│  (Rust, Windows) │      │  (JSON/binary)   │      │  (TS + React)    │
└─────────────────┘      └─────────────────┘      └─────────────────┘
        │                                                  │
   npcap/raw sockets                                  Scryfall API
        │                                           (card images/data)
   ┌────▼────┐
   │  MTGO   │
   └─────────┘
```

## Project Structure

```
mtgo-replay-omp/
├── src/
│   ├── capture/          # Packet capture and dumping
│   │   ├── mtgo.rs       # MTGO-specific capture logic
│   │   ├── pcap.rs       # Low-level packet capture
│   │   └── dumper.rs     # Packet file I/O
│   ├── protocol/         # MTGO protocol decoding
│   │   ├── decoder.rs    # Event decoding
│   │   └── raw_analyzer.rs  # Packet pattern analysis
│   ├── replay/           # Replay file handling
│   │   ├── schema.rs     # Data structures
│   │   ├── boundary.rs   # Game boundary detection
│   │   └── writer.rs     # Replay file writer
│   └── main.rs           # CLI entry point
├── web/                  # Web viewer
│   ├── src/
│   │   ├── components/   # React components
│   │   ├── engine/       # Game state reconstruction
│   │   ├── types/        # TypeScript types
│   │   └── api/          # External API calls
│   └── package.json
└── tests/                # Integration tests
```

## Replay Format

Replay files are stored as JSON with the following structure:

```json
{
  "header": {
    "game_id": "game-001",
    "players": [
      {
        "player_id": "player1",
        "name": "Alice",
        "life_total": 20
      }
    ],
    "format": "Standard",
    "start_time": "2024-01-01T10:00:00Z",
    "end_time": "2024-01-01T10:15:00Z",
    "result": {
      "Win": {
        "winner_id": "player1"
      }
    }
  },
  "actions": [
    {
      "timestamp": "2024-01-01T10:00:00Z",
      "turn": 1,
      "phase": "upkeep",
      "active_player": "player1",
      "action_type": {
        "DrawCard": {
          "player_id": "player1",
          "card_id": "forest-001"
        }
      }
    }
  ],
  "metadata": {
    "version": "1.0",
    "recorder": "flashback"
  }
}
```

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for detailed development guidelines, testing, and code style.

## Documentation

- [PROTOCOL_RESEARCH.md](PROTOCOL_RESEARCH.md) - MTGO protocol research findings
- [web/README.md](web/README.md) - Web viewer setup and architecture

## License

MIT License - see LICENSE file for details

## Contributing

Contributions are welcome! Please see [DEVELOPMENT.md](DEVELOPMENT.md) for guidelines.

## Status

This is an active development project. The capture agent can capture and dump raw packets, the replay format is defined, and the web viewer has basic functionality. Full protocol decoding is ongoing.
