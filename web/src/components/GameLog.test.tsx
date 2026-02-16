/**
 * Tests for GameLog Component
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { GameLog } from './GameLog';
import type { ReplayAction } from '../types/replay';

describe('GameLog Component', () => {
  const mockActions: ReplayAction[] = [
    {
      timestamp: '2024-01-01T10:00:00Z',
      turn: 1,
      phase: 'beginning',
      active_player: 'player-1',
      action_type: { type: 'DrawCard', card_id: 'card-1' },
    },
    {
      timestamp: '2024-01-01T10:00:05Z',
      turn: 1,
      phase: 'main1',
      active_player: 'player-1',
      action_type: { type: 'PlayLand', card_id: 'card-2' },
    },
    {
      timestamp: '2024-01-01T10:00:10Z',
      turn: 1,
      phase: 'main1',
      active_player: 'player-1',
      action_type: { type: 'CastSpell', card_id: 'card-3', targets: [] },
    },
    {
      timestamp: '2024-01-01T10:00:15Z',
      turn: 1,
      phase: 'combat',
      active_player: 'player-1',
      action_type: { type: 'Attack', attacker_id: 'card-2', defender_id: 'player-2' },
    },
    {
      timestamp: '2024-01-01T10:00:20Z',
      turn: 1,
      phase: 'end',
      active_player: 'player-1',
      action_type: { type: 'PassPriority' },
    },
  ];

  const mockPlayerNames = {
    'player-1': 'Alice',
    'player-2': 'Bob',
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should render game log with all actions', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
      />,
    );

    expect(screen.getByText('Game Log')).toBeInTheDocument();
    expect(screen.getByText('Alice drew a card')).toBeInTheDocument();
    expect(screen.getByText('Alice played a land')).toBeInTheDocument();
    expect(screen.getByText('Alice cast a spell')).toBeInTheDocument();
    expect(screen.getByText('Alice attacked')).toBeInTheDocument();
    expect(screen.getByText('Alice passed priority')).toBeInTheDocument();
  });

  it('should highlight current step', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
        currentStep={2}
      />,
    );

    const currentAction = screen.getByText('Alice cast a spell').closest('.border-blue-500');
    expect(currentAction).toBeInTheDocument();
  });

  it('should call onActionClick when action clicked', () => {
    const handleClick = vi.fn();

    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
        onActionClick={handleClick}
      />,
    );

    const actionElement = screen.getByText('Alice drew a card').closest('[onClick]');
    if (actionElement) {
      fireEvent.click(actionElement);
    }

    expect(handleClick).toHaveBeenCalledWith(0);
  });

  it('should filter actions by search query', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
      />,
    );

    const searchInput = screen.getByPlaceholderText('Search actions...');
    fireEvent.change(searchInput, { target: { value: 'land' } });

    expect(screen.getByText('Alice played a land')).toBeInTheDocument();
    expect(screen.queryByText('Alice cast a spell')).not.toBeInTheDocument();
  });

  it('should filter actions by type', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
      />,
    );

    const filterSelect = screen.getByLabelText('Filter:');
    fireEvent.change(filterSelect, { target: { value: 'DrawCard' } });

    expect(screen.getByText('Alice drew a card')).toBeInTheDocument();
    expect(screen.queryByText('Alice played a land')).not.toBeInTheDocument();
  });

  it('should display timestamps when enabled', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
        showTimestamp={true}
      />,
    );

    expect(screen.getByText('Turn 1')).toBeInTheDocument();
  });

  it('should hide timestamps when disabled', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
        showTimestamp={false}
      />,
    );

    // Timestamps are formatted, so we check if time format elements are not present
    const timeElements = screen.getAllByText(/\d{2}:\d{2}:\d{2}/);
    expect(timeElements.length).toBe(0);
  });

  it('should limit entries with maxEntries prop', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
        maxEntries={3}
      />,
    );

    expect(screen.getByText('Showing 3 of 5 actions')).toBeInTheDocument();
  });

  it('should show empty state when no actions', () => {
    render(
      <GameLog
        actions={[]}
        playerNameMap={mockPlayerNames}
      />,
    );

    expect(screen.getByText('No actions to display')).toBeInTheDocument();
  });

  it('should show empty state when search returns no results', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
      />,
    );

    const searchInput = screen.getByPlaceholderText('Search actions...');
    fireEvent.change(searchInput, { target: { value: 'nonexistent' } });

    expect(screen.getByText('No actions to display')).toBeInTheDocument();
  });

  it('should use playerId when playerName not found', () => {
    const actionsWithoutNames: ReplayAction[] = [
      {
        timestamp: '2024-01-01T10:00:00Z',
        turn: 1,
        phase: 'beginning',
        active_player: 'unknown-player',
        action_type: { type: 'DrawCard', card_id: 'card-1' },
      },
    ];

    render(
      <GameLog
        actions={actionsWithoutNames}
        playerNameMap={mockPlayerNames}
      />,
    );

    expect(screen.getByText('unknown-player drew a card')).toBeInTheDocument();
  });

  it('should apply custom className', () => {
    const { container } = render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
        className="custom-class"
      />,
    );

    expect(container.firstChild).toHaveClass('custom-class');
  });

  it('should handle actions without active_player', () => {
    const actionsWithoutPlayer: ReplayAction[] = [
      {
        timestamp: '2024-01-01T10:00:00Z',
        turn: 1,
        phase: 'beginning',
        action_type: { type: 'GameEnd', winner: 'player-1' },
      },
    ];

    render(
      <GameLog
        actions={actionsWithoutPlayer}
        playerNameMap={mockPlayerNames}
      />,
    );

    expect(screen.getByText(/game ended/i)).toBeInTheDocument();
  });

  it('should format phase names correctly', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
        showPhase={true}
      />,
    );

    expect(screen.getByText('Beginning')).toBeInTheDocument();
    expect(screen.getByText('Main 1')).toBeInTheDocument();
    expect(screen.getByText('Combat')).toBeInTheDocument();
    expect(screen.getByText('End')).toBeInTheDocument();
  });

  it('should handle different action types with appropriate colors', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
      />,
    );

    const drawAction = screen.getByText('drew a card');
    expect(drawAction).toHaveClass('text-blue-400');

    const landAction = screen.getByText('played a land');
    expect(landAction).toHaveClass('text-green-400');

    const attackAction = screen.getByText('attacked');
    expect(attackAction).toHaveClass('text-red-400');
  });

  it('should display action step numbers', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
      />,
    );

    expect(screen.getByText('#1')).toBeInTheDocument();
    expect(screen.getByText('#2')).toBeInTheDocument();
    expect(screen.getByText('#3')).toBeInTheDocument();
  });
});
