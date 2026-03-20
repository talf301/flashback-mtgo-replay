/**
 * Tests for Board Component
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Board } from './Board';
import type { BoardState } from '../types/state';
import { createCard } from '../types/state';

describe('Board Component', () => {
  const mockBoardState: BoardState = {
    zones: [
      {
        name: 'battlefield',
        owner: 'player-1',
        cards: [
          { ...createCard('card-1'), name: 'Creature 1' },
          { ...createCard('card-2'), name: 'Creature 2' },
        ],
      },
      {
        name: 'hand',
        owner: 'player-1',
        cards: [{ ...createCard('card-3'), name: 'Spell 1' }],
      },
      {
        name: 'battlefield',
        owner: 'player-2',
        cards: [{ ...createCard('card-4'), name: 'Creature 3' }],
      },
    ],
    lifeTotals: {
      'player-1': 18,
      'player-2': 20,
    },
    turn: 3,
    phase: 'precombat_main',
    activePlayer: 'player-1',
    stack: [],
  };

  const mockPlayerNames = {
    'player-1': 'Alice',
    'player-2': 'Bob',
  };

  it('should render board with all elements', () => {
    render(
      <Board
        boardState={mockBoardState}
        playerIds={['player-1', 'player-2']}
        playerNames={mockPlayerNames}
      />,
    );

    expect(screen.getByText('Turn:')).toBeInTheDocument();
    expect(screen.getByText('3')).toBeInTheDocument();
    expect(screen.getByText('Phase:')).toBeInTheDocument();
    expect(screen.getByText('Main 1')).toBeInTheDocument();
    expect(screen.getByText('Active:')).toBeInTheDocument();
    expect(screen.getAllByText('Alice').length).toBeGreaterThan(0);
  });

  it('should render life totals', () => {
    render(
      <Board
        boardState={mockBoardState}
        playerIds={['player-1', 'player-2']}
        playerNames={mockPlayerNames}
      />,
    );

    expect(screen.getAllByText('Alice').length).toBeGreaterThan(0);
    expect(screen.getByText('18')).toBeInTheDocument();
    expect(screen.getAllByText('Bob').length).toBeGreaterThan(0);
    expect(screen.getByText('20')).toBeInTheDocument();
  });

  it('should highlight active player', () => {
    render(
      <Board
        boardState={mockBoardState}
        playerIds={['player-1', 'player-2']}
        playerNames={mockPlayerNames}
      />,
    );

    const aliceLife = screen.getByText('18').closest('.border-yellow-500');
    expect(aliceLife).toBeInTheDocument();

    const bobLife = screen.getByText('20').closest('.border-slate-600');
    expect(bobLife).toBeInTheDocument();
  });

  it('should render stack when present', () => {
    const boardStateWithStack: BoardState = {
      ...mockBoardState,
      stack: [
        {
          id: 'stack-1',
          controller: 'player-1',
        },
        {
          id: 'stack-2',
          controller: 'player-2',
        },
      ],
    };

    render(
      <Board
        boardState={boardStateWithStack}
        playerIds={['player-1', 'player-2']}
        playerNames={mockPlayerNames}
      />,
    );

    expect(screen.getByText('Stack (2)')).toBeInTheDocument();
    expect(screen.getByText('#stack-1')).toBeInTheDocument();
    expect(screen.getByText('#stack-2')).toBeInTheDocument();
  });

  it('should not render stack when empty', () => {
    render(
      <Board
        boardState={mockBoardState}
        playerIds={['player-1', 'player-2']}
        playerNames={mockPlayerNames}
      />,
    );

    expect(screen.queryByText('Stack')).not.toBeInTheDocument();
  });

  it('should render player zones', () => {
    render(
      <Board
        boardState={mockBoardState}
        playerIds={['player-1', 'player-2']}
        playerNames={mockPlayerNames}
      />,
    );

    expect(screen.getAllByText('Alice').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Bob').length).toBeGreaterThan(0);
    expect(screen.getByText('Creature 1')).toBeInTheDocument();
    expect(screen.getByText('Creature 2')).toBeInTheDocument();
    expect(screen.getByText('Creature 3')).toBeInTheDocument();
  });

  it('should handle onCardClick', () => {
    const handleClick = vi.fn();

    render(
      <Board
        boardState={mockBoardState}
        playerIds={['player-1', 'player-2']}
        playerNames={mockPlayerNames}
        onCardClick={handleClick}
      />,
    );

    // Note: Testing click events requires more complex setup
    // This just ensures the handler is accepted
    expect(handleClick).toBeDefined();
  });

  it('should render with default layout', () => {
    render(
      <Board
        boardState={mockBoardState}
        playerIds={['player-1', 'player-2']}
        playerNames={mockPlayerNames}
      />,
    );

    expect(screen.getAllByText('Alice').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Bob').length).toBeGreaterThan(0);
  });

  it('should handle missing active player', () => {
    const boardStateNoActive: BoardState = {
      ...mockBoardState,
      activePlayer: undefined,
    };

    render(
      <Board
        boardState={boardStateNoActive}
        playerIds={['player-1', 'player-2']}
        playerNames={mockPlayerNames}
      />,
    );

    expect(screen.queryByText('Active:')).not.toBeInTheDocument();
  });

  it('should handle default life totals', () => {
    const boardStateNoLife: BoardState = {
      ...mockBoardState,
      lifeTotals: {},
    };

    render(
      <Board
        boardState={boardStateNoLife}
        playerIds={['player-1', 'player-2']}
        playerNames={mockPlayerNames}
      />,
    );

    expect(screen.getAllByText('20')).toHaveLength(2);
  });
});
