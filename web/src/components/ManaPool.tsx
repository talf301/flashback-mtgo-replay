/**
 * ManaPool Display Component
 *
 * Renders a player's current mana pool as colored symbols.
 * Only shows mana types that have a non-zero amount.
 */

import type { ManaPool as ManaPoolType } from '../types/state';

export interface ManaPoolProps {
  manaPool: ManaPoolType;
  compact?: boolean;
}

const MANA_COLORS: Record<keyof ManaPoolType, { label: string; bg: string; text: string }> = {
  W: { label: 'W', bg: 'bg-yellow-100', text: 'text-yellow-900' },
  U: { label: 'U', bg: 'bg-blue-400', text: 'text-blue-950' },
  B: { label: 'B', bg: 'bg-gray-800', text: 'text-gray-100' },
  R: { label: 'R', bg: 'bg-red-500', text: 'text-red-100' },
  G: { label: 'G', bg: 'bg-green-500', text: 'text-green-100' },
  C: { label: 'C', bg: 'bg-slate-400', text: 'text-slate-900' },
};

export function ManaPool({ manaPool, compact = false }: ManaPoolProps) {
  const entries = (Object.keys(MANA_COLORS) as (keyof ManaPoolType)[]).filter(
    (key) => manaPool[key] > 0,
  );

  if (entries.length === 0) return null;

  const total = entries.reduce((sum, key) => sum + manaPool[key], 0);

  return (
    <div className={`flex items-center gap-1 ${compact ? '' : 'mt-1'}`}>
      {!compact && (
        <span className="text-xs text-slate-500 mr-1">Mana ({total}):</span>
      )}
      {entries.map((key) => {
        const { label, bg, text } = MANA_COLORS[key];
        return (
          <div
            key={key}
            className={`${bg} ${text} text-xs font-bold rounded-full w-5 h-5 flex items-center justify-center`}
            title={`${label}: ${manaPool[key]}`}
          >
            {manaPool[key]}
          </div>
        );
      })}
    </div>
  );
}
