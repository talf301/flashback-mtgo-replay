# Flashback — MTGO Replay System
## Product Requirements Document

### 1. Overview

**Flashback** is a replay system for Magic: The Gathering Online (MTGO). It passively captures network packets during live MTGO games, decodes them into structured game events, and stores compact replay logs. A web-based viewer allows stepping through replays with a visual board representation.

### 2. Goals

- Passively capture MTGO game data via packet sniffing (no game modification)
- Decode MTGO's network protocol into structured, human-readable game events
- Store replays as compact action logs from which full board state can be reconstructed at any point
- Provide a web-based replay viewer with zone layout (battlefield, hand, graveyard, exile, stack), card images via Scryfall, and step-through controls
- Capture individual games as separate replay files

### 3. Non-Goals (for now)

- Chat log capture
- Match-level grouping (Bo3 as a unit)
- Replay sharing / upload / hosted service
- Format-aware features (legality, archetypes)
- Full graphical fidelity / animations resembling MTGO's UI
- Mobile support

### 4. Architecture

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

**Three components:**

| Component | Language | Role |
|-----------|----------|------|
| **Capture Agent** | Rust | Runs on Windows alongside MTGO. Sniffs packets via npcap, decodes the protocol, emits structured game events, writes replay files. |
| **Replay Store** | File-based | Compact replay format storing ordered game actions. One file per game. |
| **Web Viewer** | TypeScript + React | Reads replay files, reconstructs board state from actions, renders zones with card images from Scryfall. Runs locally. |

### 5. Component Details

#### 5.1 Capture Agent (Rust)

**Responsibilities:**
- Listen on the network interface for MTGO traffic (identify by destination IP/port)
- Reassemble TCP streams
- Decode MTGO protocol messages into game events
- Detect game boundaries (game start / game end)
- Write one replay file per game

**Key dependencies:**
- `pcap` or `pnet` crate for packet capture
- npcap driver (must be installed on the Windows machine)

**Protocol research phase:**
- This is the primary R&D risk. MTGO's protocol is undocumented.
- Initial work involves capturing raw packets, identifying patterns, and iteratively building a decoder.
- The agent should have a "raw dump" mode that saves unprocessed packet data for offline analysis.

#### 5.2 Replay Format

**Design principles:**
- Action-sourced: store the sequence of game actions, not snapshots of board state
- Compact: one file per game, small enough to eventually share
- Self-contained: include enough metadata to reconstruct state without external lookups (card IDs reference Scryfall)

**Stored data per game:**
- Header: players, format, date/time, game result
- Ordered list of game actions/events, each with:
  - Turn number and phase
  - Acting player
  - Action type (draw, play land, cast spell, activate ability, attack, block, resolve, etc.)
  - Card identifier (Scryfall ID or MTGO card ID + mapping)
  - Zone transitions (hand → battlefield, battlefield → graveyard, etc.)
  - Life total changes
  - Any revealed hidden information (opponent's revealed cards)
  - Your hand contents at each point

**Format:** JSON for MVP (easy to debug), with option to move to a binary format later for compactness.

#### 5.3 Web Viewer (TypeScript + React)

**Reference:** 17lands replay viewer style — zone-based board layout with card images and step-through controls.

**Features:**
- Load a replay file (drag-and-drop or file picker)
- Reconstruct board state at any point from the action log
- Display zones: battlefield (grouped by player), hand, graveyard, exile, stack
- Show card images fetched from Scryfall API
- Step forward / backward through actions
- Turn and phase indicators
- Life total display
- Game log sidebar showing the action list with current position highlighted

**State reconstruction engine:**
- A pure function: `(actions[0..N]) → BoardState`
- BoardState includes: each zone's contents, life totals, turn/phase, active player
- This engine should be shared logic that can be tested independently

### 6. Data Flow

```
MTGO ──packets──▶ Capture Agent ──decode──▶ Game Events ──serialize──▶ .flashback file
                                                                           │
Web Viewer ◀──load file──────────────────────────────────────────────────────┘
     │
     ├──▶ State Reconstruction Engine ──▶ BoardState @ action N
     │
     └──▶ Scryfall API ──▶ Card images + oracle text
```

### 7. Card Data

- Use **Scryfall API** for card images, oracle text, and metadata
- Cache Scryfall responses locally to avoid redundant API calls and respect rate limits
- Replay files store card identifiers (Scryfall ID preferred, MTGO ID as fallback with a mapping)

### 8. Milestones

#### Milestone 1: Protocol Research & Raw Capture (MVP)
> *First demo: successfully sniff and decode MTGO packets into readable game events*

- [ ] Set up Rust project with pcap/npcap integration
- [ ] Capture raw MTGO traffic and save packet dumps
- [ ] Analyze protocol: identify message boundaries, encoding format, key message types
- [ ] Build initial decoder for core game events (game start, draw, play, cast, attack, block, damage, life change, game end)
- [ ] Output decoded events to stdout / a JSON log
- [ ] Document discovered protocol details

#### Milestone 2: Replay File Generation
- [ ] Define the `.flashback` replay file format (JSON schema)
- [ ] Detect game boundaries automatically
- [ ] Write one replay file per game during capture
- [ ] Include all required metadata (players, result, timestamps)
- [ ] Validate replay files contain enough data to reconstruct board state

#### Milestone 3: State Reconstruction Engine
- [ ] Build the state reconstruction engine (can be in TS for the web viewer)
- [ ] Input: ordered action list, position N → Output: complete board state
- [ ] Handle all zone transitions
- [ ] Handle tokens, counters, and copies
- [ ] Unit tests with hand-crafted replay data

#### Milestone 4: Web Viewer
- [ ] Basic React app that loads `.flashback` files
- [ ] Render board state: battlefield, hand, graveyard, exile, stack
- [ ] Card images from Scryfall
- [ ] Step forward / backward controls
- [ ] Turn/phase indicator and life totals
- [ ] Game log sidebar

### 9. Technical Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| MTGO protocol is encrypted (TLS) | **High** | If TLS, may need MITM proxy approach instead of raw sniffing. Research first. |
| Protocol is too complex to reverse-engineer | **Medium** | Start with common events (draw, play, cast). Partial decoding is still useful. |
| MTGO updates break the decoder | **Medium** | Version the protocol decoder. Keep raw dump mode for re-analysis. |
| Scryfall rate limits | **Low** | Local caching with TTL. Bulk data download for card DB. |
| npcap driver conflicts | **Low** | Document setup requirements. Test with Wireshark first. |

### 10. Open Questions

1. **Is MTGO traffic encrypted?** This is the first thing to determine — it dictates the entire capture approach.
2. **What transport does MTGO use?** (TCP, UDP, WebSocket, HTTP?)
3. **What encoding format?** (protobuf, XML, binary, JSON?)
4. **Are there existing community efforts** at MTGO protocol analysis to build on?
5. **Does MTGO write any local log files** that could supplement or replace packet capture?

### 11. Success Criteria

**MVP (Milestone 1) is successful when:**
- The capture agent can run alongside MTGO without interfering with gameplay
- At least the following events are decoded from packets: game start, card draw, spell cast, land play, attack declaration, life total change, game end
- Decoded events are output as structured JSON that a human can read and follow along with what happened in the game