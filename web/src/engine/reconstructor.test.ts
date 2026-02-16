import { describe, it, expect } from 'vitest';
import { reconstructState, createEmptyBoardState } from './reconstructor';
import { ReplayAction } from '../types/replay';

describe('State Reconstruction Engine', () => {
  it('should reconstruct board state from actions', () => {
    const actions: ReplayAction[] = [
      {
        timestamp: '2024-01-01T00:00:00Z',
        action_type: { type: 'DrawCard', card_id: 'card1' },
      },
      {
        timestamp: '2024-01-01T00:00:01Z',
        action_type: { type: 'PlayLand', card_id: 'land1' },
      },
    ];

    const state = reconstructState(actions);
    expect(state.zones.find(z => z.name === 'hand')?.cards).toHaveLength(1);
    expect(state.zones.find(z => z.name === 'battlefield')?.cards).toHaveLength(1);
  });

  it('should handle zone changes', () => {
    const actions: ReplayAction[] = [
      {
        timestamp: '2024-01-01T00:00:00Z',
        action_type: { type: 'PlayLand', card_id: 'land1' },
      },
      {
        timestamp: '2024-01-01T00:00:01Z',
        action_type: { type: 'ZoneChange', card_id: 'land1', from: 'battlefield', to: 'graveyard' },
      },
    ];

    const state = reconstructState(actions);
    expect(state.zones.find(z => z.name === 'battlefield')?.cards).toHaveLength(0);
    expect(state.zones.find(z => z.name === 'graveyard')?.cards).toHaveLength(1);
  });

  it('should track life totals', () => {
    const actions: ReplayAction[] = [
      {
        timestamp: '2024-01-01T00:00:00Z',
        action_type: { type: 'LifeChange', player_id: 'player1', old: 20, new: 18 },
      },
    ];

    const state = reconstructState(actions);
    expect(state.lifeTotals['player1']).toBe(18);
  });

  it('should handle counter additions', () => {
    const actions: ReplayAction[] = [
      {
        timestamp: '2024-01-01T00:00:00Z',
        action_type: { type: 'PlayLand', card_id: 'creature1' },
      },
      {
        timestamp: '2024-01-01T00:00:01Z',
        action_type: { type: 'CounterAdd', card_id: 'creature1', counter_type: '+1/+1', amount: 1 },
      },
    ];

    const state = reconstructState(actions);
    const card = state.zones.find(z => z.name === 'battlefield')?.cards[0];
    expect(card?.counters).toHaveLength(1);
    expect(card?.counters[0].type).toBe('+1/+1');
  });
});
