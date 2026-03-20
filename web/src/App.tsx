import { useState, useCallback, useEffect, useRef } from 'react';
import type { ReplayFile } from './types/replay';
import { getWinnerId, getResultLabel } from './types/replay';
import type { BoardState } from './types/state';
import { createEmptyBoardState } from './types/state';
import { Reconstructor } from './engine/reconstructor';
import { Board } from './components/Board';
import { ReplayControls } from './components/ReplayControls';
import { GameLog } from './components/GameLog';
import { FileLoader } from './components/FileLoader';

export function App() {
  const [replayFile, setReplayFile] = useState<ReplayFile | null>(null);
  const [reconstructor, setReconstructor] = useState<Reconstructor | null>(null);
  const [currentStep, setCurrentStep] = useState(0);
  const [boardState, setBoardState] = useState<BoardState>(createEmptyBoardState());

  const [isPlaying, setIsPlaying] = useState(false);
  const [playbackSpeed, setPlaybackSpeed] = useState(1);
  const playbackIntervalRef = useRef<number | null>(null);

  const totalSteps = replayFile?.actions.length ?? 0;
  const canStepForward = currentStep < totalSteps;
  const canStepBackward = currentStep > 0;

  const playerNames = replayFile?.header.players.reduce(
    (acc, player) => ({ ...acc, [player.player_id]: player.name }),
    {} as Record<string, string>,
  ) ?? {};
  const playerIds = replayFile?.header.players.map((p) => p.player_id) ?? [];

  const handleFileLoad = useCallback((file: ReplayFile) => {
    setReplayFile(file);
    setCurrentStep(0);
    setIsPlaying(false);

    const r = new Reconstructor();
    r.loadReplay(file);
    setReconstructor(r);

    // Initialize to step 0 (empty board before any actions)
    setBoardState(r.reconstruct(0));

    // Resolve unnamed cards via Scryfall using MTGO IDs
    if (file.card_textures && Object.keys(file.card_textures).length > 0) {
      r.resolveCardTextures(file.card_textures).then((count) => {
        if (count > 0) {
          console.log(`Resolved ${count} card names from Scryfall`);
          // Merge resolved names back into the replay file for GameLog etc.
          setReplayFile((prev) => prev ? {
            ...prev,
            card_names: { ...prev.card_names, ...r.getCardNames() },
          } : prev);
        }
      });
    }
  }, []);

  const stepTo = useCallback(
    (step: number) => {
      if (!reconstructor) return;
      const clamped = Math.max(0, Math.min(step, totalSteps));
      setCurrentStep(clamped);
      setBoardState(reconstructor.reconstruct(clamped));
    },
    [reconstructor, totalSteps],
  );

  const stepForward = useCallback(() => {
    if (canStepForward) stepTo(currentStep + 1);
  }, [canStepForward, currentStep, stepTo]);

  const stepBackward = useCallback(() => {
    if (canStepBackward) stepTo(currentStep - 1);
  }, [canStepBackward, currentStep, stepTo]);

  const goToStart = useCallback(() => stepTo(0), [stepTo]);
  const goToEnd = useCallback(() => stepTo(totalSteps), [totalSteps, stepTo]);
  const togglePlayPause = useCallback(() => setIsPlaying((prev) => !prev), []);

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
    } else if (playbackIntervalRef.current) {
      clearInterval(playbackIntervalRef.current);
      playbackIntervalRef.current = null;
    }
    return () => {
      if (playbackIntervalRef.current) clearInterval(playbackIntervalRef.current);
    };
  });

  const handleActionClick = useCallback(
    (step: number) => {
      setIsPlaying(false);
      stepTo(step);
    },
    [stepTo],
  );

  const handleCardClick = useCallback((cardId: string) => {
    console.log('Card clicked:', cardId);
  }, []);

  const handleSpeedChange = useCallback((speed: number) => {
    setPlaybackSpeed(speed);
  }, []);

  const handleError = useCallback((error: string) => {
    console.error('Replay error:', error);
  }, []);

  useEffect(() => {
    return () => {
      if (playbackIntervalRef.current) clearInterval(playbackIntervalRef.current);
    };
  }, []);

  if (!replayFile) {
    return (
      <div className="min-h-screen bg-slate-950 flex items-center justify-center p-4">
        <div className="w-full max-w-lg">
          <FileLoader
            onFileLoad={handleFileLoad}
            onError={handleError}
            accept=".json,.flashback"
            maxSizeBytes={50 * 1024 * 1024}
          />
        </div>
      </div>
    );
  }

  const winnerId = getWinnerId(replayFile.header.result);

  return (
    <div className="min-h-screen bg-slate-950">
      <header className="bg-slate-900/80 backdrop-blur border-b border-slate-800 px-4 py-3">
        <div className="max-w-full mx-auto flex items-center justify-between">
          <div>
            <h1 className="text-xl font-bold text-white">MTG Replay Viewer</h1>
            <p className="text-sm text-slate-400">Game: {replayFile.header.game_id}</p>
          </div>
          {replayFile.header.format && (
            <div className="px-3 py-1 bg-slate-800 rounded text-slate-300 text-sm">
              {replayFile.header.format}
            </div>
          )}
        </div>
      </header>

      <div className="flex h-[calc(100vh-64px)]">
        <aside className="w-80 border-r border-slate-800 bg-slate-900/50 overflow-hidden flex flex-col">
          <GameLog
            actions={replayFile.actions}
            currentStep={currentStep}
            onActionClick={handleActionClick}
            playerNameMap={playerNames}
            cardNameMap={replayFile.card_names ?? {}}
            autoScroll={true}
          />
        </aside>

        <main className="flex-1 overflow-auto p-4">
          <Board
            boardState={boardState}
            playerIds={playerIds}
            playerNames={playerNames}
            onCardClick={handleCardClick}
          />
        </main>

        <aside className="w-72 border-l border-slate-800 bg-slate-900/50 overflow-y-auto">
          <div className="p-4 space-y-4">
            <ReplayControls
              isPlaying={isPlaying}
              canStepForward={canStepForward}
              canStepBackward={canStepBackward}
              canGoToStart={currentStep > 0}
              canGoToEnd={currentStep < totalSteps}
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

            <div className="bg-slate-900/80 backdrop-blur border border-slate-700 rounded-lg p-4">
              <h3 className="text-sm font-semibold text-slate-300 mb-2">Players</h3>
              <div className="space-y-2">
                {replayFile.header.players.map((player) => (
                  <div key={player.player_id} className="flex items-center justify-between">
                    <div className="text-sm text-slate-400">{player.name}</div>
                    <div className="text-xs text-slate-500 bg-slate-800 px-2 py-0.5 rounded">
                      {player.player_id}
                    </div>
                  </div>
                ))}
              </div>
            </div>

            <div className="bg-slate-900/80 backdrop-blur border border-slate-700 rounded-lg p-4">
              <h3 className="text-sm font-semibold text-slate-300 mb-2">Game Info</h3>
              <div className="space-y-1 text-xs text-slate-400">
                {replayFile.header.start_time && (
                  <div className="flex justify-between">
                    <span>Started:</span>
                    <span>{new Date(replayFile.header.start_time).toLocaleString()}</span>
                  </div>
                )}
                {replayFile.header.end_time && (
                  <div className="flex justify-between">
                    <span>Ended:</span>
                    <span>{new Date(replayFile.header.end_time).toLocaleString()}</span>
                  </div>
                )}
                <div className="flex justify-between">
                  <span>Actions:</span>
                  <span>{replayFile.actions.length}</span>
                </div>
                <div className="flex justify-between">
                  <span>Result:</span>
                  <span className={winnerId ? 'text-green-400' : ''}>
                    {getResultLabel(replayFile.header.result)}
                  </span>
                </div>
              </div>
            </div>

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
