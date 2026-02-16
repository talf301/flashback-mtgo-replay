import { BoardState, createEmptyBoardState } from '../types/state';
import { ReplayAction, ActionType } from '../types/replay';

export function reconstructState(actions: ReplayAction[]): BoardState {
  let state = createEmptyBoardState();

  for (const action of actions) {
    state = applyAction(state, action);
  }

  return state;
}

function applyAction(state: BoardState, action: ReplayAction): BoardState {
  const newState = { ...state, zones: state.zones.map(z => ({ ...z, cards: [...z.cards] })) };

  switch (action.action_type.type) {
    case 'DrawCard':
      return applyDrawCard(newState, action.action_type);
    case 'PlayLand':
      return applyPlayLand(newState, action.action_type);
    case 'CastSpell':
      return applyCastSpell(newState, action);
    case 'LifeChange':
      return applyLifeChange(newState, action.action_type);
    case 'ZoneChange':
      return applyZoneChange(newState, action.action_type);
    case 'CounterAdd':
      return applyCounterAdd(newState, action.action_type);
    case 'TokenCreate':
      return applyTokenCreate(newState, action.action_type);
    default:
      return newState;
  }
}

function applyDrawCard(state: BoardState, action: ActionType & { type: 'DrawCard' }): BoardState {
  const handZone = state.zones.find(z => z.name === 'hand');
  if (!handZone) return state;

  return {
    ...state,
    zones: state.zones.map(z =>
      z.name === 'hand'
        ? { ...z, cards: [...z.cards, { id: action.card_id, counters: [] }] }
        : z
    ),
  };
}

function applyPlayLand(state: BoardState, action: ActionType & { type: 'PlayLand' }): BoardState {
  // Remove from hand, add to battlefield
  const handZone = state.zones.find(z => z.name === 'hand');
  if (!handZone) return state;

  const cardInHand = handZone.cards.find(c => c.id === action.card_id);
  if (!cardInHand) return state;

  const battlefieldZone = state.zones.find(z => z.name === 'battlefield');
  if (!battlefieldZone) return state;

  return {
    ...state,
    zones: state.zones.map(z => {
      if (z.name === 'hand') {
        return {
          ...z,
          cards: z.cards.filter(c => c.id !== action.card_id),
        };
      }
      if (z.name === 'battlefield') {
        return {
          ...z,
          cards: [...z.cards, { id: action.card_id, counters: [] }],
        };
      }
      return z;
    }),
  };
}

function applyCastSpell(state: BoardState, action: ReplayAction): BoardState {
  // Remove from hand, add to stack
  const handZone = state.zones.find(z => z.name === 'hand');
  if (!handZone) return state;

  const spellAction = action.action_type as { type: 'CastSpell'; card_id: string; targets: string[] };
  if (spellAction.type !== 'CastSpell') return state;

  return {
    ...state,
    zones: state.zones.map(z =>
      z.name === 'hand'
        ? { ...z, cards: z.cards.filter(c => c.id !== spellAction.card_id) }
        : z
    ),
    stack: [
      ...state.stack,
      {
        id: `stack-${spellAction.card_id}-${Date.now()}`,
        card_id: spellAction.card_id,
        controller: action.active_player || 'unknown',
        targets: spellAction.targets,
      },
    ],
  };
}

function applyLifeChange(state: BoardState, action: ActionType & { type: 'LifeChange' }): BoardState {
  return {
    ...state,
    lifeTotals: {
      ...state.lifeTotals,
      [action.player_id]: action.new,
    },
  };
}

function applyZoneChange(state: BoardState, action: ActionType & { type: 'ZoneChange' }): BoardState {
  const fromZone = state.zones.find(z => z.name === action.from);
  const toZone = state.zones.find(z => z.name === action.to);

  if (!fromZone || !toZone) return state;

  const card = fromZone.cards.find(c => c.id === action.card_id);
  if (!card) return state;

  return {
    ...state,
    zones: state.zones.map(z => {
      if (z.name === action.from) {
        return { ...z, cards: z.cards.filter(c => c.id !== action.card_id) };
      }
      if (z.name === action.to) {
        return { ...z, cards: [...z.cards, card] };
      }
      return z;
    }),
  };
}

function applyCounterAdd(state: BoardState, action: ActionType & { type: 'CounterAdd' }): BoardState {
  return {
    ...state,
    zones: state.zones.map(zone => ({
      ...zone,
      cards: zone.cards.map(card =>
        card.id === action.card_id
          ? {
              ...card,
              counters: [
                ...(card.counters || []).filter(c => c.type !== action.counter_type),
                { type: action.counter_type, amount: action.amount },
              ],
            }
          : card
      ),
    })),
  };
}

function applyTokenCreate(state: BoardState, action: ActionType & { type: 'TokenCreate' }): BoardState {
  const battlefieldZone = state.zones.find(z => z.name === 'battlefield');
  if (!battlefieldZone) return state;

  return {
    ...state,
    zones: state.zones.map(z =>
      z.name === 'battlefield'
        ? {
            ...z,
            cards: [
              ...z.cards,
              {
                id: action.token_id,
                scryfall_id: action.card_id,
                counters: [],
              },
            ],
          }
        : z
    ),
  };
}

export { createEmptyBoardState } from '../types/state';
