/**
 * Tests for Zone Component
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Zone } from './Zone';
import type { Zone as ZoneType } from '../types/state';

describe('Zone Component', () => {
  const mockZone: ZoneType = {
    name: 'battlefield',
    owner: 'player-1',
    cards: [
      { id: 'card-1', name: 'Card One', counters: [] },
      { id: 'card-2', name: 'Card Two', counters: [] },
    ],
  };

  const mockZoneWithCounters: ZoneType = {
    name: 'battlefield',
    owner: 'player-1',
    cards: [
      {
        id: 'card-1',
        name: 'Card One',
        counters: [{ type: '+1/+1', amount: 3 }],
      },
    ],
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should render zone with cards', () => {
    render(<Zone zone={mockZone} />);

    expect(screen.getByText('Battlefield (player-1)')).toBeInTheDocument();
    expect(screen.getByText('2')).toBeInTheDocument();
    expect(screen.getByText('Card One')).toBeInTheDocument();
    expect(screen.getByText('Card Two')).toBeInTheDocument();
  });

  it('should render empty zone', () => {
    const emptyZone: ZoneType = {
      name: 'graveyard',
      owner: 'player-1',
      cards: [],
    };

    render(<Zone zone={emptyZone} emptyMessage="No cards in graveyard" />);

    expect(screen.getByText('Graveyard (player-1)')).toBeInTheDocument();
    expect(screen.getByText('No cards in graveyard')).toBeInTheDocument();
  });

  it('should render zone without owner when showOwner is false', () => {
    render(<Zone zone={mockZone} showOwner={false} />);

    expect(screen.getByText('Battlefield')).toBeInTheDocument();
    expect(screen.queryByText('(player-1)')).not.toBeInTheDocument();
  });

  it('should call onCardClick when card is clicked', () => {
    const handleClick = vi.fn();
    render(<Zone zone={mockZone} selectable onCardClick={handleClick} />);

    const cardElement = screen.getByText('Card One').closest('div');
    if (cardElement) {
      fireEvent.click(cardElement);
    }

    expect(handleClick).toHaveBeenCalledWith('card-1');
  });

  it('should toggle card selection when selectable', () => {
    render(<Zone zone={mockZone} selectable />);

    const cardElement = screen.getByText('Card One').closest('div');
    if (cardElement) {
      fireEvent.click(cardElement);
    }

    // Card should now have selected class
    const selectedElement = screen.getByText('Card One').closest('div');
    expect(selectedElement).toHaveClass('ring-2');

    // Click again to deselect
    if (cardElement) {
      fireEvent.click(cardElement);
    }

    const deselectedElement = screen.getByText('Card One').closest('div');
    expect(deselectedElement).not.toHaveClass('ring-2');
  });

  it('should render different layouts', () => {
    const { rerender } = render(<Zone zone={mockZone} layout="grid" />);
    expect(screen.getByText('Card One')).toBeInTheDocument();

    rerender(<Zone zone={mockZone} layout="row" />);
    expect(screen.getByText('Card One')).toBeInTheDocument();

    rerender(<Zone zone={mockZone} layout="stack" />);
    expect(screen.getByText('Card One')).toBeInTheDocument();
  });

  it('should apply correct styling for different zone types', () => {
    const { rerender } = render(<Zone zone={mockZone} />);

    let zoneElement = screen.getByText('Battlefield').closest('.border-2');
    expect(zoneElement).toHaveClass('border-emerald-900/50');

    const handZone: ZoneType = { ...mockZone, name: 'hand' };
    rerender(<Zone zone={handZone} />);

    zoneElement = screen.getByText('Hand').closest('.border-2');
    expect(zoneElement).toHaveClass('border-blue-900/50');

    const graveyardZone: ZoneType = { ...mockZone, name: 'graveyard' };
    rerender(<Zone zone={graveyardZone} />);

    zoneElement = screen.getByText('Graveyard').closest('.border-2');
    expect(zoneElement).toHaveClass('border-slate-800');

    const exileZone: ZoneType = { ...mockZone, name: 'exile' };
    rerender(<Zone zone={exileZone} />);

    zoneElement = screen.getByText('Exile').closest('.border-2');
    expect(zoneElement).toHaveClass('border-yellow-900/50');
  });

  it('should limit cards with maxCards prop', () => {
    const largeZone: ZoneType = {
      name: 'battlefield',
      owner: 'player-1',
      cards: Array.from({ length: 10 }, (_, i) => ({
        id: `card-${i}`,
        name: `Card ${i}`,
        counters: [],
      })),
    };

    render(<Zone zone={largeZone} maxCards={5} />);

    expect(screen.getByText('+5')).toBeInTheDocument();
    expect(screen.queryByText('Card 6')).not.toBeInTheDocument();
  });

  it('should render cards with different sizes', () => {
    const { rerender } = render(<Zone zone={mockZone} cardSize="small" />);
    expect(screen.getByText('Card One')).toBeInTheDocument();

    rerender(<Zone zone={mockZone} cardSize="normal" />);
    expect(screen.getByText('Card One')).toBeInTheDocument();

    rerender(<Zone zone={mockZone} cardSize="large" />);
    expect(screen.getByText('Card One')).toBeInTheDocument();
  });

  it('should capitalize zone name', () => {
    render(<Zone zone={mockZone} showOwner={false} />);

    expect(screen.getByText('Battlefield')).toBeInTheDocument();
  });
});
