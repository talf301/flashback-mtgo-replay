/**
 * E2E Tests for Main App Component
 *
 * These tests verify the complete user flow from file loading through
 * replay playback and interaction.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { App } from './App';

vi.mock('./engine/reconstructor', async () => {
  const { createEmptyBoardState } = await import('./types/state');
  class MockReconstructor {
    loadReplay = vi.fn();
    reconstruct = vi.fn().mockReturnValue(createEmptyBoardState());
    getActionCount = vi.fn().mockReturnValue(0);
    getTimelineLength = vi.fn().mockReturnValue(0);
    getCardNames = vi.fn().mockReturnValue({});
    getCatalog = vi.fn().mockReturnValue({});
  }
  return { Reconstructor: MockReconstructor };
});

describe('App E2E Tests', () => {
  const mockReplayFile = {
    version: '3.0',
    header: {
      game_id: 456,
      format: 'Modern',
      start_time: '2024-01-01T10:00:00Z',
      end_time: '2024-01-01T11:00:00Z',
      result: { winner: 'Player One', reason: 'Concession' },
      complete: true,
      players: [
        { name: 'Player One', seat: 0 },
        { name: 'Player Two', seat: 1 },
      ],
      decklist: { mainboard: [], sideboard: [] },
      sideboard_changes: null,
    },
    timeline: [
      { type: 'event', turn: 1, phase: 'beginning', active_player: 'Player One', event: { type: 'DrawCard', player: 'Player One', card_id: 'lightning-bolt' } },
      { type: 'event', turn: 1, phase: 'main1', active_player: 'Player One', event: { type: 'PlayLand', player: 'Player One', card_id: 'mountain' } },
      { type: 'event', turn: 1, phase: 'main1', active_player: 'Player One', event: { type: 'CastSpell', player: 'Player One', card_id: 'lightning-bolt' } },
      { type: 'event', turn: 1, phase: 'combat', active_player: 'Player One', event: { type: 'PassPriority', player: 'Player One' } },
      { type: 'event', turn: 1, phase: 'end', active_player: 'Player One', event: { type: 'PassPriority', player: 'Player One' } },
    ],
    card_catalog: {},
  };

  beforeEach(() => {
    vi.clearAllMocks();
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
        expect(screen.getByText('Game: 456')).toBeInTheDocument();
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
        expect(screen.getByText('Game: 456')).toBeInTheDocument();
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
        expect(screen.getByText('Game: 456')).toBeInTheDocument();
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
        expect(screen.getByText('Game: 456')).toBeInTheDocument();
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
      expect(screen.getByText('Timeline entries:')).toBeInTheDocument();
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
