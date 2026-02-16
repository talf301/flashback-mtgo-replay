/**
 * Replay Controls Component
 *
 * Provides playback controls for navigating through game replay steps.
 * Includes play/pause, step forward/back, jump to start/end, and speed controls.
 */

import { useState, useCallback } from 'react';

export interface ReplayControlsProps {
  isPlaying: boolean;
  canStepForward: boolean;
  canStepBackward: boolean;
  canGoToStart: boolean;
  canGoToEnd: boolean;
  currentStep: number;
  totalSteps: number;
  playbackSpeed: number;
  onPlayPause: () => void;
  onStepForward: () => void;
  onStepBackward: () => void;
  onGoToStart: () => void;
  onGoToEnd: () => void;
  onSpeedChange: (speed: number) => void;
  onJumpToStep?: (step: number) => void;
  className?: string;
}

export function ReplayControls({
  isPlaying,
  canStepForward,
  canStepBackward,
  canGoToStart,
  canGoToEnd,
  currentStep,
  totalSteps,
  playbackSpeed,
  onPlayPause,
  onStepForward,
  onStepBackward,
  onGoToStart,
  onGoToEnd,
  onSpeedChange,
  onJumpToStep,
  className = '',
}: ReplayControlsProps) {
  const [isDragging, setIsDragging] = useState(false);

  const handleSliderChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const newStep = parseInt(e.target.value, 10);
      onJumpToStep?.(newStep);
    },
    [onJumpToStep],
  );

  const speedOptions = [0.25, 0.5, 1, 2, 4];
  const speedLabels: Record<number, string> = {
    0.25: '0.25x',
    0.5: '0.5x',
    1: '1x',
    2: '2x',
    4: '4x',
  };

  const ControlButton = ({
    onClick,
    disabled,
    label,
    icon,
  }: {
    onClick: () => void;
    disabled: boolean;
    label: string;
    icon: string;
  }) => (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`
        p-2 rounded-lg border transition-all
        ${disabled ? 'opacity-50 cursor-not-allowed border-slate-700' : 'border-slate-600 hover:border-slate-400 hover:bg-slate-700'}
      `}
      title={label}
    >
      <span className="text-xl" aria-hidden="true">
        {icon}
      </span>
      <span className="sr-only">{label}</span>
    </button>
  );

  return (
    <div className={`bg-slate-900/80 backdrop-blur border border-slate-700 rounded-lg p-4 ${className}`}>
      {/* Playback controls */}
      <div className="flex items-center justify-center gap-2 mb-4">
        <ControlButton
          onClick={onGoToStart}
          disabled={!canGoToStart}
          label="Go to start"
          icon="⏮"
        />
        <ControlButton
          onClick={onStepBackward}
          disabled={!canStepBackward}
          label="Step backward"
          icon="⏪"
        />
        <button
          onClick={onPlayPause}
          className={`
            p-3 rounded-full border-2 transition-all w-12 h-12 flex items-center justify-center
            ${isPlaying ? 'border-red-500 bg-red-500/10 hover:bg-red-500/20' : 'border-green-500 bg-green-500/10 hover:bg-green-500/20'}
          `}
          title={isPlaying ? 'Pause' : 'Play'}
        >
          <span className="text-2xl" aria-hidden="true">
            {isPlaying ? '⏸' : '▶️'}
          </span>
          <span className="sr-only">{isPlaying ? 'Pause' : 'Play'}</span>
        </button>
        <ControlButton
          onClick={onStepForward}
          disabled={!canStepForward}
          label="Step forward"
          icon="⏩"
        />
        <ControlButton
          onClick={onGoToEnd}
          disabled={!canGoToEnd}
          label="Go to end"
          icon="⏭"
        />
      </div>

      {/* Progress slider */}
      {onJumpToStep && totalSteps > 0 && (
        <div className="mb-4">
          <div className="flex items-center justify-between mb-2">
            <span className="text-sm text-slate-400">Progress</span>
            <span className="text-sm text-slate-300">
              Step {currentStep} / {totalSteps}
            </span>
          </div>
          <div className="relative">
            <input
              type="range"
              min="0"
              max={totalSteps}
              value={currentStep}
              onChange={handleSliderChange}
              onMouseDown={() => setIsDragging(true)}
              onMouseUp={() => setIsDragging(false)}
              onMouseLeave={() => setIsDragging(false)}
              className="w-full h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-500"
            />
            {!isPlaying && (
              <div
                className="absolute top-1/2 -translate-y-1/2 w-4 h-4 bg-blue-500 rounded-full pointer-events-none"
                style={{ left: `${(currentStep / totalSteps) * 100}%` }}
              />
            )}
          </div>
          <div className="flex justify-between text-xs text-slate-500 mt-1">
            <span>Start</span>
            <span>End</span>
          </div>
        </div>
      )}

      {/* Speed controls */}
      <div>
        <div className="flex items-center justify-between mb-2">
          <span className="text-sm text-slate-400">Playback Speed</span>
          <span className="text-sm font-semibold text-slate-300">
            {speedLabels[playbackSpeed]}
          </span>
        </div>
        <div className="flex gap-2">
          {speedOptions.map((speed) => (
            <button
              key={speed}
              onClick={() => onSpeedChange(speed)}
              className={`
                px-3 py-1 rounded border transition-all text-sm
                ${
                  playbackSpeed === speed
                    ? 'border-blue-500 bg-blue-500/20 text-blue-300'
                    : 'border-slate-600 hover:border-slate-400 hover:bg-slate-700 text-slate-300'
                }
              `}
            >
              {speedLabels[speed]}
            </button>
          ))}
        </div>
      </div>

      {/* Keyboard shortcuts hint */}
      <div className="mt-4 pt-4 border-t border-slate-700">
        <div className="text-xs text-slate-500 space-y-1">
          <div className="flex items-center gap-2">
            <kbd className="px-1.5 py-0.5 bg-slate-700 rounded text-slate-300 font-mono text-xs">
              Space
            </kbd>
            <span>Play/Pause</span>
          </div>
          <div className="flex items-center gap-2">
            <kbd className="px-1.5 py-0.5 bg-slate-700 rounded text-slate-300 font-mono text-xs">
              ←
            </kbd>
            <kbd className="px-1.5 py-0.5 bg-slate-700 rounded text-slate-300 font-mono text-xs">
              →
            </kbd>
            <span>Step back/forward</span>
          </div>
          <div className="flex items-center gap-2">
            <kbd className="px-1.5 py-0.5 bg-slate-700 rounded text-slate-300 font-mono text-xs">
              Home
            </kbd>
            <kbd className="px-1.5 py-0.5 bg-slate-700 rounded text-slate-300 font-mono text-xs">
              End
            </kbd>
            <span>Go to start/end</span>
          </div>
        </div>
      </div>
    </div>
  );
}
