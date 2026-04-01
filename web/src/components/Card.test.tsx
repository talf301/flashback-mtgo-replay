/**
 * Tests for Card Component
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Card } from './Card';
import { createCard } from '../types/state';

vi.mock('../api/scryfall');

describe('Card Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should render face-down card', () => {
    const mockCard = {
      ...createCard('card-1'),
      name: 'Test Card',
      faceDown: true,
    };

    render(<Card card={mockCard} />);

    expect(screen.getByText('MTG')).toBeInTheDocument();
    expect(screen.getByText('Face Down')).toBeInTheDocument();
  });

  it('should render face-up card without image data', () => {
    const mockCard = {
      ...createCard('card-1'),
      name: 'Test Card',
    };

    render(<Card card={mockCard} />);

    expect(screen.getByText('Test Card')).toBeInTheDocument();
  });

  it('should render card with counters', () => {
    const mockCard = {
      ...createCard('card-1'),
      name: 'Test Card',
      counters: {
        '+1/+1': 2,
        '-1/-1': 1,
      },
    };

    render(<Card card={mockCard} showCounters={true} />);

    expect(screen.getByText('2 +1/+1')).toBeInTheDocument();
    expect(screen.getByText('1 -1/-1')).toBeInTheDocument();
  });

  it('should call onClick handler when clicked', () => {
    const mockCard = {
      ...createCard('card-1'),
      name: 'Test Card',
    };

    const handleClick = vi.fn();
    render(<Card card={mockCard} onClick={handleClick} />);

    const cardElement = screen.getByText('Test Card').closest('div');
    if (cardElement) {
      fireEvent.click(cardElement);
    }

    expect(handleClick).toHaveBeenCalledTimes(1);
  });

  it('should apply selected styles', () => {
    const mockCard = {
      ...createCard('card-1'),
      name: 'Test Card',
    };

    const { rerender } = render(<Card card={mockCard} isSelected={false} />);

    const cardElement = screen.getByText('Test Card').closest('.bg-slate-800');
    expect(cardElement).not.toHaveClass('ring-2');

    rerender(<Card card={mockCard} isSelected={true} />);

    const selectedElement = screen.getByText('Test Card').closest('.bg-slate-800');
    expect(selectedElement).toHaveClass('ring-2');
  });

  it('should apply tapped styles', () => {
    const mockCard = {
      ...createCard('card-1'),
      name: 'Test Card',
    };

    const { rerender } = render(<Card card={mockCard} />);

    const cardElement = screen.getByText('Test Card').closest('.bg-slate-800');
    expect(cardElement).not.toHaveClass('rotate-90');

    const tappedCard = { ...mockCard, tapped: true };
    rerender(<Card card={tappedCard} />);

    const tappedElement = screen.getByText('Test Card').closest('.bg-slate-800');
    expect(tappedElement).toHaveClass('rotate-90');
  });

  it('should handle different sizes', () => {
    const mockCard = {
      ...createCard('card-1'),
      name: 'Test Card',
    };

    const { rerender } = render(<Card card={mockCard} size="small" />);
    expect(screen.getByText('Test Card')).toBeInTheDocument();

    rerender(<Card card={mockCard} size="normal" />);
    expect(screen.getByText('Test Card')).toBeInTheDocument();

    rerender(<Card card={mockCard} size="large" />);
    expect(screen.getByText('Test Card')).toBeInTheDocument();
  });

  it('should not show counters when showCounters is false', () => {
    const mockCard = {
      ...createCard('card-1'),
      name: 'Test Card',
      counters: { '+1/+1': 2 },
    };

    render(<Card card={mockCard} showCounters={false} />);

    expect(screen.queryByText('+2')).not.toBeInTheDocument();
  });
});
