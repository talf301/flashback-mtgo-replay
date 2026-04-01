import { describe, it, expect } from 'vitest';
import { Reconstructor } from './reconstructor';
import type {
  ReplayFile,
  TimelineEntry,
  SnapshotEntry,
  EventEntry,
  CardCatalog,
} from '../types/replay';

const catalog: CardCatalog = {
  'cat-mountain': { name: 'Mountain', mana_cost: '', type_line: 'Basic Land - Mountain' },
  'cat-bolt': { name: 'Lightning Bolt', mana_cost: '{R}', type_line: 'Instant' },
  'cat-bear': { name: 'Grizzly Bears', mana_cost: '{1}{G}', type_line: 'Creature - Bear' },
};

function makeSnapshot(
  turn: number,
  phase: string,
  overrides: Partial<SnapshotEntry['state']> = {},
): SnapshotEntry {
  return {
    type: 'snapshot',
    turn,
    phase,
    active_player: 'Alice',
    state: {
      players: [
        {
          name: 'Alice',
          seat: 1,
          life: 20,
          mana_pool: {},
          zones: {
            hand: { cards: [] },
            battlefield: { cards: [] },
            library: { cards: [], count: 40 },
            graveyard: { cards: [] },
          },
        },
        {
          name: 'Bob',
          seat: 2,
          life: 20,
          mana_pool: {},
          zones: {
            hand: { cards: [] },
            battlefield: { cards: [] },
            library: { cards: [], count: 40 },
            graveyard: { cards: [] },
          },
        },
      ],
      active_player: 'Alice',
      priority_player: 'Alice',
      ...overrides,
    },
  };
}

function makeEvent(
  turn: number,
  phase: string,
  event: EventEntry['event'],
  active_player = 'Alice',
): EventEntry {
  return { type: 'event', turn, phase, active_player, event };
}

function makeReplay(timeline: TimelineEntry[], cardCatalog: CardCatalog = catalog): ReplayFile {
  return {
    version: '3',
    header: {
      game_id: 1,
      players: [
        { name: 'Alice', seat: 1 },
        { name: 'Bob', seat: 2 },
      ],
      format: 'Standard',
      start_time: '2024-01-01T00:00:00Z',
      result: { winner: '', reason: '' },
      complete: true,
      decklist: { mainboard: [], sideboard: [] },
      sideboard_changes: null,
    },
    timeline,
    card_catalog: cardCatalog,
  };
}

