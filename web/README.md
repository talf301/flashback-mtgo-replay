# Web Viewer

The web viewer is a React-based application for viewing MTGO replay files. It reconstructs game state from action logs and provides an interactive interface for stepping through replays.

## Features

- Load replay files (drag-and-drop or file picker)
- Visual board representation with zones (battlefield, hand, graveyard, exile, stack)
- Card images from Scryfall API
- Step-by-step replay navigation
- Keyboard shortcuts (arrow keys, space, home, end)
- Playback speed controls
- Search and filter game log
- Responsive design

## Quick Start

### Installation

```bash
cd web
npm install
```

### Development

```bash
# Start development server
npm run dev

# Open http://localhost:5173
```

### Build for Production

```bash
npm run build
npm run preview
```

## Architecture

```
src/
├── components/       # React components
│   ├── App.tsx       # Main application component
│   ├── Board.tsx     # Game board display
│   ├── Zone.tsx      # Individual zone (battlefield, hand, etc.)
│   ├── Card.tsx      # Card display component
│   ├── GameLog.tsx   # Action log viewer
│   ├── ReplayControls.tsx  # Playback controls
│   └── FileLoader.tsx      # File loading interface
├── engine/           # Game state reconstruction
│   └── GameState.tsx # State management logic
├── types/            # TypeScript type definitions
│   └── replay.ts     # Replay file types
├── api/              # External API calls
│   └── scryfall.ts   # Scryfall API integration
├── App.test.tsx      # Component tests
└── App.e2e.test.tsx  # End-to-end tests
```

## Components

### App

Main application component that manages overall state and coordinates between sub-components.

**State:**
- `replayFile`: Loaded replay data
- `currentStep`: Current position in action sequence
- `isPlaying`: Playback state
- `playbackSpeed`: Playback speed multiplier
- `searchQuery`: Game log search query

### Board

Displays the game board with all zones for all players. Layout:

```
┌─────────────────────────────────────────┐
│  Player 2 Info                          │
│  ┌──────────┐  ┌──────────┐           │
│  │ Hand     │  │ Graveyard │           │
│  └──────────┘  └──────────┘           │
│                                         │
│  ┌──────────────────────────┐          │
│  │     Stack                │          │
│  └──────────────────────────┘          │
│                                         │
│  ┌──────────────────────────┐          │
│  │     Battlefield          │          │
│  └──────────────────────────┘          │
│                                         │
│  Player 1 Info                          │
│  ┌──────────┐  ┌──────────┐           │
│  │ Hand     │  │ Graveyard │           │
│  └──────────┘  └──────────┘           │
└─────────────────────────────────────────┘
```

### Zone

Displays a single zone containing cards. Supports different layout modes:
- `hand`: Hidden or face-down cards for opponent
- `battlefield`: Face-up cards with tap status
- `graveyard`: Face-up cards
- `exile`: Face-up cards
- `stack`: Face-up cards with target indicators

### Card

Individual card display. Shows:
- Card image from Scryfall
- Card name (as tooltip or alt text)
- Tap status (rotated 90 degrees when tapped)
- Counters (if any)

### GameLog

Scrollable list of all actions with timestamps and descriptions. Supports:
- Search/filter by text
- Highlight current action
- Click to jump to action

### ReplayControls

Playback controls:
- Step backward / forward buttons
- Go to start / end buttons
- Play/Pause button
- Slider for position
- Speed controls (0.5x, 1x, 2x)
- Load new replay button

## Game State Reconstruction

The `GameState` engine reconstructs the board state at any point in the action sequence:

```typescript
class GameState {
  // Advance to specific step
  advanceTo(step: number): void;

  // Get current board state
  getBoardState(): BoardState;

  // Get player state
  getPlayerState(playerId: string): PlayerState;
}
```

## Replay File Format

The viewer expects replay files in the following JSON format:

```typescript
interface ReplayFile {
  version: string;
  header: {
    game_id: string;
    format: string;
    start_time: string;  // ISO 8601 timestamp
    end_time: string;
    players: Player[];
    result: GameResult;
  };
  actions: Action[];
}

interface Action {
  timestamp: string;
  turn: number;
  phase: string;
  active_player: string;
  action_type: ActionType;
}

type ActionType =
  | { type: 'DrawCard'; card_id: string }
  | { type: 'PlayLand'; card_id: string }
  | { type: 'CastSpell'; card_id: string; targets?: string[] }
  | { type: 'ActivateAbility'; card_id: string; ability_id: string }
  | { type: 'Attack'; attacker_id: string; defender_id: string }
  | { type: 'Block'; attacker_id: string; blocker_id: string }
  | { type: 'Resolve'; card_id: string }
  | { type: 'LifeChange'; old_life: number; new_life: number }
  | { type: 'ZoneTransition'; from_zone: string; to_zone: string }
  | { type: 'PassPriority' }
  | { type: 'PhaseChange'; phase: string }
  | { type: 'TurnChange'; turn: number; player_id: string }
  | { type: 'Unknown'; description: string };
```

## Scryfall API Integration

Card images are fetched from the Scryfall API:

```typescript
// Fetch card data by ID
async function getCardById(id: string): Promise<Card>;

// Batch fetch multiple cards
async function getCardBatch(ids: string[]): Promise<Card[]>;

// Card interface
interface Card {
  id: string;
  name: string;
  cmc: number;
  type_line: string;
  colors: string[];
  color_identity: string[];
  image_uris: {
    small: string;
    normal: string;
    large: string;
    png: string;
  };
  legalities: Record<string, string>;
  set_name: string;
  collector_number: string;
}
```

Images are cached in memory to avoid redundant API calls.

## Keyboard Shortcuts

- `←` / `→`: Step backward/forward
- `Home` / `End`: Go to start/end
- `Space`: Toggle play/pause
- `Ctrl+F` / `Cmd+F`: Focus search box

## Testing

### Unit Tests

Component tests use React Testing Library:

```bash
npm test
```

### End-to-End Tests

E2E tests verify complete user flows:

```bash
npm test App.e2e.test.tsx
```

### Type Checking

```bash
npm run type-check
```

## Performance Optimization

- Use `React.memo` for expensive components
- Lazy load card images
- Debounce search input
- Virtualize long lists if needed

## Browser Compatibility

Modern browsers with ES2020 support:
- Chrome 90+
- Firefox 88+
- Safari 14+
- Edge 90+

## Future Enhancements

- Support for multiple games in one file (Bo3 matches)
- Card highlighting (e.g., when hovering log entry)
- Animated card movements
- Export replay as video
- Share replay URLs
- Dark mode theme
- Mobile-friendly layout
