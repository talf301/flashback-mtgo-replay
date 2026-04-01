import { useMemo } from 'react';
import type { BoardState } from '../types/state';
import { Zone } from './Zone';
import { ManaPool } from './ManaPool';
import { CombatPairings } from './CombatPairings';

export interface BoardProps {
  boardState: BoardState;
  playerIds: string[];
  playerNames: Record<string, string>;
  onCardClick?: (cardId: string) => void;
  className?: string;
}

export function Board({
  boardState,
  playerIds,
  playerNames,
  onCardClick,
  className = '',
}: BoardProps) {
  const zonesByPlayer = useMemo(() => {
    const grouped: Record<string, typeof boardState.zones> = {};
    const shared: typeof boardState.zones = [];

    for (const zone of boardState.zones) {
      if (zone.owner) {
        if (!grouped[zone.owner]) grouped[zone.owner] = [];
        grouped[zone.owner].push(zone);
      } else {
        shared.push(zone);
      }
    }

    return { grouped, shared };
  }, [boardState.zones]);

  const playerLookup = useMemo(() => {
    const map: Record<string, (typeof boardState.players)[number]> = {};
    for (const p of boardState.players) {
      map[p.name] = p;
    }
    return map;
  }, [boardState.players]);

  const isCombatPhase = [
    'begin_combat',
    'declare_attackers',
    'declare_blockers',
    'combat_damage',
    'end_of_combat',
  ].includes(boardState.phase);

  const combatCards = useMemo(() => {
    if (!isCombatPhase) return [];
    return boardState.zones
      .flatMap((z) => z.cards)
      .filter((c) => c.combatStatus.attacking || c.combatStatus.blocking);
  }, [boardState.zones, isCombatPhase]);

  const phaseLabel = (phase: string): string => {
    const map: Record<string, string> = {
      untap: 'Untap',
      upkeep: 'Upkeep',
      draw: 'Draw',
      precombat_main: 'Main 1',
      begin_combat: 'Begin Combat',
      declare_attackers: 'Attackers',
      declare_blockers: 'Blockers',
      combat_damage: 'Combat Damage',
      end_of_combat: 'End Combat',
      postcombat_main: 'Main 2',
      end_of_turn: 'End Step',
      cleanup: 'Cleanup',
    };
    return map[phase] || phase;
  };

  return (
    <div className={`w-full ${className}`}>
      {/* Turn / Phase bar */}
      <div className="bg-slate-800/50 border border-slate-700 rounded-lg p-3 mb-4">
        <div className="flex items-center gap-4">
          <div className="flex-1">
            <span className="text-slate-400 text-sm">Turn: </span>
            <span className="text-white font-bold">{boardState.turn}</span>
          </div>
          <div className="flex-1">
            <span className="text-slate-400 text-sm">Phase: </span>
            <span className="text-white font-bold">{phaseLabel(boardState.phase)}</span>
          </div>
          {boardState.activePlayer && (
            <div className="flex-1">
              <span className="text-slate-400 text-sm">Active: </span>
              <span className="text-yellow-400 font-bold">
                {playerNames[boardState.activePlayer] || boardState.activePlayer}
              </span>
            </div>
          )}
        </div>
      </div>

      {/* Life totals and mana pools */}
      <div className="flex gap-4 mb-4 flex-wrap">
        {playerIds.map((pid) => {
          const player = playerLookup[pid];
          const life = player?.life ?? 20;
          const isActive = boardState.activePlayer === pid;
          return (
            <div
              key={pid}
              className={`px-4 py-2 rounded-lg border-2 ${
                isActive ? 'border-yellow-500 bg-yellow-500/10' : 'border-slate-600 bg-slate-800'
              } transition-colors`}
            >
              <div className="text-sm text-slate-400">{playerNames[pid] || pid}</div>
              <div className={`text-2xl font-bold ${life <= 5 ? 'text-red-400' : 'text-white'}`}>{life}</div>
              <div className="text-xs text-slate-500">Life</div>
              {player?.manaPool && <ManaPool manaPool={player.manaPool} />}
            </div>
          );
        })}
      </div>

      {/* Combat pairings */}
      {isCombatPhase && combatCards.length > 0 && (
        <CombatPairings cards={combatCards} playerNames={playerNames} />
      )}

      {/* Stack */}
      {boardState.stack.length > 0 && (
        <div className="mb-4 border-2 border-purple-900/50 bg-purple-950/30 rounded-lg p-3">
          <h3 className="text-sm font-semibold text-purple-300 mb-2 flex items-center gap-2">
            <span className="w-2 h-2 bg-purple-500 rounded-full animate-pulse" />
            Stack ({boardState.stack.length})
          </h3>
          <div className="flex gap-2 overflow-x-auto pb-2">
            {boardState.stack.map((obj, i) => (
              <div
                key={obj.id}
                className="flex-shrink-0 w-48 bg-slate-800 border border-slate-600 rounded-lg p-2"
                style={{ transform: `translateY(${i * 4}px)`, zIndex: boardState.stack.length - i }}
              >
                <div className="text-xs text-purple-400 mb-1">
                  {i === 0 ? 'Top of stack' : `Stack #${boardState.stack.length - i}`}
                </div>
                <div className="text-sm font-semibold text-white">#{obj.id}</div>
                {obj.controller && (
                  <div className="text-xs text-slate-400">
                    {playerNames[obj.controller] || obj.controller}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Per-player zones */}
      <div className="space-y-4">
        {playerIds.map((pid) => {
          const playerZones = zonesByPlayer.grouped[pid] || [];
          if (playerZones.length === 0) return null;

          return (
            <div key={pid} className="bg-slate-900/50 border border-slate-700 rounded-lg p-4">
              <h3 className="text-lg font-semibold text-white mb-3 flex items-center gap-2">
                {boardState.activePlayer === pid && (
                  <span className="w-3 h-3 bg-yellow-500 rounded-full animate-pulse" />
                )}
                {playerNames[pid] || pid}
              </h3>
              <div className="space-y-3">
                {playerZones.map((zone) => (
                  <Zone
                    key={`${zone.name}-${zone.owner}`}
                    zone={zone}
                    layout="row"
                    cardSize="normal"
                    onCardClick={onCardClick}
                    showOwner={false}
                  />
                ))}
              </div>
            </div>
          );
        })}

        {zonesByPlayer.shared.length > 0 && (
          <div className="bg-slate-900/50 border border-slate-700 rounded-lg p-4">
            <h3 className="text-lg font-semibold text-white mb-3">Shared Zones</h3>
            <div className="space-y-3">
              {zonesByPlayer.shared.map((zone) => (
                <Zone
                  key={zone.name}
                  zone={zone}
                  layout="row"
                  cardSize="normal"
                  onCardClick={onCardClick}
                  showOwner={false}
                />
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
