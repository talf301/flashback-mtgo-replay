/**
 * Game Log Sidebar Component
 *
 * Displays a chronological list of game actions with filtering and search.
 * Supports auto-scroll to latest action and clickable log entries.
 */

import { useState, useRef, useEffect, useMemo } from 'react';
import type { ReplayAction } from '../types/replay';

export interface GameLogProps {
  actions: ReplayAction[];
  currentStep?: number;
  onActionClick?: (step: number) => void;
  playerNameMap?: Record<string, string>;
  className?: string;
  maxEntries?: number;
  autoScroll?: boolean;
  showTimestamp?: boolean;
  showPhase?: boolean;
}

export function GameLog({
  actions,
  currentStep,
  onActionClick,
  playerNameMap = {},
  className = '',
  maxEntries,
  autoScroll = true,
  showTimestamp = true,
  showPhase = true,
}: GameLogProps) {
  const [searchQuery, setSearchQuery] = useState('');
  const [filterType, setFilterType] = useState<string>('all');
  const scrollRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when new actions arrive
  useEffect(() => {
    if (autoScroll && scrollRef.current && actions.length > 0) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [actions, autoScroll]);

  // Filter actions based on search and type filter
  const filteredActions = useMemo(() => {
    let filtered = [...actions];

    // Apply type filter
    if (filterType !== 'all') {
      filtered = filtered.filter((action) => {
        const actionType = typeof action.action_type === 'object'
          ? action.action_type.type
          : action.action_type;
        return actionType === filterType;
      });
    }

    // Apply search query
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter((action) => {
        const actionType = typeof action.action_type === 'object'
          ? action.action_type.type
          : action.action_type;
        return (
          actionType.toLowerCase().includes(query) ||
          action.active_player?.toLowerCase().includes(query) ||
          action.phase?.toLowerCase().includes(query)
        );
      });
    }

    // Limit entries if specified
    if (maxEntries) {
      filtered = filtered.slice(-maxEntries);
    }

    return filtered;
  }, [actions, searchQuery, filterType, maxEntries]);

  // Get unique action types for filter dropdown
  const actionTypes = useMemo(() => {
    const types = new Set<string>();
    actions.forEach((action) => {
      const actionType = typeof action.action_type === 'object'
        ? action.action_type.type
        : action.action_type;
      types.add(actionType);
    });
    return Array.from(types).sort();
  }, [actions]);

  const formatTimestamp = (timestamp: string) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString(undefined, {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  };

  const formatPhase = (phase?: string) => {
    if (!phase) return '';
    const phaseNames: Record<string, string> = {
      beginning: 'Beginning',
      main1: 'Main 1',
      combat: 'Combat',
      main2: 'Main 2',
      end: 'End',
    };
    return phaseNames[phase] || phase;
  };

  const getActionDescription = (action: ReplayAction): string => {
    const actionType = typeof action.action_type === 'object'
      ? action.action_type
      : { type: action.action_type };

    switch (actionType.type) {
      case 'DrawCard':
        return 'drew a card';
      case 'PlayLand':
        return 'played a land';
      case 'CastSpell':
        return 'cast a spell';
      case 'ActivateAbility':
        return 'activated an ability';
      case 'Attack':
        return 'attacked';
      case 'Block':
        return 'blocked';
      case 'Damage':
        return 'dealt damage';
      case 'LifeChange':
        const lifeChange = actionType as { old: number; new: number };
        const diff = lifeChange.new - lifeChange.old;
        return `${diff > 0 ? 'gained' : 'lost'} ${Math.abs(diff)} life`;
      case 'PassPriority':
        return 'passed priority';
      case 'ResolveSpell':
        return 'resolved a spell';
      case 'TokenCreate':
        return 'created a token';
      case 'ZoneChange':
        const zoneChange = actionType as { from: string; to: string };
        return `moved card from ${zoneChange.from} to ${zoneChange.to}`;
      case 'CounterAdd':
        const counterAdd = actionType as { counter_type: string; amount: number };
        return `added ${counterAdd.amount} ${counterAdd.counter_type} counter(s)`;
      case 'GameEnd':
        const gameEnd = actionType as { type: 'GameEnd'; winner: string };
        return `game ended - ${gameEnd.winner} won`;
      default:
        return `performed ${actionType.type}`;
    }
  };

  const getPlayerName = (playerId?: string): string => {
    if (!playerId) return '';
    return playerNameMap[playerId] || playerId;
  };

  const getActionColor = (actionType: string): string => {
    const colorMap: Record<string, string> = {
      DrawCard: 'text-blue-400',
      PlayLand: 'text-green-400',
      CastSpell: 'text-purple-400',
      ActivateAbility: 'text-yellow-400',
      Attack: 'text-red-400',
      Block: 'text-orange-400',
      Damage: 'text-red-500',
      LifeChange: 'text-pink-400',
      PassPriority: 'text-slate-500',
      ResolveSpell: 'text-cyan-400',
      TokenCreate: 'text-emerald-400',
      ZoneChange: 'text-indigo-400',
      CounterAdd: 'text-amber-400',
      GameEnd: 'text-red-600 font-bold',
    };
    return colorMap[actionType] || 'text-slate-400';
  };

  return (
    <div className={`bg-slate-900/80 backdrop-blur border border-slate-700 rounded-lg flex flex-col h-full ${className}`}>
      {/* Header */}
      <div className="p-4 border-b border-slate-700">
        <h2 className="text-lg font-semibold text-white mb-3">Game Log</h2>

        {/* Search input */}
        <div className="mb-3">
          <input
            type="text"
            placeholder="Search actions..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full px-3 py-2 bg-slate-800 border border-slate-600 rounded-lg text-white placeholder-slate-500 focus:outline-none focus:border-slate-400"
          />
        </div>

        {/* Type filter */}
        {actionTypes.length > 0 && (
          <div className="flex items-center gap-2">
            <label htmlFor="action-filter" className="text-sm text-slate-400">
              Filter:
            </label>
            <select
              id="action-filter"
              value={filterType}
              onChange={(e) => setFilterType(e.target.value)}
              className="flex-1 px-3 py-2 bg-slate-800 border border-slate-600 rounded-lg text-white focus:outline-none focus:border-slate-400"
            >
              <option value="all">All Actions</option>
              {actionTypes.map((type) => (
                <option key={type} value={type}>
                  {type}
                </option>
              ))}
            </select>
          </div>
        )}
      </div>

      {/* Log entries */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto p-4 space-y-2">
        {filteredActions.length === 0 ? (
          <div className="text-center text-slate-500 italic py-8">
            No actions to display
          </div>
        ) : (
          filteredActions.map((action, index) => {
            const actionType = typeof action.action_type === 'object'
              ? action.action_type.type
              : action.action_type;

            const isCurrentStep = currentStep !== undefined && index === currentStep;
            const stepNumber = actions.indexOf(action);

            return (
              <div
                key={action.timestamp}
                onClick={() => onActionClick?.(stepNumber)}
                className={`
                  p-3 rounded-lg border transition-all cursor-pointer
                  ${isCurrentStep ? 'border-blue-500 bg-blue-500/10' : 'border-slate-700 hover:border-slate-500 hover:bg-slate-800/50'}
                `}
              >
                <div className="flex items-start gap-2">
                  {showTimestamp && (
                    <span className="text-xs text-slate-500 flex-shrink-0">
                      {formatTimestamp(action.timestamp)}
                    </span>
                  )}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-xs text-slate-500">#{stepNumber + 1}</span>
                      {action.turn && (
                        <span className="text-xs text-slate-500">
                          Turn {action.turn}
                        </span>
                      )}
                      {showPhase && action.phase && (
                        <span className="text-xs text-slate-400">
                          {formatPhase(action.phase)}
                        </span>
                      )}
                    </div>
                    <div className="flex items-center gap-2">
                      {action.active_player && (
                        <span className="text-sm font-medium text-slate-300">
                          {getPlayerName(action.active_player)}
                        </span>
                      )}
                      <span className={`text-sm ${getActionColor(actionType)}`}>
                        {getActionDescription(action)}
                      </span>
                    </div>
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>

      {/* Footer */}
      {filteredActions.length > 0 && maxEntries && (
        <div className="p-3 border-t border-slate-700 text-xs text-slate-500">
          Showing {filteredActions.length} of {actions.length} actions
        </div>
      )}
    </div>
  );
}
