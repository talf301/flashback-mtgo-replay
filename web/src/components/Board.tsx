/**
 * Main Board Display Component
 *
 * Renders the full game board with zones, life totals, turn indicator, and stack.
 * Provides the central view of the MTG game state.
 */

import { useMemo } from 'react';
import type { BoardState } from '../types/state';
import { Zone } from './Zone';
import type { ZoneProps } from './Zone';
import type { ScryfallCard } from '../api/scryfall';

export interface BoardProps {
  boardState: BoardState;
  playerIds: string[];
  playerNames: Record<string, string>;
  cardData: Record<string, ScryfallCard>;
  zoneLayout?: 'separate' | 'compact';
  showLifeTotals?: boolean;
  showStack?: boolean;
  showTurnInfo?: boolean;
  onCardClick?: (cardId: string) => void;
  className?: string;
}

export function Board({
  boardState,
  playerIds,
  playerNames,
  cardData,
  zoneLayout = 'separate',
  showLifeTotals = true,
  showStack = true,
  showTurnInfo = true,
  onCardClick,
  className = '',
}: BoardProps) {
  // Group zones by player
  const zonesByPlayer = useMemo(() => {
    const grouped: Record<string, typeof boardState.zones> = {};
    const sharedZones: typeof boardState.zones = [];

    for (const zone of boardState.zones) {
      if (zone.owner) {
        if (!grouped[zone.owner]) {
          grouped[zone.owner] = [];
        }
        grouped[zone.owner].push(zone);
      } else {
        sharedZones.push(zone);
      }
    }

    return { grouped, sharedZones };
  }, [boardState.zones]);

  const renderLifeTotals = () => {
    if (!showLifeTotals) return null;

    return (
      <div className="flex gap-4 mb-4 flex-wrap">
        {playerIds.map((playerId) => {
          const lifeTotal = boardState.lifeTotals[playerId] ?? 20;
          const isActivePlayer = boardState.activePlayer === playerId;

          return (
            <div
              key={playerId}
              className={`px-4 py-2 rounded-lg border-2 ${
                isActivePlayer
                  ? 'border-yellow-500 bg-yellow-500/10'
                  : 'border-slate-600 bg-slate-800'
              } transition-colors`}
            >
              <div className="text-sm text-slate-400">{playerNames[playerId] || 'Unknown'}</div>
              <div className="text-2xl font-bold text-white">{lifeTotal}</div>
              <div className="text-xs text-slate-500">Life</div>
            </div>
          );
        })}
      </div>
    );
  };

  const renderTurnInfo = () => {
    if (!showTurnInfo) return null;

    const phaseNames: Record<string, string> = {
      beginning: 'Beginning',
      main1: 'Main 1',
      combat: 'Combat',
      main2: 'Main 2',
      end: 'End',
    };

    return (
      <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-3 mb-4">
        <div className="flex items-center gap-4">
          <div className="flex-1">
            <span className="text-slate-400 text-sm">Turn: </span>
            <span className="text-white font-bold">{boardState.turn}</span>
          </div>
          <div className="flex-1">
            <span className="text-slate-400 text-sm">Phase: </span>
            <span className="text-white font-bold">{phaseNames[boardState.phase]}</span>
          </div>
          {boardState.activePlayer && (
            <div className="flex-1">
              <span className="text-slate-400 text-sm">Active Player: </span>
              <span className="text-yellow-400 font-bold">
                {playerNames[boardState.activePlayer] || boardState.activePlayer}
              </span>
            </div>
          )}
        </div>
      </div>
    );
  };

  const renderStack = () => {
    if (!showStack || boardState.stack.length === 0) return null;

    return (
      <div className="mb-4 border-2 border-purple-900/50 bg-purple-950/30 rounded-lg p-3">
        <h3 className="text-sm font-semibold text-purple-300 mb-2 flex items-center gap-2">
          <span className="w-2 h-2 bg-purple-500 rounded-full animate-pulse" />
          Stack ({boardState.stack.length})
        </h3>
        <div className="flex gap-2 overflow-x-auto pb-2">
          {boardState.stack.map((stackObject, index) => {
            const cardData = cardData[stackObject.card_id];
            const cardName = cardData?.name || 'Unknown Spell';

            return (
              <div
                key={stackObject.id}
                className="flex-shrink-0 w-48 bg-slate-800 border border-slate-600 rounded-lg p-2"
                style={{
                  transform: `translateY(${index * 4}px)`,
                  zIndex: boardState.stack.length - index,
                }}
              >
                <div className="text-xs text-purple-400 mb-1">
                  {index === 0 ? 'Resolving...' : 'Waiting...'}
                </div>
                <div className="text-sm font-semibold text-white truncate">{cardName}</div>
                {stackObject.targets.length > 0 && (
                  <div className="text-xs text-slate-400 mt-1">
                    Targets: {stackObject.targets.length}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    );
  };

  const renderPlayerBoard = (playerId: string) => {
    const playerZones = zonesByPlayer.grouped[playerId] || [];

    if (playerZones.length === 0) return null;

    return (
      <div key={playerId} className="bg-slate-900/50 border border-slate-700 rounded-lg p-4 mb-4">
        <h3 className="text-lg font-semibold text-white mb-3 flex items-center gap-2">
          {boardState.activePlayer === playerId && (
            <span className="w-3 h-3 bg-yellow-500 rounded-full animate-pulse" />
          )}
          {playerNames[playerId] || playerId}
        </h3>
        <div
          className={
            zoneLayout === 'compact'
              ? 'grid grid-cols-1 md:grid-cols-2 gap-3'
              : 'space-y-3'
          }
        >
          {playerZones.map((zone) => (
            <Zone
              key={`${zone.name}-${zone.owner}`}
              zone={zone}
              layout={zoneLayout === 'compact' ? 'grid' : 'row'}
              cardSize={zoneLayout === 'compact' ? 'small' : 'normal'}
              onCardClick={onCardClick}
              showOwner={false}
            />
          ))}
        </div>
      </div>
    );
  };

  const renderSharedZones = () => {
    if (zonesByPlayer.sharedZones.length === 0) return null;

    return (
      <div className="bg-slate-900/50 border border-slate-700 rounded-lg p-4">
        <h3 className="text-lg font-semibold text-white mb-3">Shared Zones</h3>
        <div
          className={
            zoneLayout === 'compact'
              ? 'grid grid-cols-1 md:grid-cols-2 gap-3'
              : 'space-y-3'
          }
        >
          {zonesByPlayer.sharedZones.map((zone) => (
            <Zone
              key={zone.name}
              zone={zone}
              layout={zoneLayout === 'compact' ? 'grid' : 'row'}
              cardSize={zoneLayout === 'compact' ? 'small' : 'normal'}
              onCardClick={onCardClick}
              showOwner={false}
            />
          ))}
        </div>
      </div>
    );
  };

  return (
    <div className={`w-full ${className}`}>
      {renderTurnInfo()}
      {renderLifeTotals()}
      {renderStack()}

      <div className="space-y-4">
        {playerIds.map(renderPlayerBoard)}
        {renderSharedZones()}
      </div>
    </div>
  );
}
