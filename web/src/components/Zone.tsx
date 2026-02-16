/**
 * Zone Display Component
 *
 * Displays a collection of cards in a specific zone (battlefield, hand, graveyard, exile).
 * Supports different layout modes and card interaction.
 */

import { useState } from 'react';
import type { Zone as ZoneType } from '../types/state';
import { Card } from './Card';

export interface ZoneProps {
  zone: ZoneType;
  layout?: 'grid' | 'row' | 'stack';
  cardSize?: 'small' | 'normal' | 'large';
  selectable?: boolean;
  onCardClick?: (cardId: string) => void;
  emptyMessage?: string;
  showOwner?: boolean;
  maxCards?: number;
}

export function Zone({
  zone,
  layout = 'grid',
  cardSize = 'normal',
  selectable = false,
  onCardClick,
  emptyMessage = 'Empty zone',
  showOwner = false,
  maxCards,
}: ZoneProps) {
  const [selectedCardId, setSelectedCardId] = useState<string | null>(null);

  const handleCardClick = (cardId: string) => {
    if (selectable) {
      setSelectedCardId(cardId === selectedCardId ? null : cardId);
    }
    onCardClick?.(cardId);
  };

  const displayCards = maxCards ? zone.cards.slice(0, maxCards) : zone.cards;
  const hasMoreCards = maxCards && zone.cards.length > maxCards;

  const layoutClasses = {
    grid: 'grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-2',
    row: 'flex flex-wrap gap-2',
    stack: 'flex flex-col gap-1',
  };

  const getZoneTitle = () => {
    let title = zone.name.charAt(0).toUpperCase() + zone.name.slice(1);
    if (showOwner && zone.owner) {
      title += ` (${zone.owner})`;
    }
    return title;
  };

  const getZoneColor = () => {
    switch (zone.name) {
      case 'battlefield':
        return 'border-emerald-900/50 bg-emerald-950/30';
      case 'hand':
        return 'border-blue-900/50 bg-blue-950/30';
      case 'graveyard':
        return 'border-slate-800 bg-slate-900/50';
      case 'exile':
        return 'border-yellow-900/50 bg-yellow-950/30';
      default:
        return 'border-slate-700 bg-slate-800/30';
    }
  };

  const renderEmptyState = () => (
    <div className="flex items-center justify-center h-32 text-slate-500 italic">
      {emptyMessage}
    </div>
  );

  const renderStackLayout = () => (
    <div className="flex gap-2">
      {displayCards.map((card, index) => (
        <div key={card.id} style={{ transform: `translateX(${index * 8}px)` }}>
          <Card
            card={card}
            size={cardSize}
            isSelected={selectedCardId === card.id}
            onClick={() => handleCardClick(card.id)}
          />
        </div>
      ))}
      {hasMoreCards && (
        <div className="flex items-center justify-center w-12 h-44 bg-slate-700 rounded-lg text-slate-400 font-bold">
          +{zone.cards.length - maxCards}
        </div>
      )}
    </div>
  );

  return (
    <div className={`border-2 rounded-lg p-4 ${getZoneColor()}`}>
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-lg font-semibold text-slate-200">{getZoneTitle()}</h3>
        {zone.cards.length > 0 && (
          <span className="text-sm text-slate-400 bg-slate-700 px-2 py-0.5 rounded-full">
            {zone.cards.length}
          </span>
        )}
      </div>

      {displayCards.length === 0 ? (
        renderEmptyState()
      ) : (
        <div className={layoutClasses[layout]}>
          {layout === 'stack' ? (
            renderStackLayout()
          ) : (
            displayCards.map((card) => (
              <Card
                key={card.id}
                card={card}
                size={cardSize}
                isSelected={selectedCardId === card.id}
                onClick={() => handleCardClick(card.id)}
              />
            ))
          )}
          {hasMoreCards && (
            <div
              className={`${
                cardSize === 'small'
                  ? 'w-16 h-22'
                  : cardSize === 'large'
                    ? 'w-48 h-66'
                    : 'w-32 h-44'
              } bg-slate-700 rounded-lg flex items-center justify-center text-slate-400 font-bold text-xl`}
            >
              +{zone.cards.length - maxCards}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
