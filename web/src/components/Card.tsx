import type { CardState } from '../types/state';

export interface CardProps {
  card: CardState;
  isSelected?: boolean;
  onClick?: () => void;
  size?: 'small' | 'normal' | 'large';
  showCounters?: boolean;
}

export function Card({
  card,
  isSelected = false,
  onClick,
  size = 'normal',
  showCounters = true,
}: CardProps) {
  const sizeClasses = {
    small: 'w-16 h-22',
    normal: 'w-32 h-44',
    large: 'w-48 h-66',
  };

  const isSelectedClass = isSelected ? 'ring-2 ring-blue-500 ring-offset-2' : '';
  const tappedClass = card.tapped ? 'rotate-90 opacity-80' : '';
  const counterEntries = Object.entries(card.counters);

  if (card.faceDown) {
    return (
      <div
        className={`${sizeClasses[size]} ${isSelectedClass} ${tappedClass} bg-gradient-to-br from-blue-900 to-purple-900 rounded-lg border-2 border-slate-600 flex items-center justify-center cursor-pointer hover:border-slate-400 transition-colors`}
        onClick={onClick}
      >
        <div className="text-center">
          <div className="text-white text-2xl font-bold mb-1">MTG</div>
          <div className="text-slate-400 text-xs">Face Down</div>
        </div>
      </div>
    );
  }

  const { attacking, blocking } = card.combatStatus;

  return (
    <div className="relative">
      <div
        className={`${sizeClasses[size]} ${isSelectedClass} ${tappedClass} bg-slate-800 rounded-lg border-2 ${attacking ? 'border-red-500' : blocking ? 'border-orange-500' : 'border-slate-600'} p-2 cursor-pointer hover:border-slate-400 transition-colors`}
        onClick={onClick}
      >
        <div className="h-full flex flex-col justify-between">
          <div>
            <div className="font-bold text-white text-xs mb-1 truncate">
              {card.name || `#${card.id}`}
            </div>
            {card.power !== undefined && card.toughness !== undefined && (
              <div className="text-slate-300 text-xs">
                {card.power}/{card.toughness}
                {card.damage > 0 && (
                  <span className="text-red-400 ml-1">({card.damage} dmg)</span>
                )}
              </div>
            )}
          </div>

          <div className="space-y-1">
            {card.summoningSick && (
              <div className="text-yellow-500 text-xs">Summoning Sick</div>
            )}
            {card.attachments.length > 0 && (
              <div className="text-blue-400 text-xs truncate">
                {card.attachments.length} attachment{card.attachments.length !== 1 ? 's' : ''}
              </div>
            )}
            {showCounters && counterEntries.length > 0 && (
              <div className="flex gap-1 flex-wrap">
                {counterEntries.map(([type, amount]) => (
                  <div
                    key={type}
                    className="bg-amber-600 text-white text-xs px-1 rounded"
                    title={`${type}: ${amount}`}
                  >
                    {amount} {type}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>

      {attacking && (
        <div className="absolute -top-1 -right-1 bg-red-500 text-white text-xs px-1 rounded">ATK</div>
      )}
      {blocking && (
        <div className="absolute -top-1 -right-1 bg-orange-500 text-white text-xs px-1 rounded">BLK</div>
      )}
    </div>
  );
}
