/**
 * Tests for FileLoader Component
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { FileLoader } from './FileLoader';
import type { ReplayFile } from '../types/replay';

describe('FileLoader Component', () => {
  const mockReplayFile: ReplayFile = {
    version: '1.0',
    header: {
      game_id: 'test-game-123',
      format: 'Standard',
      start_time: '2024-01-01T10:00:00Z',
      players: [
        { id: 'player-1', name: 'Alice' },
        { id: 'player-2', name: 'Bob' },
      ],
    },
    actions: [
      {
        timestamp: '2024-01-01T10:00:00Z',
        turn: 1,
        phase: 'beginning',
        active_player: 'player-1',
        action_type: { type: 'DrawCard', card_id: 'card-1' },
      },
    ],
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
      expect(screen.getByText(/Invalid replay file structure/)).toBeInTheDocument();
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

    const dropZone = screen.getByText(/Drag and drop/).closest('div');
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
});
