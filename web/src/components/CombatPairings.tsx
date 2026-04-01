/**
 * CombatPairings Display Component
 *
 * Shows attacker/blocker pairings during combat phases.
 * Groups attackers with their blockers based on combatStatus.blockTarget.
 */

import type { CardState } from '../types/state';

export interface CombatPairingsProps {
  cards: CardState[];
  playerNames: Record<string, string>;
}

interface CombatGroup {
  attacker: CardState;
  blockers: CardState[];
}

export function CombatPairings({ cards, playerNames }: CombatPairingsProps) {
  const attackers = cards.filter((c) => c.combatStatus.attacking);
  const blockers = cards.filter((c) => c.combatStatus.blocking);

  // Build combat groups: each attacker with its blockers
  const groups: CombatGroup[] = attackers.map((attacker) => ({
    attacker,
    blockers: blockers.filter((b) => b.combatStatus.blockTarget === attacker.id),
  }));

  // Blockers not assigned to any known attacker
  const assignedBlockerIds = new Set(groups.flatMap((g) => g.blockers.map((b) => b.id)));
  const unassignedBlockers = blockers.filter((b) => !assignedBlockerIds.has(b.id));

  if (groups.length === 0 && unassignedBlockers.length === 0) return null;

  return (
    <div className="mb-4 border-2 border-red-900/50 bg-red-950/20 rounded-lg p-3">
      <h3 className="text-sm font-semibold text-red-300 mb-3 flex items-center gap-2">
        <span className="w-2 h-2 bg-red-500 rounded-full animate-pulse" />
        Combat
      </h3>

      <div className="space-y-3">
        {groups.map((group) => (
          <div
            key={group.attacker.id}
            className="flex items-center gap-3 bg-slate-800/60 rounded-lg p-2"
          >
            {/* Attacker */}
            <div className="flex-shrink-0">
              <div className="border-2 border-red-500 rounded-lg px-3 py-1.5 bg-red-500/10">
                <div className="text-xs text-red-400 font-semibold">ATK</div>
                <div className="text-sm text-white font-bold truncate max-w-[120px]">
                  {group.attacker.name || `#${group.attacker.id}`}
                </div>
                {group.attacker.power !== undefined && group.attacker.toughness !== undefined && (
                  <div className="text-xs text-slate-300">
                    {group.attacker.power}/{group.attacker.toughness}
                  </div>
                )}
                {group.attacker.controller && (
                  <div className="text-xs text-slate-500">
                    {playerNames[group.attacker.controller] || group.attacker.controller}
                  </div>
                )}
              </div>
            </div>

            {/* Arrow */}
            <div className="text-slate-500 flex-shrink-0">
              {group.blockers.length > 0 ? (
                <span className="text-orange-400 font-bold">vs</span>
              ) : (
                <span className="text-red-400 italic text-xs">unblocked</span>
              )}
            </div>

            {/* Blockers */}
            {group.blockers.length > 0 && (
              <div className="flex gap-2 flex-wrap">
                {group.blockers.map((blocker) => (
                  <div
                    key={blocker.id}
                    className="border-2 border-orange-500 rounded-lg px-3 py-1.5 bg-orange-500/10"
                  >
                    <div className="text-xs text-orange-400 font-semibold">BLK</div>
                    <div className="text-sm text-white font-bold truncate max-w-[120px]">
                      {blocker.name || `#${blocker.id}`}
                    </div>
                    {blocker.power !== undefined && blocker.toughness !== undefined && (
                      <div className="text-xs text-slate-300">
                        {blocker.power}/{blocker.toughness}
                      </div>
                    )}
                    {blocker.controller && (
                      <div className="text-xs text-slate-500">
                        {playerNames[blocker.controller] || blocker.controller}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}

        {/* Unassigned blockers */}
        {unassignedBlockers.length > 0 && (
          <div className="flex items-center gap-3 bg-slate-800/60 rounded-lg p-2">
            <div className="text-xs text-orange-400 font-semibold mr-2">Blocking:</div>
            <div className="flex gap-2 flex-wrap">
              {unassignedBlockers.map((blocker) => (
                <div
                  key={blocker.id}
                  className="border-2 border-orange-500 rounded-lg px-3 py-1.5 bg-orange-500/10"
                >
                  <div className="text-xs text-orange-400 font-semibold">BLK</div>
                  <div className="text-sm text-white font-bold truncate max-w-[120px]">
                    {blocker.name || `#${blocker.id}`}
                  </div>
                  {blocker.power !== undefined && blocker.toughness !== undefined && (
                    <div className="text-xs text-slate-300">
                      {blocker.power}/{blocker.toughness}
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
