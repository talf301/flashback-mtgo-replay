/**
 * Tests for Main App Component
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { App } from './App';
import type { ReplayFile } from './types/replay';
import { getCardBatch } from './api/scryfall';

vi.mock('./api/scryfall');
vi.mock('./engine/reconstructor');

describe('App Component', () => {
  const mockReplayFile: ReplayFile = {
    version: '1.0',
    header: {
      game_id: 'test-game-123',
      format: 'Standard',
      start_time: '2024-01-01T10:00:00Z',
      end_time: '2024-01-01T10:30:00Z',
      players: [
        { id: 'player-1', name: 'Alice' },
        { id: 'player-2', name: 'Bob' },
      ],
      result: {
        winner: 'player-1',
        reason: 'opponent conceded',
      },
    },
    actions: [
      {
        timestamp: '2024-01-01T10:00:00Z',
        turn: 1,
        phase: 'beginning',
        active_player: 'player-1',
        action_type: { type: 'DrawCard', card_id: 'card-1' },
      },
      {
        timestamp: '2024-01-01T10:00:05Z',
        turn: 1,
        phase: 'main1',
        active_player: 'player-1',
        action_type: { type: 'PlayLand', card_id: 'card-2' },
      },
    ],
  };

  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
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

    // Simulate file load by creating a mock file
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
      expect(screen.getByText('Alice')).toBeInTheDocument();
      expect(screen.getByText('Bob')).toBeInTheDocument();
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
      expect(screen.getByText('1x')).toBeInTheDocument();
    });

    // Change speed to 2x
    const speed2xButton = screen.getByText('2x').closest('button');
    if (speed2xButton) {
      await userEvent.click(speed2xButton);
    }

    await waitFor(() => {
      const speedElement = screen.getByText('2x');
      expect(speedElement).toBeInTheDocument();
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
      expect(screen.getByText('Alice')).toBeInTheDocument();
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
