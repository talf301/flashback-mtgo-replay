/**
 * E2E Tests for Main App Component
 *
 * These tests verify the complete user flow from file loading through
 * replay playback and interaction.
 *
 * App.tsx still uses the old games[] structure. We mock parseActionType
 * (used by GameLog) so the app can render.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// Mock parseActionType so GameLog renders inside App
vi.mock('./types/replay', async () => {
  const actual = await vi.importActual<typeof import('./types/replay')>('./types/replay');
  return {
    ...actual,
    parseActionType: (actionType: Record<string, unknown>) => {
      if (!actionType) return { type: 'Unknown', data: {} };
      const keys = Object.keys(actionType);
      if (keys.length > 0) {
        const type = keys[0];
        return { type, data: (actionType[type] as Record<string, unknown>) || {} };
      }
      return { type: 'Unknown', data: {} };
    },
  };
});

import { App } from './App';
import { getCardBatch } from './api/scryfall';

vi.mock('./api/scryfall');

describe('App E2E Tests', () => {
  const mockReplayFile = {
    metadata: {},
    header: {
      format: 'Modern',
      start_time: '2024-01-01T10:00:00Z',
      end_time: '2024-01-01T11:00:00Z',
      players: [
        { player_id: 'player-1', name: 'Player One', life_total: 20 },
        { player_id: 'player-2', name: 'Player Two', life_total: 20 },
      ],
    },
    // games[] is read by App.tsx (old format, still present until App is updated to v3)
    games: [
      {
        game_number: 1,
        header: {
          game_id: 'e2e-test-game',
          players: [
            { player_id: 'player-1', name: 'Player One', life_total: 20 },
            { player_id: 'player-2', name: 'Player Two', life_total: 20 },
          ],
          result: { Win: { winner_id: 'player-1' } },
        },
        actions: [
          {
            timestamp: '2024-01-01T10:00:00Z',
            turn: 1,
            phase: 'beginning',
            active_player: 'player-1',
            action_type: { DrawCard: { player_id: 'player-1', card_id: 'lightning-bolt' } },
          },
          {
            timestamp: '2024-01-01T10:00:05Z',
            turn: 1,
            phase: 'main1',
            active_player: 'player-1',
            action_type: { PlayLand: { player_id: 'player-1', card_id: 'mountain' } },
          },
          {
            timestamp: '2024-01-01T10:00:10Z',
            turn: 1,
            phase: 'main1',
            active_player: 'player-1',
            action_type: { CastSpell: { player_id: 'player-1', card_id: 'lightning-bolt' } },
          },
          {
            timestamp: '2024-01-01T10:00:15Z',
            turn: 1,
            phase: 'combat',
            active_player: 'player-1',
            action_type: { PassPriority: { player_id: 'player-1' } },
          },
          {
            timestamp: '2024-01-01T10:00:20Z',
            turn: 1,
            phase: 'end',
            active_player: 'player-1',
            action_type: { PassPriority: { player_id: 'player-1' } },
          },
        ],
      },
    ],
    // v3 timeline/catalog for the real Reconstructor
    timeline: [
      { type: 'event', turn: 1, phase: 'beginning', active_player: 'Player One', event: { type: 'DrawCard', player: 'Player One', card_id: 'lightning-bolt' } },
      { type: 'event', turn: 1, phase: 'main1', active_player: 'Player One', event: { type: 'PlayLand', player: 'Player One', card_id: 'mountain' } },
      { type: 'event', turn: 1, phase: 'main1', active_player: 'Player One', event: { type: 'CastSpell', player: 'Player One', card_id: 'lightning-bolt' } },
      { type: 'event', turn: 1, phase: 'combat', active_player: 'Player One', event: { type: 'PassPriority', player: 'Player One' } },
      { type: 'event', turn: 1, phase: 'end', active_player: 'Player One', event: { type: 'PassPriority', player: 'Player One' } },
    ],
    card_catalog: {},
  } as any;

  beforeEach(() => {
    vi.clearAllMocks();
    (getCardBatch as any).mockResolvedValue([
      {
        id: 'lb-123',
        name: 'Lightning Bolt',
        cmc: 1,
        type_line: 'Instant',
        colors: ['R'],
        color_identity: ['R'],
        image_uris: {
          small: 'https://example.com/small.jpg',
          normal: 'https://example.com/normal.jpg',
          large: 'https://example.com/large.jpg',
          png: 'https://example.com/card.png',
          art_crop: 'https://example.com/art.jpg',
          border_crop: 'https://example.com/border.jpg',
        },
        legalities: { modern: 'legal' },
        set_name: 'Modern Horizons 2',
        collector_number: '123',
      },
    ]);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('Complete User Flow', () => {
    it('should complete full replay playback cycle', async () => {
      // Step 1: Load file
      const { container } = render(<App />);
      const input = container.querySelector('input[type="file"]') as HTMLInputElement;
      const file = new File([JSON.stringify(mockReplayFile)], 'replay.json', {
        type: 'application/json',
      });

      if (input) {
        Object.defineProperty(input, 'files', { value: [file] });
        fireEvent.change(input);
      }

      // Verify app loaded
      await waitFor(() => {
        expect(screen.getByText('MTG Replay Viewer')).toBeInTheDocument();
        expect(screen.getByText('Game: e2e-test-game')).toBeInTheDocument();
      });

      // Step 2: Verify initial state
      await waitFor(() => {
        expect(screen.getByText('Step 0 / 5')).toBeInTheDocument();
        expect(screen.getByText('Turn:')).toBeInTheDocument();
        expect(screen.getByText('Phase:')).toBeInTheDocument();
      });

      // Step 3: Step through all actions
      for (let i = 1; i <= 5; i++) {
        const stepForwardButton = screen.getByTitle('Step forward');
        await userEvent.click(stepForwardButton);

        await waitFor(() => {
          expect(screen.getByText(`Step ${i} / 5`)).toBeInTheDocument();
        });
      }

      // Step 4: Step back to beginning
      for (let i = 4; i >= 0; i--) {
        const stepBackwardButton = screen.getByTitle('Step backward');
        await userEvent.click(stepBackwardButton);

        await waitFor(() => {
          expect(screen.getByText(`Step ${i} / 5`)).toBeInTheDocument();
        });
      }

      // Step 5: Use slider to jump
      const slider = screen.getByRole('slider');
      fireEvent.change(slider, { target: { value: '3' } });

      await waitFor(() => {
        expect(screen.getByText('Step 3 / 5')).toBeInTheDocument();
      });

      // Step 6: Use go to start/end
      const goToEndButton = screen.getByTitle('Go to end');
      await userEvent.click(goToEndButton);

      await waitFor(() => {
        expect(screen.getByText('Step 5 / 5')).toBeInTheDocument();
      });

      const goToStartButton = screen.getByTitle('Go to start');
      await userEvent.click(goToStartButton);

      await waitFor(() => {
        expect(screen.getByText('Step 0 / 5')).toBeInTheDocument();
      });
    });

    it('should search and filter game log', async () => {
      // Load file
      const { container } = render(<App />);
      const input = container.querySelector('input[type="file"]') as HTMLInputElement;
      const file = new File([JSON.stringify(mockReplayFile)], 'replay.json', {
        type: 'application/json',
      });

      if (input) {
        Object.defineProperty(input, 'files', { value: [file] });
        fireEvent.change(input);
      }

      await waitFor(() => {
        expect(screen.getByText('Game Log')).toBeInTheDocument();
      });

      // Search for specific action type
      const searchInput = screen.getByPlaceholderText('Search actions...');
      await userEvent.type(searchInput, 'PlayLand');

      await waitFor(() => {
        expect(screen.getByText(/played #mountain/)).toBeInTheDocument();
      });

      // Clear search
      await userEvent.clear(searchInput);

      await waitFor(() => {
        expect(screen.getByText(/drew #lightning-bolt/)).toBeInTheDocument();
      });
    });

    it('should change playback speed', async () => {
      // Load file
      const { container } = render(<App />);
      const input = container.querySelector('input[type="file"]') as HTMLInputElement;
      const file = new File([JSON.stringify(mockReplayFile)], 'replay.json', {
        type: 'application/json',
      });

      if (input) {
        Object.defineProperty(input, 'files', { value: [file] });
        fireEvent.change(input);
      }

      await waitFor(() => {
        expect(screen.getAllByText('1x').length).toBeGreaterThan(0);
      });

      // Change to 2x
      const speed2xButtons = screen.getAllByText('2x');
      const speed2xButton = speed2xButtons.find(el => el.closest('button'))?.closest('button');
      if (speed2xButton) {
        await userEvent.click(speed2xButton);
      }

      await waitFor(() => {
        expect(screen.getAllByText('2x').length).toBeGreaterThan(0);
      });

      // Change to 0.5x
      const speed05xButtons = screen.getAllByText('0.5x');
      const speed05xButton = speed05xButtons.find(el => el.closest('button'))?.closest('button');
      if (speed05xButton) {
        await userEvent.click(speed05xButton);
      }

      await waitFor(() => {
        expect(screen.getAllByText('0.5x').length).toBeGreaterThan(0);
      });
    });

    it('should load new replay after finishing one', async () => {
      // Load first file
      const { container } = render(<App />);
      const input = container.querySelector('input[type="file"]') as HTMLInputElement;
      const file = new File([JSON.stringify(mockReplayFile)], 'replay.json', {
        type: 'application/json',
      });

      if (input) {
        Object.defineProperty(input, 'files', { value: [file] });
        fireEvent.change(input);
      }

      await waitFor(() => {
        expect(screen.getByText('Game: e2e-test-game')).toBeInTheDocument();
      });

      // Click load new replay
      const loadNewButton = screen.getByText('Load New Replay');
      await userEvent.click(loadNewButton);

      // Verify back to file loader
      await waitFor(() => {
        expect(screen.getByText('Load Replay File')).toBeInTheDocument();
      });

      // Load another file
      const input2 = container.querySelector('input[type="file"]') as HTMLInputElement;
      const file2 = new File([JSON.stringify(mockReplayFile)], 'replay2.json', {
        type: 'application/json',
      });

      if (input2) {
        Object.defineProperty(input2, 'files', { value: [file2] });
        fireEvent.change(input2);
      }

      await waitFor(() => {
        expect(screen.getByText('Game: e2e-test-game')).toBeInTheDocument();
      });
    });

    it('should display all game info correctly', async () => {
      // Load file
      const { container } = render(<App />);
      const input = container.querySelector('input[type="file"]') as HTMLInputElement;
      const file = new File([JSON.stringify(mockReplayFile)], 'replay.json', {
        type: 'application/json',
      });

      if (input) {
        Object.defineProperty(input, 'files', { value: [file] });
        fireEvent.change(input);
      }

      // Verify header info
      await waitFor(() => {
        expect(screen.getByText('MTG Replay Viewer')).toBeInTheDocument();
        expect(screen.getByText('Game: e2e-test-game')).toBeInTheDocument();
        expect(screen.getByText('Modern')).toBeInTheDocument();
      });

      // Verify player info
      expect(screen.getAllByText('Player One').length).toBeGreaterThan(0);
      expect(screen.getAllByText('Player Two').length).toBeGreaterThan(0);

      // Verify game info sidebar
      expect(screen.getByText('Players')).toBeInTheDocument();
      expect(screen.getByText('Game Info')).toBeInTheDocument();
      expect(screen.getByText('Started:')).toBeInTheDocument();
      expect(screen.getByText('Ended:')).toBeInTheDocument();
      expect(screen.getByText('Actions:')).toBeInTheDocument();
      expect(screen.getByText(/Result:/)).toBeInTheDocument();
    });

    it('should handle error states gracefully', async () => {
      // Try to load invalid file
      const { container } = render(<App />);
      const input = container.querySelector('input[type="file"]') as HTMLInputElement;
      const invalidFile = new File(['invalid json'], 'invalid.json', {
        type: 'application/json',
      });

      if (input) {
        Object.defineProperty(input, 'files', { value: [invalidFile] });
        fireEvent.change(input);
      }

      // Should show error and stay on file loader
      await waitFor(() => {
        expect(screen.getByText(/Failed to parse JSON file/)).toBeInTheDocument();
      });

      expect(screen.getByText('Load Replay File')).toBeInTheDocument();
    });
  });

  describe('Keyboard Shortcuts', () => {
    it('should display keyboard shortcut hints', async () => {
      // Load file
      const { container } = render(<App />);
      const input = container.querySelector('input[type="file"]') as HTMLInputElement;
      const file = new File([JSON.stringify(mockReplayFile)], 'replay.json', {
        type: 'application/json',
      });

      if (input) {
        Object.defineProperty(input, 'files', { value: [file] });
        fireEvent.change(input);
      }

      await waitFor(() => {
        expect(screen.getByText('Step 0 / 5')).toBeInTheDocument();
      });

      // Verify keyboard shortcut hints are displayed
      expect(screen.getByText('Space')).toBeInTheDocument();
      expect(screen.getByText('Step back/forward')).toBeInTheDocument();
      expect(screen.getByText('Go to start/end')).toBeInTheDocument();
    });
  });
});
