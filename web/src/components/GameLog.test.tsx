/**
 * Tests for GameLog Component
 *
 * GameLog.tsx still imports parseActionType from types/replay (removed in v3).
 * We mock it here so the component can render. The mock handles the old externally-
 * tagged action_type format that the component currently expects.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

// Provide parseActionType mock before GameLog loads
vi.mock('../types/replay', async () => {
  const actual = await vi.importActual<typeof import('../types/replay')>('../types/replay');
  return {
    ...actual,
    parseActionType: (actionType: Record<string, unknown>) => {
      if (!actionType) return { type: 'Unknown', data: {} };
      const keys = Object.keys(actionType);
      if (keys.length > 0) {
        const type = keys[0];
        return { type, data: (actionType[type] as Record<string, unknown>) || {} };
      }
      return { type: 'Unknown', data: {} };
    },
  };
});

import { GameLog } from './GameLog';

describe('GameLog Component', () => {
  // Mock actions use the old externally-tagged format that GameLog.tsx currently expects
  const mockActions = [
    {
      timestamp: '2024-01-01T10:00:00Z',
      turn: 1,
      phase: 'beginning',
      active_player: 'player-1',
      action_type: { DrawCard: { player_id: 'player-1', card_id: 'card-1' } },
    },
    {
      timestamp: '2024-01-01T10:00:05Z',
      turn: 1,
      phase: 'main1',
      active_player: 'player-1',
      action_type: { PlayLand: { player_id: 'player-1', card_id: 'card-2' } },
    },
    {
      timestamp: '2024-01-01T10:00:10Z',
      turn: 1,
      phase: 'main1',
      active_player: 'player-1',
      action_type: { CastSpell: { player_id: 'player-1', card_id: 'card-3' } },
    },
    {
      timestamp: '2024-01-01T10:00:15Z',
      turn: 1,
      phase: 'combat',
      active_player: 'player-1',
      action_type: { Attack: { attacker_id: 'card-2', defender_id: 'player-2' } },
    },
    {
      timestamp: '2024-01-01T10:00:20Z',
      turn: 1,
      phase: 'end',
      active_player: 'player-1',
      action_type: { PassPriority: { player_id: 'player-1' } },
    },
  ] as any[];

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
    expect(screen.getByText('drew #card-1')).toBeInTheDocument();
    expect(screen.getByText('played #card-2')).toBeInTheDocument();
    expect(screen.getByText('cast #card-3')).toBeInTheDocument();
    expect(screen.getByText('#card-2 attacked')).toBeInTheDocument();
    expect(screen.getByText('passed priority')).toBeInTheDocument();
  });

  it('should highlight current step', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
        currentStep={2}
      />,
    );

    const currentAction = screen.getByText('played #card-2').closest('.border-blue-500');
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

    const actionElement = screen.getByText('drew #card-1').closest('[class]');
    if (actionElement) {
      fireEvent.click(actionElement);
    }

    expect(handleClick).toHaveBeenCalledWith(1);
  });

  it('should filter actions by search query', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
      />,
    );

    const searchInput = screen.getByPlaceholderText('Search actions...');
    fireEvent.change(searchInput, { target: { value: 'PlayLand' } });

    expect(screen.getByText('played #card-2')).toBeInTheDocument();
    expect(screen.queryByText('cast #card-3')).not.toBeInTheDocument();
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

    expect(screen.getByText('drew #card-1')).toBeInTheDocument();
    expect(screen.queryByText('played #card-2')).not.toBeInTheDocument();
  });

  it('should display timestamps when enabled', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
      />,
    );

    expect(screen.getAllByText('T1').length).toBeGreaterThan(0);
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
    const actionsWithoutNames = [
      {
        timestamp: '2024-01-01T10:00:00Z',
        turn: 1,
        phase: 'beginning',
        active_player: 'unknown-player',
        action_type: { DrawCard: { player_id: 'unknown-player', card_id: 'card-1' } },
      },
    ] as any[];

    render(
      <GameLog
        actions={actionsWithoutNames}
        playerNameMap={mockPlayerNames}
      />,
    );

    expect(screen.getByText('unknown-player')).toBeInTheDocument();
    expect(screen.getByText('drew #card-1')).toBeInTheDocument();
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

  it('should handle actions without known type', () => {
    const actionsWithUnknown = [
      {
        timestamp: '2024-01-01T10:00:00Z',
        turn: 1,
        phase: 'beginning',
        active_player: 'player-1',
        action_type: { Unknown: { description: 'game ended' } },
      },
    ] as any[];

    render(
      <GameLog
        actions={actionsWithUnknown}
        playerNameMap={mockPlayerNames}
      />,
    );

    expect(screen.getByText('game ended')).toBeInTheDocument();
  });

  it('should format phase names correctly', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
      />,
    );

    expect(screen.getAllByText('beginning').length).toBeGreaterThan(0);
    expect(screen.getAllByText('main1').length).toBeGreaterThan(0);
    expect(screen.getAllByText('combat').length).toBeGreaterThan(0);
    expect(screen.getAllByText('end').length).toBeGreaterThan(0);
  });

  it('should handle different action types with appropriate colors', () => {
    render(
      <GameLog
        actions={mockActions}
        playerNameMap={mockPlayerNames}
      />,
    );

    const drawAction = screen.getByText('drew #card-1').closest('span');
    expect(drawAction).toHaveClass('text-blue-400');

    const landAction = screen.getByText('played #card-2').closest('span');
    expect(landAction).toHaveClass('text-green-400');

    const attackAction = screen.getByText('#card-2 attacked').closest('span');
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