describe('State Reconstruction Engine (v3)', () => {
  describe('snapshot loading', () => {
    it('should reconstruct board state from a snapshot', () => {
      const snap = makeSnapshot(1, 'precombat_main', {
        players: [
          {
            name: 'Alice',
            seat: 1,
            life: 17,
            mana_pool: { R: 1 },
            zones: {
              hand: { cards: [{ id: 'inst-1', catalog_id: 'cat-bolt' }] },
              battlefield: { cards: [{ id: 'inst-2', catalog_id: 'cat-mountain' }] },
              library: { cards: [], count: 38 },
              graveyard: { cards: [] },
            },
          },
          {
            name: 'Bob',
            seat: 2,
            life: 20,
            mana_pool: {},
            zones: {
              hand: { cards: [] },
              battlefield: { cards: [] },
              library: { cards: [], count: 40 },
              graveyard: { cards: [] },
            },
          },
        ],
        active_player: 'Alice',
        priority_player: 'Alice',
      });

      const rec = new Reconstructor();
      rec.loadReplay(makeReplay([snap]));
      const state = rec.reconstruct(1);

      expect(state.turn).toBe(1);
      expect(state.phase).toBe('precombat_main');
      expect(state.activePlayer).toBe('Alice');
      expect(state.players).toHaveLength(2);
      expect(state.players[0].life).toBe(17);
      expect(state.players[0].manaPool.R).toBe(1);

      const hand = state.zones.find(z => z.name === 'hand' && z.owner === 'Alice');
      expect(hand?.cards).toHaveLength(1);
      expect(hand?.cards[0].name).toBe('Lightning Bolt');

      const bf = state.zones.find(z => z.name === 'battlefield' && z.owner === 'Alice');
      expect(bf?.cards).toHaveLength(1);
      expect(bf?.cards[0].name).toBe('Mountain');
    });

    it('should return empty board state at position 0', () => {
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay([makeSnapshot(1, 'beginning')]));
      const state = rec.reconstruct(0);
      expect(state.zones).toHaveLength(0);
      expect(state.players).toHaveLength(0);
      expect(state.turn).toBe(0);
    });
  });

  describe('event application', () => {
    it('should apply DrawCard event', () => {
      const timeline: TimelineEntry[] = [
        makeSnapshot(1, 'beginning'),
        makeEvent(1, 'beginning', { type: 'DrawCard', player: 'Alice', card_id: 'cat-bolt' }),
      ];
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay(timeline));
      const state = rec.reconstruct(2);

      const hand = state.zones.find(z => z.name === 'hand' && z.owner === 'Alice');
      expect(hand?.cards).toHaveLength(1);
      expect(hand?.cards[0].name).toBe('Lightning Bolt');
    });

    it('should apply PlayLand event', () => {
      const timeline: TimelineEntry[] = [
        makeSnapshot(1, 'beginning'),
        makeEvent(1, 'beginning', { type: 'DrawCard', player: 'Alice', card_id: 'cat-mountain' }),
        makeEvent(1, 'precombat_main', { type: 'PlayLand', player: 'Alice', card_id: 'cat-mountain' }),
      ];
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay(timeline));
      const state = rec.reconstruct(3);

      const hand = state.zones.find(z => z.name === 'hand' && z.owner === 'Alice');
      expect(hand?.cards ?? []).toHaveLength(0);
      const bf = state.zones.find(z => z.name === 'battlefield' && z.owner === 'Alice');
      expect(bf?.cards).toHaveLength(1);
      expect(bf?.cards[0].name).toBe('Mountain');
    });

    it('should apply LifeChange event', () => {
      const timeline: TimelineEntry[] = [
        makeSnapshot(1, 'precombat_main'),
        makeEvent(1, 'precombat_main', {
          type: 'LifeChange',
          player: 'Alice',
          old_life: 20,
          new_life: 17,
        }),
      ];
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay(timeline));
      const state = rec.reconstruct(2);
      expect(state.players.find(p => p.name === 'Alice')?.life).toBe(17);
    });

    it('should apply ZoneTransition event', () => {
      const snap = makeSnapshot(1, 'precombat_main', {
        players: [
          {
            name: 'Alice',
            seat: 1,
            life: 20,
            mana_pool: {},
            zones: {
              hand: { cards: [] },
              battlefield: { cards: [{ id: 'inst-1', catalog_id: 'cat-mountain' }] },
              library: { cards: [], count: 38 },
              graveyard: { cards: [] },
            },
          },
          {
            name: 'Bob',
            seat: 2,
            life: 20,
            mana_pool: {},
            zones: {
              hand: { cards: [] },
              battlefield: { cards: [] },
              library: { cards: [], count: 40 },
              graveyard: { cards: [] },
            },
          },
        ],
        active_player: 'Alice',
        priority_player: 'Alice',
      });

      const timeline: TimelineEntry[] = [
        snap,
        makeEvent(1, 'precombat_main', {
          type: 'ZoneTransition',
          card_id: 'inst-1',
          from_zone: 'battlefield',
          to_zone: 'graveyard',
          player: 'Alice',
        }),
      ];
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay(timeline));
      const state = rec.reconstruct(2);

      const bf = state.zones.find(z => z.name === 'battlefield' && z.owner === 'Alice');
      expect(bf?.cards).toHaveLength(0);
      const gy = state.zones.find(z => z.name === 'graveyard' && z.owner === 'Alice');
      expect(gy?.cards).toHaveLength(1);
    });

    it('should apply CounterUpdate event', () => {
      const snap = makeSnapshot(1, 'precombat_main', {
        players: [
          {
            name: 'Alice',
            seat: 1,
            life: 20,
            mana_pool: {},
            zones: {
              hand: { cards: [] },
              battlefield: { cards: [{ id: 'inst-bear', catalog_id: 'cat-bear' }] },
              library: { cards: [], count: 38 },
              graveyard: { cards: [] },
            },
          },
          {
            name: 'Bob',
            seat: 2,
            life: 20,
            mana_pool: {},
            zones: {
              hand: { cards: [] },
              battlefield: { cards: [] },
              library: { cards: [], count: 40 },
              graveyard: { cards: [] },
            },
          },
        ],
        active_player: 'Alice',
        priority_player: 'Alice',
      });

      const timeline: TimelineEntry[] = [
        snap,
        makeEvent(1, 'precombat_main', {
          type: 'CounterUpdate',
          card_id: 'inst-bear',
          counter_type: '+1/+1',
          count: 2,
        }),
      ];
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay(timeline));
      const state = rec.reconstruct(2);

      const bf = state.zones.find(z => z.name === 'battlefield' && z.owner === 'Alice');
      const card = bf?.cards.find(c => c.id === 'inst-bear');
      expect(card?.counters['+1/+1']).toBe(2);
    });

    it('should apply Discard event (zone transition to graveyard)', () => {
      const timeline: TimelineEntry[] = [
        makeSnapshot(1, 'precombat_main'),
        makeEvent(1, 'beginning', { type: 'DrawCard', player: 'Alice', card_id: 'cat-bolt' }),
        makeEvent(1, 'end_of_turn', { type: 'Discard', player: 'Alice', card_id: 'cat-bolt' }),
      ];
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay(timeline));
      const state = rec.reconstruct(3);

      const hand = state.zones.find(z => z.name === 'hand' && z.owner === 'Alice');
      expect(hand?.cards ?? []).toHaveLength(0);
      const gy = state.zones.find(z => z.name === 'graveyard' && z.owner === 'Alice');
      expect(gy?.cards).toHaveLength(1);
    });

    it('should apply Mill event (library to graveyard)', () => {
      const timeline: TimelineEntry[] = [
        makeSnapshot(1, 'precombat_main'),
        makeEvent(1, 'precombat_main', { type: 'Mill', player: 'Alice', card_id: 'cat-bear' }),
      ];
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay(timeline));
      const state = rec.reconstruct(2);

      const gy = state.zones.find(z => z.name === 'graveyard' && z.owner === 'Alice');
      expect(gy?.cards).toHaveLength(1);
    });

    it('should apply CreateToken event', () => {
      const timeline: TimelineEntry[] = [
        makeSnapshot(1, 'precombat_main'),
        makeEvent(1, 'precombat_main', {
          type: 'CreateToken',
          player: 'Alice',
          card_id: 'token-1',
          token_name: 'Goblin Token',
        }),
      ];
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay(timeline));
      const state = rec.reconstruct(2);

      const bf = state.zones.find(z => z.name === 'battlefield' && z.owner === 'Alice');
      expect(bf?.cards).toHaveLength(1);
      expect(bf?.cards[0].name).toBe('Goblin Token');
    });

    it('should apply Attack event', () => {
      const snap = makeSnapshot(1, 'declare_attackers', {
        players: [
          {
            name: 'Alice',
            seat: 1,
            life: 20,
            mana_pool: {},
            zones: {
              hand: { cards: [] },
              battlefield: { cards: [{ id: 'inst-bear', catalog_id: 'cat-bear' }] },
              library: { cards: [], count: 38 },
              graveyard: { cards: [] },
            },
          },
          {
            name: 'Bob',
            seat: 2,
            life: 20,
            mana_pool: {},
            zones: {
              hand: { cards: [] },
              battlefield: { cards: [] },
              library: { cards: [], count: 40 },
              graveyard: { cards: [] },
            },
          },
        ],
        active_player: 'Alice',
        priority_player: 'Alice',
      });

      const timeline: TimelineEntry[] = [
        snap,
        makeEvent(1, 'declare_attackers', {
          type: 'Attack',
          attacker_id: 'inst-bear',
          target: 'Bob',
        }),
      ];
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay(timeline));
      const state = rec.reconstruct(2);

      const bf = state.zones.find(z => z.name === 'battlefield' && z.owner === 'Alice');
      const card = bf?.cards.find(c => c.id === 'inst-bear');
      expect(card?.combatStatus.attacking).toBe(true);
      expect(card?.tapped).toBe(true);
    });

    it('should apply TapPermanent and UntapPermanent events', () => {
      const snap = makeSnapshot(1, 'precombat_main', {
        players: [
          {
            name: 'Alice',
            seat: 1,
            life: 20,
            mana_pool: {},
            zones: {
              hand: { cards: [] },
              battlefield: { cards: [{ id: 'inst-1', catalog_id: 'cat-mountain' }] },
              library: { cards: [], count: 38 },
              graveyard: { cards: [] },
            },
          },
          {
            name: 'Bob',
            seat: 2,
            life: 20,
            mana_pool: {},
            zones: {
              hand: { cards: [] },
              battlefield: { cards: [] },
              library: { cards: [], count: 40 },
              graveyard: { cards: [] },
            },
          },
        ],
        active_player: 'Alice',
        priority_player: 'Alice',
      });

      const timeline: TimelineEntry[] = [
        snap,
        makeEvent(1, 'precombat_main', { type: 'TapPermanent', card_id: 'inst-1' }),
      ];
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay(timeline));

      let state = rec.reconstruct(2);
      let card = state.zones.find(z => z.name === 'battlefield')?.cards[0];
      expect(card?.tapped).toBe(true);

      // Add untap and reconstruct further
      timeline.push(makeEvent(2, 'untap', { type: 'UntapPermanent', card_id: 'inst-1' }));
      rec.loadReplay(makeReplay(timeline));
      state = rec.reconstruct(3);
      card = state.zones.find(z => z.name === 'battlefield')?.cards[0];
      expect(card?.tapped).toBe(false);
    });

    it('should apply CastSpell and Resolve events', () => {
      const timeline: TimelineEntry[] = [
        makeSnapshot(1, 'beginning'),
        makeEvent(1, 'beginning', { type: 'DrawCard', player: 'Alice', card_id: 'cat-bolt' }),
        makeEvent(1, 'precombat_main', {
          type: 'CastSpell',
          player: 'Alice',
          card_id: 'cat-bolt',
        }),
      ];
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay(timeline));
      let state = rec.reconstruct(3);
      expect(state.stack).toHaveLength(1);
      expect(state.stack[0].id).toBe('cat-bolt');

      timeline.push(
        makeEvent(1, 'precombat_main', { type: 'Resolve', card_id: 'cat-bolt' }),
      );
      rec.loadReplay(makeReplay(timeline));
      state = rec.reconstruct(4);
      expect(state.stack).toHaveLength(0);
    });
  });

  describe('seeking with snapshots', () => {
    it('should seek to nearest snapshot and replay forward', () => {
      const snap1 = makeSnapshot(1, 'beginning');
      const snap2 = makeSnapshot(3, 'beginning', {
        players: [
          {
            name: 'Alice',
            seat: 1,
            life: 15,
            mana_pool: {},
            zones: {
              hand: { cards: [] },
              battlefield: { cards: [] },
              library: { cards: [], count: 35 },
              graveyard: { cards: [] },
            },
          },
          {
            name: 'Bob',
            seat: 2,
            life: 18,
            mana_pool: {},
            zones: {
              hand: { cards: [] },
              battlefield: { cards: [] },
              library: { cards: [], count: 38 },
              graveyard: { cards: [] },
            },
          },
        ],
        active_player: 'Alice',
        priority_player: 'Alice',
      });

      const timeline: TimelineEntry[] = [
        snap1,
        makeEvent(1, 'precombat_main', { type: 'DrawCard', player: 'Alice', card_id: 'cat-bolt' }),
        makeEvent(2, 'precombat_main', {
          type: 'LifeChange',
          player: 'Bob',
          old_life: 20,
          new_life: 18,
        }),
        snap2, // index 3
        makeEvent(3, 'precombat_main', {
          type: 'LifeChange',
          player: 'Alice',
          old_life: 15,
          new_life: 12,
        }),
      ];

      const rec = new Reconstructor();
      rec.loadReplay(makeReplay(timeline));

      // Position 5: uses snap2 (index 3) then applies event at index 4
      const state = rec.reconstruct(5);
      expect(state.players.find(p => p.name === 'Alice')?.life).toBe(12);
      expect(state.turn).toBe(3);
    });

    it('should handle timeline with no snapshots (events only)', () => {
      const timeline: TimelineEntry[] = [
        makeEvent(1, 'beginning', { type: 'DrawCard', player: 'Alice', card_id: 'cat-bolt' }),
        makeEvent(1, 'beginning', { type: 'DrawCard', player: 'Alice', card_id: 'cat-mountain' }),
      ];
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay(timeline));
      const state = rec.reconstruct(2);
      const hand = state.zones.find(z => z.name === 'hand' && z.owner === 'Alice');
      expect(hand?.cards).toHaveLength(2);
    });
  });

  describe('card catalog', () => {
    it('should look up card names from catalog', () => {
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay([]));
      expect(rec.getCardName('cat-bolt')).toBe('Lightning Bolt');
      expect(rec.getCardName('nonexistent')).toBeUndefined();
    });

    it('should return all card names', () => {
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay([]));
      const names = rec.getCardNames();
      expect(names['cat-bolt']).toBe('Lightning Bolt');
      expect(names['cat-mountain']).toBe('Mountain');
      expect(Object.keys(names)).toHaveLength(3);
    });

    it('should return raw catalog', () => {
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay([]));
      const cat = rec.getCatalog();
      expect(cat['cat-bear'].type_line).toBe('Creature - Bear');
    });
  });

  describe('timeline length', () => {
    it('should report timeline length', () => {
      const timeline: TimelineEntry[] = [
        makeSnapshot(1, 'beginning'),
        makeEvent(1, 'beginning', { type: 'DrawCard', player: 'Alice', card_id: 'cat-bolt' }),
        makeEvent(1, 'precombat_main', { type: 'PlayLand', player: 'Alice', card_id: 'cat-mountain' }),
      ];
      const rec = new Reconstructor();
      rec.loadReplay(makeReplay(timeline));
      expect(rec.getTimelineLength()).toBe(3);
      // Deprecated alias
      expect(rec.getActionCount()).toBe(3);
    });
  });

  describe('card state details from snapshots', () => {
    it('should preserve card properties from snapshot', () => {
      const snap = makeSnapshot(1, 'precombat_main', {
        players: [
          {
            name: 'Alice',
            seat: 1,
            life: 20,
            mana_pool: {},
            zones: {
              hand: { cards: [] },
              battlefield: {
                cards: [
                  {
                    id: 'inst-bear',
                    catalog_id: 'cat-bear',
                    tapped: true,
                    damage: 1,
                    counters: { '+1/+1': 3 },
                    summoning_sickness: true,
                    controller: 'Bob',
                    power: 4,
                    toughness: 4,
                  },
                ],
              },
              library: { cards: [], count: 38 },
              graveyard: { cards: [] },
            },
          },
          {
            name: 'Bob',
            seat: 2,
            life: 20,
            mana_pool: {},
            zones: {
              hand: { cards: [] },
              battlefield: { cards: [] },
              library: { cards: [], count: 40 },
              graveyard: { cards: [] },
            },
          },
        ],
        active_player: 'Alice',
        priority_player: 'Alice',
      });

      const rec = new Reconstructor();
      rec.loadReplay(makeReplay([snap]));
      const state = rec.reconstruct(1);

      const bf = state.zones.find(z => z.name === 'battlefield' && z.owner === 'Alice');
      const card = bf?.cards[0];
      expect(card?.tapped).toBe(true);
      expect(card?.damage).toBe(1);
      expect(card?.counters['+1/+1']).toBe(3);
      expect(card?.summoningSick).toBe(true);
      expect(card?.controller).toBe('Bob');
      expect(card?.power).toBe(4);
      expect(card?.toughness).toBe(4);
      expect(card?.name).toBe('Grizzly Bears');
      expect(card?.catalogId).toBe('cat-bear');
    });

    it('should reset transient state when card leaves battlefield', () => {
      const snap = makeSnapshot(1, 'precombat_main', {
        players: [
          {
            name: 'Alice',
            seat: 1,
            life: 20,
            mana_pool: {},
            zones: {
              hand: { cards: [] },
              battlefield: {
                cards: [
                  {
                    id: 'inst-bear',
                    catalog_id: 'cat-bear',
                    tapped: true,
                    damage: 2,
                    counters: { '+1/+1': 1 },
                  },
                ],
              },
              library: { cards: [], count: 38 },
              graveyard: { cards: [] },
            },
          },
          {
            name: 'Bob',
            seat: 2,
            life: 20,
            mana_pool: {},
            zones: {
              hand: { cards: [] },
              battlefield: { cards: [] },
              library: { cards: [], count: 40 },
              graveyard: { cards: [] },
            },
          },
        ],
        active_player: 'Alice',
        priority_player: 'Alice',
      });

      const timeline: TimelineEntry[] = [
        snap,
        makeEvent(1, 'precombat_main', {
          type: 'ZoneTransition',
          card_id: 'inst-bear',
          from_zone: 'battlefield',
          to_zone: 'graveyard',
          player: 'Alice',
        }),
      ];

      const rec = new Reconstructor();
      rec.loadReplay(makeReplay(timeline));
      const state = rec.reconstruct(2);

      const gy = state.zones.find(z => z.name === 'graveyard' && z.owner === 'Alice');
      const card = gy?.cards[0];
      expect(card?.tapped).toBe(false);
      expect(card?.damage).toBe(0);
      expect(card?.counters).toEqual({});
    });
  });
});
