/**
 * Tests for FileLoader Component
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { FileLoader } from './FileLoader';
import type { ReplayFile } from '../types/replay';

describe('FileLoader Component', () => {
  const mockReplayFile: ReplayFile = {
    version: '3.0',
    header: {
      game_id: 123,
      format: 'Standard',
      start_time: '2024-01-01T10:00:00Z',
      result: { winner: '', reason: '' },
      complete: false,
      players: [
        { name: 'Alice', seat: 0 },
        { name: 'Bob', seat: 1 },
      ],
      decklist: { mainboard: [], sideboard: [] },
      sideboard_changes: null,
    },
    timeline: [
      {
        type: 'event',
        turn: 1,
        phase: 'beginning',
        active_player: 'Alice',
        event: { type: 'DrawCard', player: 'Alice', card_id: 'card-1' },
      },
    ],
    card_catalog: {},
  };

  const mockJsonContent = JSON.stringify(mockReplayFile);

  const mockFile = new File([mockJsonContent], 'test-replay.json', {
    type: 'application/json',
  });

  beforeEach(() => {
    vi.clearAllMocks();
    // Mock URL.createObjectURL
    global.URL.createObjectURL = vi.fn(() => 'mock-url');
    global.URL.revokeObjectURL = vi.fn();
  });

  it('should render drop zone', () => {
    const handleLoad = vi.fn();
    render(<FileLoader onFileLoad={handleLoad} />);

    expect(screen.getByText('Load Replay File')).toBeInTheDocument();
    expect(screen.getByText(/Drag and drop replay file here/)).toBeInTheDocument();
  });

  it('should handle file selection via button click', async () => {
    const handleLoad = vi.fn();

    const { container } = render(<FileLoader onFileLoad={handleLoad} />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    expect(input).toBeInTheDocument();

    if (input) {
      const fileList = [mockFile] as unknown as FileList;
      Object.defineProperty(input, 'files', { value: fileList, writable: false });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(handleLoad).toHaveBeenCalledWith(mockReplayFile);
    });
  });

  it('should handle file drop', async () => {
    const handleLoad = vi.fn();

    render(<FileLoader onFileLoad={handleLoad} />);

    const dropZone = screen.getByText(/Drag and drop/).closest('div');
    if (dropZone) {
      fireEvent.drop(dropZone, {
        dataTransfer: { files: [mockFile] },
      });
    }

    await waitFor(() => {
      expect(handleLoad).toHaveBeenCalledWith(mockReplayFile);
    });
  });

  it('should show loading state', async () => {
    const handleLoad = vi.fn(() => new Promise(() => {})); // Never resolves

    const { container } = render(<FileLoader onFileLoad={handleLoad} />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    if (input) {
      const fileList = [mockFile] as unknown as FileList;
      Object.defineProperty(input, 'files', { value: fileList, writable: false });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getByText('Loading file...')).toBeInTheDocument();
    });
  });

  it('should show success state after loading', async () => {
    const handleLoad = vi.fn();

    const { container } = render(<FileLoader onFileLoad={handleLoad} />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    if (input) {
      const fileList = [mockFile] as unknown as FileList;
      Object.defineProperty(input, 'files', { value: fileList, writable: false });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getByText('test-replay.json')).toBeInTheDocument();
      expect(screen.getByText('Click to load another file')).toBeInTheDocument();
    });
  });

  it('should handle invalid JSON', async () => {
    const handleLoad = vi.fn();
    const invalidFile = new File(['invalid json'], 'invalid.json', {
      type: 'application/json',
    });

    const { container } = render(<FileLoader onFileLoad={handleLoad} />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    if (input) {
      const fileList = [invalidFile] as unknown as FileList;
      Object.defineProperty(input, 'files', { value: fileList, writable: false });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getByText(/Failed to parse JSON file/)).toBeInTheDocument();
    });

    expect(handleLoad).not.toHaveBeenCalled();
  });

  it('should handle invalid file structure', async () => {
    const handleLoad = vi.fn();
    const invalidStructure = JSON.stringify({ invalid: 'structure' });
    const invalidFile = new File([invalidStructure], 'invalid-structure.json', {
      type: 'application/json',
    });

    const { container } = render(<FileLoader onFileLoad={handleLoad} />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    if (input) {
      const fileList = [invalidFile] as unknown as FileList;
      Object.defineProperty(input, 'files', { value: fileList, writable: false });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getByText(/Missing or invalid header/)).toBeInTheDocument();
    });

    expect(handleLoad).not.toHaveBeenCalled();
  });

  it('should call onError callback on error', async () => {
    const handleLoad = vi.fn();
    const handleError = vi.fn();
    const invalidFile = new File(['invalid'], 'invalid.json', {
      type: 'application/json',
    });

    const { container } = render(
      <FileLoader onFileLoad={handleLoad} onError={handleError} />,
    );

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    if (input) {
      const fileList = [invalidFile] as unknown as FileList;
      Object.defineProperty(input, 'files', { value: fileList, writable: false });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(handleError).toHaveBeenCalled();
    });
  });

  it('should clear file on clear button click', async () => {
    const handleLoad = vi.fn();

    const { container } = render(<FileLoader onFileLoad={handleLoad} />);

    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    if (input) {
      const fileList = [mockFile] as unknown as FileList;
      Object.defineProperty(input, 'files', { value: fileList, writable: false });
      fireEvent.change(input);
    }

    await waitFor(() => {
      expect(screen.getByText('test-replay.json')).toBeInTheDocument();
    });

    const clearButton = screen.getByText('Clear File');
    fireEvent.click(clearButton);

    await waitFor(() => {
      expect(screen.queryByText('test-replay.json')).not.toBeInTheDocument();
    });
  });

  it('should handle drag over state', () => {
    const handleLoad = vi.fn();

    render(<FileLoader onFileLoad={handleLoad} />);

    const dropZone = screen.getByText(/Drag and drop/).closest('.border-dashed');
    if (dropZone) {
      fireEvent.dragOver(dropZone);
      expect(dropZone).toHaveClass('border-blue-500');

      fireEvent.dragLeave(dropZone);
      expect(dropZone).not.toHaveClass('border-blue-500');
    }
  });

  it('should apply custom className', () => {
    const handleLoad = vi.fn();
    const { container } = render(
      <FileLoader onFileLoad={handleLoad} className="custom-class" />,
    );

    expect(container.firstChild).toHaveClass('custom-class');
  });

  it('should show file size limit info', () => {
    const handleLoad = vi.fn();
    render(<FileLoader onFileLoad={handleLoad} maxSizeBytes={5 * 1024 * 1024} />);

    expect(screen.getByText(/Maximum size:/)).toBeInTheDocument();
    expect(screen.getByText(/5\.00 MB/)).toBeInTheDocument();
  });

  describe('Load Demo Replay', () => {
    const demoReplayContent: ReplayFile = {
      version: '3',
      header: {
        game_id: 100042,
        format: 'Modern',
        start_time: '2026-03-15T19:05:00Z',
        end_time: '2026-03-15T19:28:00Z',
        result: { winner: 'Alice', reason: 'life' },
        complete: true,
        players: [
          { name: 'Alice', seat: 1 },
          { name: 'Bob', seat: 2 },
        ],
        decklist: { mainboard: ['Ragavan, Nimble Pilferer'], sideboard: [] },
        sideboard_changes: null,
      },
      timeline: [
        {
          type: 'event',
          turn: 1,
          phase: 'precombat_main',
          active_player: 'Alice',
          event: { type: 'DrawCard', player: 'Alice', card_id: 'a-1' },
        },
      ],
      card_catalog: {
        ragavan: { name: 'Ragavan, Nimble Pilferer', mana_cost: '{R}', type_line: 'Legendary Creature - Monkey Pirate' },
      },
    };

    it('should show Load Demo Replay button in empty state', () => {
      const handleLoad = vi.fn();
      render(<FileLoader onFileLoad={handleLoad} />);

      expect(screen.getByText('Load Demo Replay')).toBeInTheDocument();
    });

    it('should not show Load Demo Replay button when file is loaded', async () => {
      const handleLoad = vi.fn();
      const { container } = render(<FileLoader onFileLoad={handleLoad} />);

      const input = container.querySelector('input[type="file"]') as HTMLInputElement;
      if (input) {
        const fileList = [mockFile] as unknown as FileList;
        Object.defineProperty(input, 'files', { value: fileList, writable: false });
        fireEvent.change(input);
      }

      await waitFor(() => {
        expect(screen.queryByText('Load Demo Replay')).not.toBeInTheDocument();
      });
    });

    it('should fetch and load demo replay on button click', async () => {
      const handleLoad = vi.fn();
      global.fetch = vi.fn().mockResolvedValue({
        ok: true,
        text: () => Promise.resolve(JSON.stringify(demoReplayContent)),
      });

      render(<FileLoader onFileLoad={handleLoad} />);

      fireEvent.click(screen.getByText('Load Demo Replay'));

      await waitFor(() => {
        expect(global.fetch).toHaveBeenCalledWith(`${import.meta.env.BASE_URL}demo.flashback`);
        expect(handleLoad).toHaveBeenCalledWith(demoReplayContent);
        expect(screen.getByText('demo.flashback')).toBeInTheDocument();
      });
    });

    it('should show error when demo fetch fails', async () => {
      const handleLoad = vi.fn();
      const handleError = vi.fn();
      global.fetch = vi.fn().mockResolvedValue({
        ok: false,
        status: 404,
        text: () => Promise.resolve('Not found'),
      });

      render(<FileLoader onFileLoad={handleLoad} onError={handleError} />);

      fireEvent.click(screen.getByText('Load Demo Replay'));

      await waitFor(() => {
        expect(screen.getByText(/Failed to fetch demo replay: 404/)).toBeInTheDocument();
        expect(handleError).toHaveBeenCalledWith('Failed to fetch demo replay: 404');
      });

      expect(handleLoad).not.toHaveBeenCalled();
    });

    it('should show error when demo response is invalid JSON', async () => {
      const handleLoad = vi.fn();
      global.fetch = vi.fn().mockResolvedValue({
        ok: true,
        text: () => Promise.resolve('not valid json'),
      });

      render(<FileLoader onFileLoad={handleLoad} />);

      fireEvent.click(screen.getByText('Load Demo Replay'));

      await waitFor(() => {
        expect(screen.getByText(/Failed to parse demo replay JSON/)).toBeInTheDocument();
      });

      expect(handleLoad).not.toHaveBeenCalled();
    });
  });
});
