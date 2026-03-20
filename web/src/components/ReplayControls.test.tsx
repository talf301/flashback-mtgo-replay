/**
 * Tests for ReplayControls Component
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ReplayControls } from './ReplayControls';

describe('ReplayControls Component', () => {
  const defaultProps = {
    isPlaying: false,
    canStepForward: true,
    canStepBackward: true,
    canGoToStart: true,
    canGoToEnd: true,
    currentStep: 5,
    totalSteps: 100,
    playbackSpeed: 1,
    onPlayPause: vi.fn(),
    onStepForward: vi.fn(),
    onStepBackward: vi.fn(),
    onGoToStart: vi.fn(),
    onGoToEnd: vi.fn(),
    onSpeedChange: vi.fn(),
    onJumpToStep: vi.fn(),
  };

  it('should render all control buttons', () => {
    render(<ReplayControls {...defaultProps} />);

    expect(screen.getByTitle('Go to start')).toBeInTheDocument();
    expect(screen.getByTitle('Step backward')).toBeInTheDocument();
    expect(screen.getByTitle('Play')).toBeInTheDocument();
    expect(screen.getByTitle('Step forward')).toBeInTheDocument();
    expect(screen.getByTitle('Go to end')).toBeInTheDocument();
  });

  it('should call onPlayPause when play button clicked', () => {
    render(<ReplayControls {...defaultProps} />);

    const playButton = screen.getByTitle('Play');
    fireEvent.click(playButton);

    expect(defaultProps.onPlayPause).toHaveBeenCalledTimes(1);
  });

  it('should show pause icon when playing', () => {
    render(<ReplayControls {...defaultProps} isPlaying={true} />);

    expect(screen.getByTitle('Pause')).toBeInTheDocument();
  });

  it('should disable step backward at start', () => {
    render(
      <ReplayControls {...defaultProps} canStepBackward={false} currentStep={0} />,
    );

    const stepBackButton = screen.getByTitle('Step backward');
    expect(stepBackButton).toBeDisabled();
  });

  it('should disable step forward at end', () => {
    render(
      <ReplayControls
        {...defaultProps}
        canStepForward={false}
        currentStep={100}
        totalSteps={100}
      />,
    );

    const stepForwardButton = screen.getByTitle('Step forward');
    expect(stepForwardButton).toBeDisabled();
  });

  it('should call onStepForward when clicked', () => {
    render(<ReplayControls {...defaultProps} />);

    const stepForwardButton = screen.getByTitle('Step forward');
    fireEvent.click(stepForwardButton);

    expect(defaultProps.onStepForward).toHaveBeenCalledTimes(1);
  });

  it('should call onStepBackward when clicked', () => {
    render(<ReplayControls {...defaultProps} />);

    const stepBackButton = screen.getByTitle('Step backward');
    fireEvent.click(stepBackButton);

    expect(defaultProps.onStepBackward).toHaveBeenCalledTimes(1);
  });

  it('should call onGoToStart when clicked', () => {
    render(<ReplayControls {...defaultProps} />);

    const goStartButton = screen.getByTitle('Go to start');
    fireEvent.click(goStartButton);

    expect(defaultProps.onGoToStart).toHaveBeenCalledTimes(1);
  });

  it('should call onGoToEnd when clicked', () => {
    render(<ReplayControls {...defaultProps} />);

    const goEndButton = screen.getByTitle('Go to end');
    fireEvent.click(goEndButton);

    expect(defaultProps.onGoToEnd).toHaveBeenCalledTimes(1);
  });

  it('should render progress slider', () => {
    render(<ReplayControls {...defaultProps} />);

    expect(screen.getByText('Progress')).toBeInTheDocument();
    expect(screen.getByText('Step 5 / 100')).toBeInTheDocument();

    const slider = screen.getByRole('slider');
    expect(slider).toBeInTheDocument();
  });

  it('should call onJumpToStep when slider changes', () => {
    render(<ReplayControls {...defaultProps} />);

    const slider = screen.getByRole('slider');
    fireEvent.change(slider, { target: { value: '50' } });

    expect(defaultProps.onJumpToStep).toHaveBeenCalledWith(50);
  });

  it('should render speed controls', () => {
    render(<ReplayControls {...defaultProps} />);

    expect(screen.getByText('Playback Speed')).toBeInTheDocument();
    expect(screen.getAllByText('1x').length).toBeGreaterThan(0);
    expect(screen.getByText('0.5x')).toBeInTheDocument();
    expect(screen.getByText('2x')).toBeInTheDocument();
  });

  it('should highlight selected speed', () => {
    render(<ReplayControls {...defaultProps} playbackSpeed={2} />);

    const speed2xButtons = screen.getAllByText('2x');
    const speed2xButton = speed2xButtons.find(el => el.closest('button'))?.closest('button');
    expect(speed2xButton).toHaveClass('border-blue-500');

    const speed1xButtons = screen.getAllByText('1x');
    const speed1xButton = speed1xButtons.find(el => el.closest('button'))?.closest('button');
    expect(speed1xButton).not.toHaveClass('border-blue-500');
  });

  it('should call onSpeedChange when speed button clicked', () => {
    render(<ReplayControls {...defaultProps} />);

    const speed2xButton = screen.getByText('2x').closest('button');
    if (speed2xButton) {
      fireEvent.click(speed2xButton);
    }

    expect(defaultProps.onSpeedChange).toHaveBeenCalledWith(2);
  });

  it('should display keyboard shortcuts hint', () => {
    render(<ReplayControls {...defaultProps} />);

    expect(screen.getByText('Space')).toBeInTheDocument();
    expect(screen.getByText('Step back/forward')).toBeInTheDocument();
    expect(screen.getByText('Go to start/end')).toBeInTheDocument();
  });

  it('should not render slider when onJumpToStep not provided', () => {
    const propsWithoutJump = {
      ...defaultProps,
      onJumpToStep: undefined,
    };

    render(<ReplayControls {...propsWithoutJump} />);

    expect(screen.queryByRole('slider')).not.toBeInTheDocument();
  });

  it('should handle zero total steps', () => {
    render(
      <ReplayControls {...defaultProps} totalSteps={0} currentStep={0} />,
    );

    expect(screen.queryByRole('slider')).not.toBeInTheDocument();
  });

  it('should apply custom className', () => {
    const { container } = render(
      <ReplayControls {...defaultProps} className="custom-class" />,
    );

    expect(container.firstChild).toHaveClass('custom-class');
  });
});
