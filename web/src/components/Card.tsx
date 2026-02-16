/**
 * Card Display Component
 *
 * Renders a single Magic card with optional Scryfall data.
 * Shows card image, name, type, and counters.
 */

import { useState } from 'react';
import type { CardRef } from '../types/state';
import { getCardData } from '../api/scryfall';

export interface CardProps {
  card: CardRef;
  isFaceUp?: boolean;
  isSelected?: boolean;
  isTapped?: boolean;
  onClick?: () => void;
  size?: 'small' | 'normal' | 'large';
  showCounters?: boolean;
}

export function Card({
  card,
  isFaceUp = true,
  isSelected = false,
  isTapped = false,
  onClick,
  size = 'normal',
  showCounters = true,
}: CardProps) {
  const [scryfallData, setScryfallData] = useState<any>(null);
  const [imageError, setImageError] = useState(false);
  const [isLoading, setIsLoading] = useState(false);

  // Load Scryfall data when card face-up and has scryfall_id
  useState(() => {
    if (isFaceUp && card.scryfall_id && !scryfallData && !isLoading) {
      setIsLoading(true);
      getCardData(card.scryfall_id)
        .then(setScryfallData)
        .catch(() => {
          // Silently fail - card will render without image
          setImageError(true);
        })
        .finally(() => setIsLoading(false));
    }
  });

  const sizeClasses = {
    small: 'w-16 h-22',
    normal: 'w-32 h-44',
    large: 'w-48 h-66',
  };

  const imageSize = {
    small: 64,
    normal: 128,
    large: 192,
  };

  const isSelectedClass = isSelected ? 'ring-2 ring-blue-500 ring-offset-2' : '';
  const isTappedClass = isTapped ? 'rotate-90 opacity-80' : '';

  const renderCardBack = () => (
    <div className={`${sizeClasses[size]} ${isSelectedClass} bg-gradient-to-br from-blue-900 to-purple-900 rounded-lg border-2 border-slate-600 flex items-center justify-center cursor-pointer hover:border-slate-400 transition-colors`}>
      <div className="text-center">
        <div className="text-white text-2xl font-bold mb-1">MTG</div>
        <div className="text-slate-400 text-xs">Card</div>
      </div>
    </div>
  );

  const renderCardFace = () => {
    if (!scryfallData || imageError) {
      return (
        <div
          className={`${sizeClasses[size]} ${isSelectedClass} bg-slate-800 rounded-lg border-2 border-slate-600 p-2 cursor-pointer hover:border-slate-400 transition-colors ${isTappedClass}`}
          onClick={onClick}
        >
          <div className="h-full flex flex-col justify-between">
            <div>
              <div className="font-bold text-white text-xs mb-1 truncate">{card.name || 'Unknown Card'}</div>
              <div className="text-slate-400 text-xs truncate">ID: {card.id}</div>
            </div>

            {showCounters && card.counters.length > 0 && (
              <div className="flex gap-1 flex-wrap">
                {card.counters.map((counter, idx) => (
                  <div
                    key={idx}
                    className="bg-red-600 text-white text-xs px-1 rounded"
                    title={`${counter.type}: ${counter.amount}`}
                  >
                    {counter.amount > 0 ? '+' : ''}
                    {counter.amount}
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      );
    }

    const imageUrl =
      size === 'small'
        ? scryfallData.image_uris.small
        : size === 'large'
          ? scryfallData.image_uris.large
          : scryfallData.image_uris.normal;

    return (
      <div
        className={`${sizeClasses[size]} ${isSelectedClass} cursor-pointer hover:scale-105 transition-transform ${isTappedClass}`}
        onClick={onClick}
      >
        <img
          src={imageUrl}
          alt={card.name || 'Card'}
          className="w-full h-full object-cover rounded-lg shadow-lg"
          onError={() => setImageError(true)}
          loading="lazy"
          width={imageSize[size]}
          height={Math.floor(imageSize[size] * 1.375)}
        />

        {showCounters && card.counters.length > 0 && (
          <div className="absolute bottom-0 left-0 right-0 flex gap-1 flex-wrap p-1 bg-black/50 rounded-b-lg">
            {card.counters.map((counter, idx) => (
              <div
                key={idx}
                className="bg-red-600 text-white text-xs px-2 py-0.5 rounded-full font-bold"
                title={`${counter.type}: ${counter.amount}`}
              >
                {counter.amount > 0 ? '+' : ''}
                {counter.amount}
              </div>
            ))}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="relative">
      {card.is_face_down || !isFaceUp ? renderCardBack() : renderCardFace()}

      {isLoading && (
        <div className={`${sizeClasses[size]} absolute inset-0 bg-slate-900/80 rounded-lg flex items-center justify-center`}>
          <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-white"></div>
        </div>
      )}
    </div>
  );
}
