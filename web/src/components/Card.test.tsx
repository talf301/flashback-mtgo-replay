/**
 * Tests for Card Component
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Card } from './Card';
// Note: Scryfall loading is tested in scryfall.test.ts

vi.mock('../api/scryfall');

describe('Card Component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should render face-down card', () => {
    const mockCard = {
      id: 'card-1',
      name: 'Test Card',
      is_face_down: true,
      counters: [],
    };

    render(<Card card={mockCard} isFaceUp={false} />);

    expect(screen.getByText('MTG')).toBeInTheDocument();
    expect(screen.getByText('Card')).toBeInTheDocument();
  });

  it('should render face-up card without image data', () => {
    const mockCard = {
      id: 'card-1',
      name: 'Test Card',
      is_face_down: false,
      counters: [],
    };

    render(<Card card={mockCard} isFaceUp={true} />);

    expect(screen.getByText('Test Card')).toBeInTheDocument();
    expect(screen.getByText('ID: card-1')).toBeInTheDocument();
  });

  it('should render card with counters', () => {
    const mockCard = {
      id: 'card-1',
      name: 'Test Card',
      is_face_down: false,
      counters: [
        { type: '+1/+1', amount: 2 },
        { type: '-1/-1', amount: 1 },
      ],
    };

    render(<Card card={mockCard} isFaceUp={true} showCounters={true} />);

    expect(screen.getByText('+2')).toBeInTheDocument();
    expect(screen.getByText('-1')).toBeInTheDocument();
  });

  it('should call onClick handler when clicked', () => {
    const mockCard = {
      id: 'card-1',
      name: 'Test Card',
      is_face_down: false,
      counters: [],
    };

    const handleClick = vi.fn();
    render(<Card card={mockCard} isFaceUp={true} onClick={handleClick} />);

    const cardElement = screen.getByText('Test Card').closest('div');
    if (cardElement) {
      fireEvent.click(cardElement);
    }

    expect(handleClick).toHaveBeenCalledTimes(1);
  });

  it('should apply selected styles', () => {
    const mockCard = {
      id: 'card-1',
      name: 'Test Card',
      is_face_down: false,
      counters: [],
    };

    const { rerender } = render(<Card card={mockCard} isFaceUp={true} isSelected={false} />);

    const cardElement = screen.getByText('Test Card').closest('div');
    expect(cardElement).not.toHaveClass('ring-2');

    rerender(<Card card={mockCard} isFaceUp={true} isSelected={true} />);

    const selectedElement = screen.getByText('Test Card').closest('div');
    expect(selectedElement).toHaveClass('ring-2');
  });

  it('should apply tapped styles', () => {
    const mockCard = {
      id: 'card-1',
      name: 'Test Card',
      is_face_down: false,
      counters: [],
    };

    const { rerender } = render(<Card card={mockCard} isFaceUp={true} isTapped={false} />);

    const cardElement = screen.getByText('Test Card').closest('div');
    expect(cardElement).not.toHaveClass('rotate-90');

    rerender(<Card card={mockCard} isFaceUp={true} isTapped={true} />);

    const tappedElement = screen.getByText('Test Card').closest('div');
    expect(tappedElement).toHaveClass('rotate-90');
  });

  it('should handle different sizes', () => {
    const mockCard = {
      id: 'card-1',
      name: 'Test Card',
      is_face_down: false,
      counters: [],
    };

    const { rerender } = render(<Card card={mockCard} isFaceUp={true} size="small" />);
    expect(screen.getByText('Test Card')).toBeInTheDocument();

    rerender(<Card card={mockCard} isFaceUp={true} size="normal" />);
    expect(screen.getByText('Test Card')).toBeInTheDocument();

    rerender(<Card card={mockCard} isFaceUp={true} size="large" />);
    expect(screen.getByText('Test Card')).toBeInTheDocument();
  });

  it('should not show counters when showCounters is false', () => {
    const mockCard = {
      id: 'card-1',
      name: 'Test Card',
      is_face_down: false,
      counters: [{ type: '+1/+1', amount: 2 }],
    };

    render(<Card card={mockCard} isFaceUp={true} showCounters={false} />);

    expect(screen.queryByText('+2')).not.toBeInTheDocument();
  });
});
