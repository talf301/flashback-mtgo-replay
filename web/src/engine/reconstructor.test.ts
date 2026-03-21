import { describe, it, expect } from 'vitest';
import { Reconstructor } from './reconstructor';
import type { RawReplayAction, ReplayFile } from '../types/replay';

function makeReplay(actions: RawReplayAction[]): ReplayFile {
  return {
    metadata: {},
    header: {
      format: 'Standard',
      start_time: '2024-01-01T00:00:00Z',
      players: [
        { player_id: 'player1', name: 'Player 1', life_total: 20 },
      ],
    },
    games: [
      {
        game_number: 1,
        header: {
          game_id: 'test',
          players: [
            { player_id: 'player1', name: 'Player 1', life_total: 20 },
          ],
          result: 'Incomplete',
        },
        actions,
      },
    ],
  };
}

describe('State Reconstruction Engine', () => {
  it('should reconstruct board state from actions', () => {
    const actions: RawReplayAction[] = [
      {
        timestamp: '2024-01-01T00:00:00Z',
        turn: 1,
        phase: 'beginning',
        active_player: 'player1',
        action_type: { DrawCard: { player_id: 'player1', card_id: 'card1' } },
      },
      {
        timestamp: '2024-01-01T00:00:01Z',
        turn: 1,
        phase: 'beginning',
        active_player: 'player1',
        action_type: { DrawCard: { player_id: 'player1', card_id: 'land1' } },
      },
      {
        timestamp: '2024-01-01T00:00:02Z',
        turn: 1,
        phase: 'main1',
        active_player: 'player1',
        action_type: { PlayLand: { player_id: 'player1', card_id: 'land1' } },
      },
    ];
    const rec = new Reconstructor();
    rec.loadReplay(makeReplay(actions));
    const state = rec.reconstruct(3);
    expect(state.zones.find(z => z.name === 'hand')?.cards).toHaveLength(1);
    expect(state.zones.find(z => z.name === 'battlefield')?.cards).toHaveLength(1);
  });

  it('should handle zone changes', () => {
    const actions: RawReplayAction[] = [
      {
        timestamp: '2024-01-01T00:00:00Z',
        turn: 1,
        phase: 'beginning',
        active_player: 'player1',
        action_type: { DrawCard: { player_id: 'player1', card_id: 'land1' } },
      },
      {
        timestamp: '2024-01-01T00:00:01Z',
        turn: 1,
        phase: 'main1',
        active_player: 'player1',
        action_type: { PlayLand: { player_id: 'player1', card_id: 'land1' } },
      },
      {
        timestamp: '2024-01-01T00:00:02Z',
        turn: 1,
        phase: 'main1',
        active_player: 'player1',
        action_type: { ZoneTransition: { card_id: 'land1', from_zone: 'battlefield', to_zone: 'graveyard', player_id: 'player1' } },
      },
    ];
    const rec = new Reconstructor();
    rec.loadReplay(makeReplay(actions));
    const state = rec.reconstruct(3);
    expect(state.zones.find(z => z.name === 'battlefield' && z.owner === 'player1')?.cards).toHaveLength(0);
    expect(state.zones.find(z => z.name === 'graveyard')?.cards).toHaveLength(1);
  });

  it('should track life totals', () => {
    const actions: RawReplayAction[] = [
      {
        timestamp: '2024-01-01T00:00:00Z',
        turn: 1,
        phase: 'main1',
        active_player: 'player1',
        action_type: { LifeChange: { player_id: 'player1', old_life: 20, new_life: 18 } },
      },
    ];

    const rec = new Reconstructor();
    rec.loadReplay(makeReplay(actions));
    const state = rec.reconstruct(1);
    expect(state.lifeTotals['player1']).toBe(18);
  });

  it('should handle counter additions', () => {
    const actions: RawReplayAction[] = [
      {
        timestamp: '2024-01-01T00:00:00Z',
        turn: 1,
        phase: 'beginning',
        active_player: 'player1',
        action_type: { DrawCard: { player_id: 'player1', card_id: 'land1' } },
      },
      {
        timestamp: '2024-01-01T00:00:01Z',
        turn: 1,
        phase: 'main1',
        active_player: 'player1',
        action_type: { PlayLand: { player_id: 'player1', card_id: 'land1' } },
      },
      {
        timestamp: '2024-01-01T00:00:02Z',
        turn: 1,
        phase: 'main1',
        active_player: 'player1',
        action_type: { CounterUpdate: { card_id: 'land1', counter_type: '+1/+1', count: 1 } },
      },
    ];
    const rec = new Reconstructor();
    rec.loadReplay(makeReplay(actions));
    const state = rec.reconstruct(3);
    const card = state.zones.find(z => z.name === 'battlefield')?.cards[0];
    expect(card?.counters).toHaveLength(1);
    expect(card?.counters[0].type).toBe('+1/+1');
  });
});
