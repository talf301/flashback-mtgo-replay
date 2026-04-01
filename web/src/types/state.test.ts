import { describe, it, expect } from 'vitest';
import {
  createEmptyBoardState,
  createEmptyManaPool,
  createCard,
} from './state';

describe('Board State Types', () => {
  it('should create empty board state', () => {
    const state = createEmptyBoardState();
    expect(state.zones).toHaveLength(0);
    expect(state.players).toEqual([]);
    expect(state.turn).toBe(0);
    expect(state.phase).toBe('');
    expect(state.stack).toEqual([]);
    expect(state.activePlayer).toBeUndefined();
    expect(state.priorityPlayer).toBeUndefined();
  });

  it('should create empty mana pool with all colors at zero', () => {
    const pool = createEmptyManaPool();
    expect(pool).toEqual({ W: 0, U: 0, B: 0, R: 0, G: 0, C: 0 });
  });

  it('should create a card with default values', () => {
    const card = createCard('card-1', 'Alice');
    expect(card.id).toBe('card-1');
    expect(card.owner).toBe('Alice');
    expect(card.tapped).toBe(false);
    expect(card.flipped).toBe(false);
    expect(card.faceDown).toBe(false);
    expect(card.summoningSick).toBe(false);
    expect(card.damage).toBe(0);
    expect(card.counters).toEqual({});
    expect(card.attachments).toEqual([]);
    expect(card.combatStatus).toEqual({ attacking: false, blocking: false });
  });

  it('should create a card without owner', () => {
    const card = createCard('card-2');
    expect(card.id).toBe('card-2');
    expect(card.owner).toBeUndefined();
  });

  it('should produce independent card instances', () => {
    const card1 = createCard('a');
    const card2 = createCard('b');
    card1.counters['test'] = 1;
    card1.attachments.push('x');
    expect(card2.counters).toEqual({});
    expect(card2.attachments).toEqual([]);
  });
});
