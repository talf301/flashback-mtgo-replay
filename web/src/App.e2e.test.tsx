/**
 * E2E Tests for Main App Component
 *
 * These tests verify the complete user flow from file loading through
 * replay playback and interaction.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { App } from './App';
import type { ReplayFile } from './types/replay';
import { getCardBatch } from './api/scryfall';

vi.mock('./api/scryfall');

describe('App E2E Tests', () => {
  const mockReplayFile: ReplayFile = {
    version: '1.0',
    header: {
      game_id: 'e2e-test-game',
      format: 'Modern',
      start_time: '2024-01-01T10:00:00Z',
      end_time: '2024-01-01T11:00:00Z',
      players: [
        { id: 'player-1', name: 'Player One' },
        { id: 'player-2', name: 'Player Two' },
      ],
      result: {
        winner: 'player-1',
        reason: 'concede',
      },
    },
    actions: [
      {
        timestamp: '2024-01-01T10:00:00Z',
        turn: 1,
        phase: 'beginning',
        active_player: 'player-1',
        action_type: { type: 'DrawCard', card_id: 'lightning-bolt' },
      },
      {
        timestamp: '2024-01-01T10:00:05Z',
        turn: 1,
        phase: 'main1',
        active_player: 'player-1',
        action_type: { type: 'PlayLand', card_id: 'mountain' },
      },
      {
        timestamp: '2024-01-01T10:00:10Z',
        turn: 1,
        phase: 'main1',
        active_player: 'player-1',
        action_type: { type: 'CastSpell', card_id: 'lightning-bolt', targets: ['player-2'] },
      },
      {
        timestamp: '2024-01-01T10:00:15Z',
        turn: 1,
        phase: 'combat',
        active_player: 'player-1',
        action_type: { type: 'PassPriority' },
      },
      {
        timestamp: '2024-01-01T10:00:20Z',
        turn: 1,
        phase: 'end',
        active_player: 'player-1',
        action_type: { type: 'PassPriority' },
      },
    ],
  };

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
      render(<App />);

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
      render(<App />);

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

      // Search for specific action
      const searchInput = screen.getByPlaceholderText('Search actions...');
      await userEvent.type(searchInput, 'Land');

      await waitFor(() => {
        expect(screen.getByText(/played a land/)).toBeInTheDocument();
      });

      // Clear search
      await userEvent.clear(searchInput);

      await waitFor(() => {
        expect(screen.getByText(/drew a card/)).toBeInTheDocument();
      });
    });

    it('should change playback speed', async () => {
      render(<App />);

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
        expect(screen.getByText('1x')).toBeInTheDocument();
      });

      // Change to 2x
      const speed2xButton = screen.getByText('2x').closest('button');
      if (speed2xButton) {
        await userEvent.click(speed2xButton);
      }

      await waitFor(() => {
        expect(screen.getByText('2x')).toBeInTheDocument();
      });

      // Change to 0.5x
      const speed05xButton = screen.getByText('0.5x').closest('button');
      if (speed05xButton) {
        await userEvent.click(speed05xButton);
      }

      await waitFor(() => {
        expect(screen.getByText('0.5x')).toBeInTheDocument();
      });
    });

    it('should load new replay after finishing one', async () => {
      render(<App />);

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
      render(<App />);

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
      expect(screen.getByText('Player One')).toBeInTheDocument();
      expect(screen.getByText('Player Two')).toBeInTheDocument();

      // Verify game info sidebar
      expect(screen.getByText('Players')).toBeInTheDocument();
      expect(screen.getByText('Game Info')).toBeInTheDocument();
      expect(screen.getByText('Started:')).toBeInTheDocument();
      expect(screen.getByText('Ended:')).toBeInTheDocument();
      expect(screen.getByText('Actions:')).toBeInTheDocument();
      expect(screen.getByText('Winner:')).toBeInTheDocument();
    });

    it('should handle error states gracefully', async () => {
      render(<App />);

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
    it('should handle keyboard shortcuts', async () => {
      render(<App />);

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

      // Step forward with right arrow
      await userEvent.keyboard('{ArrowRight}');

      await waitFor(() => {
        expect(screen.getByText('Step 1 / 5')).toBeInTheDocument();
      });

      // Step back with left arrow
      await userEvent.keyboard('{ArrowLeft}');

      await waitFor(() => {
        expect(screen.getByText('Step 0 / 5')).toBeInTheDocument();
      });

      // Go to end with End key
      await userEvent.keyboard('{End}');

      await waitFor(() => {
        expect(screen.getByText('Step 5 / 5')).toBeInTheDocument();
      });

      // Go to start with Home key
      await userEvent.keyboard('{Home}');

      await waitFor(() => {
        expect(screen.getByText('Step 0 / 5')).toBeInTheDocument();
      });

      // Toggle play/pause with Space
      await userEvent.keyboard(' ');

      await waitFor(() => {
        expect(screen.getByTitle('Pause')).toBeInTheDocument();
      });

      await userEvent.keyboard(' ');

      await waitFor(() => {
        expect(screen.getByTitle('Play')).toBeInTheDocument();
      });
    });
  });
});
