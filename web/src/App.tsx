/**
 * Main App Component
 *
 * Integrates all replay viewer components into a cohesive application.
 * Manages replay state, playback controls, and component coordination.
 */

import { useState, useCallback, useEffect, useRef } from 'react';
import type { ReplayFile, ReplayAction } from './types/replay';
import type { BoardState } from './types/state';
import { createEmptyBoardState } from './types/state';
import { Reconstructor } from './engine/reconstructor';
import { Board } from './components/Board';
import { ReplayControls } from './components/ReplayControls';
import { GameLog } from './components/GameLog';
import { FileLoader } from './components/FileLoader';
import { getCardBatch, clearCardCache } from './api/scryfall';
import type { ScryfallCard } from './api/scryfall';

export function App() {
  // Replay data
  const [replayFile, setReplayFile] = useState<ReplayFile | null>(null);
  const [reconstructor, setReconstructor] = useState<Reconstructor | null>(null);

  // Current state
  const [currentStep, setCurrentStep] = useState(0);
  const [boardState, setBoardState] = useState<BoardState>(createEmptyBoardState());
  const [cardData, setCardData] = useState<Record<string, ScryfallCard>>({});

  // Playback state
  const [isPlaying, setIsPlaying] = useState(false);
  const [playbackSpeed, setPlaybackSpeed] = useState(1);
  const playbackIntervalRef = useRef<number | null>(null);

  // Derived state
  const totalSteps = replayFile?.actions.length ?? 0;
  const canStepForward = currentStep < totalSteps;
  const canStepBackward = currentStep > 0;
  const canGoToStart = currentStep > 0;
  const canGoToEnd = currentStep < totalSteps;

  // Player info
  const playerNames = replayFile?.header.players.reduce(
    (acc, player) => ({
      ...acc,
      [player.id]: player.name,
    }),
    {} as Record<string, string>,
  ) ?? {};
  const playerIds = replayFile?.header.players.map((p) => p.id) ?? [];

  // Load card data when replay file is loaded
  useEffect(() => {
    const loadCardData = async () => {
      if (!replayFile) {
        setCardData({});
        return;
      }

      // Extract unique card IDs from actions
      const cardIds = new Set<string>();
      replayFile.actions.forEach((action) => {
        const actionType = typeof action.action_type === 'object'
          ? action.action_type
          : { type: action.action_type };

        if ('card_id' in actionType && actionType.card_id) {
          cardIds.add(actionType.card_id);
        }
      });

      // Try to load card data from Scryfall
      if (cardIds.size > 0) {
        try {
          const cards = await getCardBatch(Array.from(cardIds));
          const cardMap = cards.reduce(
            (acc, card) => ({
              ...acc,
              [card.name]: card,
            }),
            {} as Record<string, ScryfallCard>,
          );
          setCardData(cardMap);
        } catch {
          // Silently fail - cards will render without images
          console.warn('Failed to load card data from Scryfall');
        }
      }
    };

    loadCardData();
  }, [replayFile]);

  // Handle file load
  const handleFileLoad = useCallback((file: ReplayFile) => {
    setReplayFile(file);
    setCurrentStep(0);
    setIsPlaying(false);

    // Create new reconstructor
    const newReconstructor = new Reconstructor();
    newReconstructor.loadReplay(file);
    setReconstructor(newReconstructor);

    // Initialize board state
    const initialState = newReconstructor.reconstruct(0);
    setBoardState(initialState);
  }, []);

  // Step to specific position
  const stepTo = useCallback(
    (step: number) => {
      if (!reconstructor) return;

      const clampedStep = Math.max(0, Math.min(step, totalSteps));
      setCurrentStep(clampedStep);

      const newState = reconstructor.reconstruct(clampedStep);
      setBoardState(newState);
    },
    [reconstructor, totalSteps],
  );

  // Step forward
  const stepForward = useCallback(() => {
    if (canStepForward) {
      stepTo(currentStep + 1);
    }
  }, [canStepForward, currentStep, stepTo]);

  // Step backward
  const stepBackward = useCallback(() => {
    if (canStepBackward) {
      stepTo(currentStep - 1);
    }
  }, [canStepBackward, currentStep, stepTo]);

  // Go to start
  const goToStart = useCallback(() => {
    stepTo(0);
  }, [stepTo]);

  // Go to end
  const goToEnd = useCallback(() => {
    stepTo(totalSteps);
  }, [totalSteps, stepTo]);

  // Toggle play/pause
  const togglePlayPause = useCallback(() => {
    setIsPlaying((prev) => !prev);
  }, []);

  // Handle playback interval
  useEffect(() => {
    if (isPlaying) {
      const interval = 1000 / playbackSpeed;

      playbackIntervalRef.current = window.setInterval(() => {
        if (canStepForward) {
          stepForward();
        } else {
          setIsPlaying(false);
        }
      }, interval);
    } else {
      if (playbackIntervalRef.current) {
        clearInterval(playbackIntervalRef.current);
        playbackIntervalRef.current = null;
      }
    }

    return () => {
      if (playbackIntervalRef.current) {
        clearInterval(playbackIntervalRef.current);
      }
    };
  import type { ReplayFile } from './types/replay';

  // Handle card click
  const handleCardClick = useCallback((cardId: string) => {
    console.log('Card clicked:', cardId);
    // Future: show card details modal
  }, []);

  // Handle action log click
  const handleActionClick = useCallback(
    (step: number) => {
      setIsPlaying(false);
      stepTo(step);
    },
    [stepTo],
  );

  // Handle speed change
  const handleSpeedChange = useCallback((speed: number) => {
    setPlaybackSpeed(speed);
  }, []);

  // Handle error
  const handleError = useCallback((error: string) => {
    console.error('Replay error:', error);
    // Future: show error toast
  }, []);

  // Clean up on unmount
  useEffect(() => {
    return () => {
      if (playbackIntervalRef.current) {
        clearInterval(playbackIntervalRef.current);
      }
      clearCardCache();
    };
  }, []);

  // Render empty state
  if (!replayFile) {
    return (
      <div className="min-h-screen bg-slate-950 flex items-center justify-center p-4">
        <div className="w-full max-w-lg">
          <FileLoader
            onFileLoad={handleFileLoad}
            onError={handleError}
            accept=".json"
            maxSizeBytes={50 * 1024 * 1024} // 50MB
          />
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-slate-950">
      {/* Header */}
      <header className="bg-slate-900/80 backdrop-blur border-b border-slate-800 px-4 py-3">
        <div className="max-w-full mx-auto flex items-center justify-between">
          <div>
            <h1 className="text-xl font-bold text-white">MTG Replay Viewer</h1>
            {replayFile.header.game_id && (
              <p className="text-sm text-slate-400">Game: {replayFile.header.game_id}</p>
            )}
          </div>
          {replayFile.header.format && (
            <div className="px-3 py-1 bg-slate-800 rounded text-slate-300 text-sm">
              {replayFile.header.format}
            </div>
          )}
        </div>
      </header>

      {/* Main content */}
      <div className="flex h-[calc(100vh-64px)]">
        {/* Left sidebar - Game Log */}
        <aside className="w-80 border-r border-slate-800 bg-slate-900/50 overflow-hidden flex flex-col">
          <GameLog
            actions={replayFile.actions}
            currentStep={currentStep}
            onActionClick={handleActionClick}
            playerNameMap={playerNames}
            autoScroll={true}
            showTimestamp={true}
            showPhase={true}
          />
        </aside>

        {/* Center - Board */}
        <main className="flex-1 overflow-auto p-4">
          <Board
            boardState={boardState}
            playerIds={playerIds}
            playerNames={playerNames}
            cardData={cardData}
            onCardClick={handleCardClick}
            zoneLayout="separate"
            showLifeTotals={true}
            showStack={true}
            showTurnInfo={true}
          />
        </main>

        {/* Right sidebar - Controls */}
        <aside className="w-72 border-l border-slate-800 bg-slate-900/50 overflow-y-auto">
          <div className="p-4 space-y-4">
            {/* Replay controls */}
            <ReplayControls
              isPlaying={isPlaying}
              canStepForward={canStepForward}
              canStepBackward={canStepBackward}
              canGoToStart={canGoToStart}
              canGoToEnd={canGoToEnd}
              currentStep={currentStep}
              totalSteps={totalSteps}
              playbackSpeed={playbackSpeed}
              onPlayPause={togglePlayPause}
              onStepForward={stepForward}
              onStepBackward={stepBackward}
              onGoToStart={goToStart}
              onGoToEnd={goToEnd}
              onSpeedChange={handleSpeedChange}
              onJumpToStep={stepTo}
            />

            {/* Player info */}
            <div className="bg-slate-900/80 backdrop-blur border border-slate-700 rounded-lg p-4">
              <h3 className="text-sm font-semibold text-slate-300 mb-2">Players</h3>
              <div className="space-y-2">
                {replayFile.header.players.map((player) => (
                  <div key={player.id} className="flex items-center justify-between">
                    <div className="text-sm text-slate-400">{player.name}</div>
                    <div className="text-xs text-slate-500 bg-slate-800 px-2 py-0.5 rounded">
                      {player.id}
                    </div>
                  </div>
                ))}
              </div>
            </div>

            {/* Game info */}
            <div className="bg-slate-900/80 backdrop-blur border border-slate-700 rounded-lg p-4">
              <h3 className="text-sm font-semibold text-slate-300 mb-2">Game Info</h3>
              <div className="space-y-1 text-xs text-slate-400">
                <div className="flex justify-between">
                  <span>Started:</span>
                  <span>
                    {new Date(replayFile.header.start_time).toLocaleString()}
                  </span>
                </div>
                {replayFile.header.end_time && (
                  <div className="flex justify-between">
                    <span>Ended:</span>
                    <span>
                      {new Date(replayFile.header.end_time).toLocaleString()}
                    </span>
                  </div>
                )}
                <div className="flex justify-between">
                  <span>Actions:</span>
                  <span>{replayFile.actions.length}</span>
                </div>
                {replayFile.header.result && (
                  <div className="flex justify-between">
                    <span>Winner:</span>
                    <span className="text-green-400">
                      {playerNames[replayFile.header.result.winner] ||
                        replayFile.header.result.winner}
                    </span>
                  </div>
                )}
              </div>
            </div>

            {/* Load new file button */}
            <button
              onClick={() => {
                setReplayFile(null);
                setReconstructor(null);
                setCurrentStep(0);
                setIsPlaying(false);
                setBoardState(createEmptyBoardState());
              }}
              className="w-full px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition-colors"
            >
              Load New Replay
            </button>
          </div>
        </aside>
      </div>
    </div>
  );
}
