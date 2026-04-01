/**
 * DeckList Display Component
 *
 * Shows the mainboard deck list, with sideboard changes highlighted for games 2+.
 * Accepts a card catalog and optional sideboard-in/out lists.
 */

import { useState } from 'react';

export interface DeckEntry {
  name: string;
  count: number;
}

export interface SideboardChanges {
  in: DeckEntry[];
  out: DeckEntry[];
}

export interface DeckListProps {
  mainboard: DeckEntry[];
  sideboard?: DeckEntry[];
  sideboardChanges?: SideboardChanges;
  gameNumber?: number;
  className?: string;
}

export function DeckList({
  mainboard,
  sideboard,
  sideboardChanges,
  gameNumber = 1,
  className = '',
}: DeckListProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const [showSideboard, setShowSideboard] = useState(false);

  const totalMain = mainboard.reduce((sum, e) => sum + e.count, 0);
  const totalSide = sideboard?.reduce((sum, e) => sum + e.count, 0) ?? 0;

  const sideboardInNames = new Set(sideboardChanges?.in.map((e) => e.name) ?? []);
  const sideboardOutNames = new Set(sideboardChanges?.out.map((e) => e.name) ?? []);

  return (
    <div className={`border-2 border-slate-700 bg-slate-900/80 rounded-lg ${className}`}>
      {/* Header */}
      <button
        className="w-full flex items-center justify-between p-3 hover:bg-slate-800/50 transition-colors"
        onClick={() => setIsExpanded(!isExpanded)}
      >
        <div className="flex items-center gap-2">
          <span className="text-sm font-semibold text-slate-200">Deck List</span>
          <span className="text-xs text-slate-500">({totalMain} cards)</span>
          {gameNumber > 1 && sideboardChanges && (
            <span className="text-xs bg-amber-600 text-white px-1.5 py-0.5 rounded">
              G{gameNumber} sideboarded
            </span>
          )}
        </div>
        <span className="text-slate-400 text-sm">{isExpanded ? '\u25B2' : '\u25BC'}</span>
      </button>

      {/* Content */}
      {isExpanded && (
        <div className="px-3 pb-3">
          {/* Sideboard changes for games 2+ */}
          {gameNumber > 1 && sideboardChanges && (
            <div className="mb-3 border border-amber-900/50 bg-amber-950/20 rounded p-2">
              <div className="text-xs font-semibold text-amber-300 mb-1">
                Sideboard Changes (Game {gameNumber})
              </div>
              {sideboardChanges.in.length > 0 && (
                <div className="mb-1">
                  <span className="text-xs text-green-400 font-semibold">IN: </span>
                  {sideboardChanges.in.map((entry, i) => (
                    <span key={entry.name} className="text-xs text-green-300">
                      {i > 0 && ', '}
                      {entry.count}x {entry.name}
                    </span>
                  ))}
                </div>
              )}
              {sideboardChanges.out.length > 0 && (
                <div>
                  <span className="text-xs text-red-400 font-semibold">OUT: </span>
                  {sideboardChanges.out.map((entry, i) => (
                    <span key={entry.name} className="text-xs text-red-300">
                      {i > 0 && ', '}
                      {entry.count}x {entry.name}
                    </span>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Mainboard */}
          <div className="mb-2">
            <div className="text-xs font-semibold text-slate-400 mb-1">
              Mainboard ({totalMain})
            </div>
            <div className="grid grid-cols-2 gap-x-4 gap-y-0.5 max-h-64 overflow-y-auto">
              {mainboard.map((entry) => {
                const isSidedIn = sideboardInNames.has(entry.name);
                const isSidedOut = sideboardOutNames.has(entry.name);
                return (
                  <div
                    key={entry.name}
                    className={`text-xs py-0.5 flex justify-between ${
                      isSidedIn
                        ? 'text-green-300'
                        : isSidedOut
                          ? 'text-red-300 line-through'
                          : 'text-slate-300'
                    }`}
                  >
                    <span className="truncate">{entry.name}</span>
                    <span className="text-slate-500 ml-1 flex-shrink-0">{entry.count}</span>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Sideboard toggle */}
          {sideboard && sideboard.length > 0 && (
            <div>
              <button
                className="text-xs text-slate-500 hover:text-slate-300 transition-colors mb-1"
                onClick={() => setShowSideboard(!showSideboard)}
              >
                {showSideboard ? '\u25B2' : '\u25BC'} Sideboard ({totalSide})
              </button>
              {showSideboard && (
                <div className="grid grid-cols-2 gap-x-4 gap-y-0.5 max-h-40 overflow-y-auto">
                  {sideboard.map((entry) => (
                    <div
                      key={entry.name}
                      className="text-xs text-slate-400 py-0.5 flex justify-between"
                    >
                      <span className="truncate">{entry.name}</span>
                      <span className="text-slate-600 ml-1 flex-shrink-0">{entry.count}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
