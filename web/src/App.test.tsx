/**
 * Tests for Main App Component
 *
 * App.tsx still uses the old games[] structure. These tests mock parseActionType
 * (used by GameLog) so the app can render, and use old-format mock data that
 * matches App.tsx's current expectations. When App.tsx is updated for v3, the
 * mock data should be updated to use timeline/card_catalog format.
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

describe('App Component', () => {
  // Mock replay file using the format that App.tsx currently reads (games[])
  const mockReplayFile = {
    metadata: {},
    header: {
      format: 'Standard',
      start_time: '2024-01-01T10:00:00Z',
      end_time: '2024-01-01T10:30:00Z',
      players: [
        { player_id: 'player-1', name: 'Alice', life_total: 20 },
        { player_id: 'player-2', name: 'Bob', life_total: 20 },
      ],
    },
    games: [
      {
        game_number: 1,
        header: {
          game_id: 'test-game-123',
          players: [
            { player_id: 'player-1', name: 'Alice', life_total: 20 },
            { player_id: 'player-2', name: 'Bob', life_total: 20 },
          ],
          result: { Win: { winner_id: 'player-1' } },
        },
        actions: [
          {
            timestamp: '2024-01-01T10:00:00Z',
            turn: 1,
            phase: 'beginning',
            active_player: 'player-1',
            action_type: { DrawCard: { player_id: 'player-1', card_id: 'card-1' } },
          },
          {
            timestamp: '2024-01-01T10:00:05Z',
            turn: 1,
            phase: 'main1',
            active_player: 'player-1',
            action_type: { PlayLand: { player_id: 'player-1', card_id: 'card-2' } },
          },
        ],
      },
    ],
  };

  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    (getCardBatch as any).mockResolvedValue([]);
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it('should render file loader when no replay file', () => {
    render(<App />);

    expect(screen.getByText('Load Replay File')).toBeInTheDocument();
  });

  it('should render app after file load', async () => {
    const { container } = render(<App />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File([JSON.stringify(mockReplayFile)], 'test.json', {
      type: 'application/json',
    });

    if (input) {
      Object.defineProperty(input, 'files', { value: [file] });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getByText('MTG Replay Viewer')).toBeInTheDocument();
    });
  });

  it('should display game info', async () => {
    const { container } = render(<App />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File([JSON.stringify(mockReplayFile)], 'test.json', {
      type: 'application/json',
    });

    if (input) {
      Object.defineProperty(input, 'files', { value: [file] });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getByText('Game: test-game-123')).toBeInTheDocument();
      expect(screen.getByText('Standard')).toBeInTheDocument();
      expect(screen.getAllByText('Alice').length).toBeGreaterThan(0);
      expect(screen.getAllByText('Bob').length).toBeGreaterThan(0);
    });
  });

  it('should load and display replay', async () => {
    const { container } = render(<App />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File([JSON.stringify(mockReplayFile)], 'test.json', {
      type: 'application/json',
    });

    if (input) {
      Object.defineProperty(input, 'files', { value: [file] });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getByText('Turn:')).toBeInTheDocument();
      expect(screen.getByText('Phase:')).toBeInTheDocument();
    });
  });

  it('should handle playback controls', async () => {
    const { container } = render(<App />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File([JSON.stringify(mockReplayFile)], 'test.json', {
      type: 'application/json',
    });

    if (input) {
      Object.defineProperty(input, 'files', { value: [file] });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getByText('Step 0 / 2')).toBeInTheDocument();
    });

    // Step forward
    const stepForwardButton = screen.getByTitle('Step forward');
    await userEvent.click(stepForwardButton);

    await waitFor(() => {
      expect(screen.getByText('Step 1 / 2')).toBeInTheDocument();
    });

    // Step backward
    const stepBackwardButton = screen.getByTitle('Step backward');
    await userEvent.click(stepBackwardButton);

    await waitFor(() => {
      expect(screen.getByText('Step 0 / 2')).toBeInTheDocument();
    });
  });

  it('should handle play/pause', async () => {
    const { container } = render(<App />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File([JSON.stringify(mockReplayFile)], 'test.json', {
      type: 'application/json',
    });

    if (input) {
      Object.defineProperty(input, 'files', { value: [file] });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getByTitle('Play')).toBeInTheDocument();
    });

    // Play
    const playButton = screen.getByTitle('Play');
    await userEvent.click(playButton);

    // Check if playing state is active (pause button appears)
    await waitFor(() => {
      expect(screen.getByTitle('Pause')).toBeInTheDocument();
    });

    // Pause
    const pauseButton = screen.getByTitle('Pause');
    await userEvent.click(pauseButton);

    await waitFor(() => {
      expect(screen.getByTitle('Play')).toBeInTheDocument();
    });
  });

  it('should handle speed change', async () => {
    const { container } = render(<App />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File([JSON.stringify(mockReplayFile)], 'test.json', {
      type: 'application/json',
    });

    if (input) {
      Object.defineProperty(input, 'files', { value: [file] });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getAllByText('1x').length).toBeGreaterThan(0);
    });

    // Change speed to 2x
    const speed2xElements = screen.getAllByText('2x');
    const speed2xButton = speed2xElements.find(el => el.closest('button'))?.closest('button');
    if (speed2xButton) {
      await userEvent.click(speed2xButton);
    }

    await waitFor(() => {
      expect(screen.getAllByText('2x').length).toBeGreaterThan(0);
    });
  });

  it('should jump to step via slider', async () => {
    const { container } = render(<App />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File([JSON.stringify(mockReplayFile)], 'test.json', {
      type: 'application/json',
    });

    if (input) {
      Object.defineProperty(input, 'files', { value: [file] });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getByText('Step 0 / 2')).toBeInTheDocument();
    });

    const slider = screen.getByRole('slider');
    fireEvent.change(slider, { target: { value: '1' } });

    await waitFor(() => {
      expect(screen.getByText('Step 1 / 2')).toBeInTheDocument();
    });
  });

  it('should display game log', async () => {
    const { container } = render(<App />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File([JSON.stringify(mockReplayFile)], 'test.json', {
      type: 'application/json',
    });

    if (input) {
      Object.defineProperty(input, 'files', { value: [file] });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getByText('Game Log')).toBeInTheDocument();
    });
  });

  it('should load new replay button', async () => {
    const { container } = render(<App />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File([JSON.stringify(mockReplayFile)], 'test.json', {
      type: 'application/json',
    });

    if (input) {
      Object.defineProperty(input, 'files', { value: [file] });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getByText('Load New Replay')).toBeInTheDocument();
    });

    // Click load new replay
    const loadNewButton = screen.getByText('Load New Replay');
    await userEvent.click(loadNewButton);

    await waitFor(() => {
      expect(screen.getByText('Load Replay File')).toBeInTheDocument();
    });
  });

  it('should display winner info', async () => {
    const { container } = render(<App />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File([JSON.stringify(mockReplayFile)], 'test.json', {
      type: 'application/json',
    });

    if (input) {
      Object.defineProperty(input, 'files', { value: [file] });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getAllByText('Alice').length).toBeGreaterThan(0);
    });
  });

  it('should handle go to start/end', async () => {
    const { container } = render(<App />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File([JSON.stringify(mockReplayFile)], 'test.json', {
      type: 'application/json',
    });

    if (input) {
      Object.defineProperty(input, 'files', { value: [file] });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getByText('Step 0 / 2')).toBeInTheDocument();
    });

    // Go to end
    const goToEndButton = screen.getByTitle('Go to end');
    await userEvent.click(goToEndButton);

    await waitFor(() => {
      expect(screen.getByText('Step 2 / 2')).toBeInTheDocument();
    });

    // Go to start
    const goToStartButton = screen.getByTitle('Go to start');
    await userEvent.click(goToStartButton);

    await waitFor(() => {
      expect(screen.getByText('Step 0 / 2')).toBeInTheDocument();
    });
  });

  it('should handle auto-play progression', async () => {
    const { container } = render(<App />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    const file = new File([JSON.stringify(mockReplayFile)], 'test.json', {
      type: 'application/json',
    });

    if (input) {
      Object.defineProperty(input, 'files', { value: [file] });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getByText('Step 0 / 2')).toBeInTheDocument();
    });

    // Start playback
    const playButton = screen.getByTitle('Play');
    await userEvent.click(playButton);

    // Advance timer
    vi.advanceTimersByTime(1100);

    await waitFor(() => {
      expect(screen.getByText('Step 1 / 2')).toBeInTheDocument();
    });

    // Advance timer again
    vi.advanceTimersByTime(1100);

    await waitFor(() => {
      expect(screen.getByText('Step 2 / 2')).toBeInTheDocument();
    });

    // Playback should stop at end
    await waitFor(() => {
      expect(screen.getByTitle('Play')).toBeInTheDocument();
    });
  });
});
