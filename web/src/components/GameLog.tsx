import { useState, useRef, useEffect, useMemo } from 'react';
import type { EventEntry } from '../types/replay';

export interface GameLogProps {
  actions: EventEntry[];
  currentStep?: number;
  onActionClick?: (step: number) => void;
  playerNameMap?: Record<string, string>;
  cardNameMap?: Record<string, string>;
  className?: string;
  maxEntries?: number;
  autoScroll?: boolean;
}

export function GameLog({
  actions,
  currentStep,
  onActionClick,
  playerNameMap = {},
  cardNameMap = {},
  className = '',
  maxEntries,
  autoScroll = true,
}: GameLogProps) {
  const [searchQuery, setSearchQuery] = useState('');
  const [filterType, setFilterType] = useState<string>('all');
  const currentRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (autoScroll && currentRef.current) {
      currentRef.current.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    }
  }, [currentStep, autoScroll]);

  const parsedActions = useMemo(
    () => actions.map((entry, i) => ({
      index: i,
      entry,
      type: entry.event.type,
      data: entry.event as Record<string, unknown>,
    })),
    [actions],
  );

  const filteredActions = useMemo(() => {
    let filtered = parsedActions;

    if (filterType !== 'all') {
      filtered = filtered.filter((a) => a.type === filterType);
    }

    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter((a) =>
        a.type.toLowerCase().includes(query) ||
        a.entry.active_player?.toLowerCase().includes(query) ||
        a.entry.phase?.toLowerCase().includes(query),
      );
    }

    if (maxEntries) {
      filtered = filtered.slice(-maxEntries);
    }

    return filtered;
  }, [parsedActions, searchQuery, filterType, maxEntries]);

  const actionTypes = useMemo(() => {
    const types = new Set<string>();
    parsedActions.forEach((a) => types.add(a.type));
    return Array.from(types).sort();
  }, [parsedActions]);

  const getPlayerName = (id?: string): string => {
    if (!id) return '';
    return playerNameMap[id] || id;
  };

  const cardName = (id: string) => cardNameMap[id] || `#${id}`;

  const getActionDescription = (type: string, data: Record<string, unknown>): string => {
    switch (type) {
      case 'DrawCard':
        return `drew ${cardName(data.card_id as string)}`;
      case 'PlayLand':
        return `played ${cardName(data.card_id as string)}`;
      case 'CastSpell':
        return `cast ${cardName(data.card_id as string)}`;
      case 'ActivateAbility':
        return `activated ${cardName(data.card_id as string)}`;
      case 'Attack':
        return `${cardName(data.attacker_id as string)} attacked`;
      case 'Block':
        return `${cardName(data.blocker_id as string)} blocked`;
      case 'Resolve':
        return `${cardName(data.card_id as string)} resolved`;
      case 'LifeChange': {
        const diff = (data.new_life as number) - (data.old_life as number);
        return `${diff > 0 ? 'gained' : 'lost'} ${Math.abs(diff)} life`;
      }
      case 'ZoneTransition':
        return `${data.from_zone} -> ${data.to_zone}`;
      case 'TapPermanent':
        return `tapped a permanent`;
      case 'UntapPermanent':
        return `untapped a permanent`;
      case 'DamageMarked':
        return `${data.damage} damage marked`;
      case 'SummoningSickness':
        return data.has_sickness ? `summoning sick` : `ready to act`;
      case 'FaceDown':
        return `turned face down`;
      case 'FaceUp':
        return `turned face up`;
      case 'Attach':
        return `attached`;
      case 'Detach':
        return `detached`;
      case 'CounterUpdate':
        return `${data.count} ${data.counter_type} counter(s)`;
      case 'PowerToughnessUpdate':
        return `now ${data.power}/${data.toughness}`;
      case 'PhaseChange':
        return `${data.phase}`;
      case 'TurnChange':
        return `Turn ${data.turn}`;
      case 'PassPriority':
        return `passed priority`;
      default:
        return type;
    }
  };

  const getActionColor = (type: string): string => {
    const colorMap: Record<string, string> = {
      DrawCard: 'text-blue-400',
      PlayLand: 'text-green-400',
      CastSpell: 'text-purple-400',
      ActivateAbility: 'text-yellow-400',
      Attack: 'text-red-400',
      Block: 'text-orange-400',
      Resolve: 'text-cyan-400',
      LifeChange: 'text-pink-400',
      ZoneTransition: 'text-indigo-400',
      TapPermanent: 'text-slate-400',
      UntapPermanent: 'text-slate-300',
      DamageMarked: 'text-red-500',
      SummoningSickness: 'text-slate-500',
      PhaseChange: 'text-slate-500',
      TurnChange: 'text-white font-semibold',
      CounterUpdate: 'text-amber-400',
      PowerToughnessUpdate: 'text-teal-400',
    };
    return colorMap[type] || 'text-slate-400';
  };

  return (
    <div className={`bg-slate-900/80 backdrop-blur border border-slate-700 rounded-lg flex flex-col h-full ${className}`}>
      <div className="p-4 border-b border-slate-700">
        <h2 className="text-lg font-semibold text-white mb-3">Game Log</h2>

        <div className="mb-3">
          <input
            type="text"
            placeholder="Search actions..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full px-3 py-2 bg-slate-800 border border-slate-600 rounded-lg text-white placeholder-slate-500 focus:outline-none focus:border-slate-400"
          />
        </div>

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

      <div className="flex-1 overflow-y-auto p-4 space-y-2">
        {filteredActions.length === 0 ? (
          <div className="text-center text-slate-500 italic py-8">
            No actions to display
          </div>
        ) : (
          filteredActions.map((action) => {
            const isCurrentStep = currentStep !== undefined && action.index === currentStep - 1;
            const playerForAction = (action.data.player as string) ?? action.entry.active_player;

            return (
              <div
                key={action.index}
                ref={isCurrentStep ? currentRef : undefined}
                onClick={() => onActionClick?.(action.index + 1)}
                className={`
                  p-3 rounded-lg border transition-all cursor-pointer
                  ${isCurrentStep ? 'border-blue-500 bg-blue-500/10' : 'border-slate-700 hover:border-slate-500 hover:bg-slate-800/50'}
                `}
              >
                <div className="flex items-start gap-2">
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-xs text-slate-500">#{action.index + 1}</span>
                      {action.entry.turn > 0 && (
                        <span className="text-xs text-slate-500">
                          T{action.entry.turn}
                        </span>
                      )}
                      {action.entry.phase && (
                        <span className="text-xs text-slate-400">
                          {action.entry.phase}
                        </span>
                      )}
                    </div>
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-slate-300">
                        {getPlayerName(playerForAction)}
                      </span>
                      <span className={`text-sm ${getActionColor(action.type)}`}>
                        {getActionDescription(action.type, action.data)}
                      </span>
                    </div>
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>

      {filteredActions.length > 0 && maxEntries && (
        <div className="p-3 border-t border-slate-700 text-xs text-slate-500">
          Showing {filteredActions.length} of {actions.length} actions
        </div>
      )}
    </div>
  );
}
