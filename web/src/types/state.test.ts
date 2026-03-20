import { describe, it, expect } from 'vitest';
import { createEmptyBoardState } from './state';

describe('Board State Types', () => {
  it('should create empty board state', () => {
    const state = createEmptyBoardState();
    expect(state.zones).toHaveLength(0);
    expect(state.lifeTotals).toEqual({});
    expect(state.turn).toBe(0);
    expect(state.phase).toBe('');
  });
});
