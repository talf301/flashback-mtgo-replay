import { BoardState, createEmptyBoardState, CardState, createCard, Zone } from '../types/state';
import { RawReplayAction, ReplayFile, parseActionType } from '../types/replay';
import { resolveCardNamesByMtgoId } from '../api/scryfall';

/**
 * Reconstructs board state by applying actions sequentially.
 *
 * The Rust pipeline emits actions like DrawCard, PlayLand, ZoneTransition,
 * TapPermanent, etc. Each modifies the board state. We replay them to
 * reconstruct the board at any point.
 */
export class Reconstructor {
  private actions: RawReplayAction[] = [];
  private cardNames: Record<string, string> = {};

  loadReplay(replay: ReplayFile, gameIndex: number = 0): void {
    const game = replay.games[gameIndex];
    if (!game) {
      throw new Error(`Game index ${gameIndex} out of range (${replay.games.length} games)`);
    }
    this.actions = game.actions;
    this.cardNames = game.card_names ? { ...game.card_names } : {};
  }

  /**
   * Resolves unnamed cards using MTGO IDs from card_textures via Scryfall.
   * Merges resolved names into cardNames. Returns the number of newly resolved names.
   */
  async resolveCardTextures(cardTextures: Record<string, number>): Promise<number> {
    // Find MTGO IDs for cards that don't already have names
    const unresolvedIds = new Map<number, string[]>(); // mtgo_id → [card_ids]
    for (const [cardId, mtgoId] of Object.entries(cardTextures)) {
      if (!this.cardNames[cardId]) {
        const existing = unresolvedIds.get(mtgoId) ?? [];
        existing.push(cardId);
        unresolvedIds.set(mtgoId, existing);
      }
    }

    if (unresolvedIds.size === 0) return 0;

    const resolved = await resolveCardNamesByMtgoId([...unresolvedIds.keys()]);

    let count = 0;
    for (const [mtgoId, card] of resolved) {
      const cardIds = unresolvedIds.get(mtgoId) ?? [];
      for (const cardId of cardIds) {
        this.cardNames[cardId] = card.name;
        count++;
      }
    }

    return count;
  }

  reconstruct(step: number): BoardState {
    let state = createEmptyBoardState();
    const limit = Math.min(step, this.actions.length);
    for (let i = 0; i < limit; i++) {
      state = applyAction(state, this.actions[i], this.cardNames);
    }
    return state;
  }

  getCardName(cardId: string): string | undefined {
    return this.cardNames[cardId];
  }

  getCardNames(): Record<string, string> {
    return { ...this.cardNames };
  }

  getActionCount(): number {
    return this.actions.length;
  }
}

function applyAction(state: BoardState, raw: RawReplayAction, cardNames: Record<string, string> = {}): BoardState {
  const { type, data } = parseActionType(raw.action_type);

  // Always update turn/phase/active_player from the action envelope
  let newState: BoardState = {
    ...state,
    turn: raw.turn,
    phase: raw.phase,
    activePlayer: raw.active_player,
    zones: state.zones.map(z => ({ ...z, cards: [...z.cards] })),
    stack: [...state.stack],
  };

  switch (type) {
    case 'DrawCard':
      return applyDrawCard(newState, data as { player_id: string; card_id: string }, cardNames);

    case 'PlayLand':
      return applyPlayLand(newState, data as { player_id: string; card_id: string }, cardNames);

    case 'CastSpell':
      return applyCastSpell(newState, data as { player_id: string; card_id: string });

    case 'ActivateAbility':
      return applyActivateAbility(newState, data as { player_id: string; card_id: string; ability_id: string });

    case 'Resolve':
      return applyResolve(newState, data as { card_id: string });

    case 'ZoneTransition':
      return applyZoneTransition(newState, data as { card_id: string; from_zone: string; to_zone: string; player_id?: string }, cardNames);

    case 'LifeChange':
      return applyLifeChange(newState, data as { player_id: string; old_life: number; new_life: number });

    case 'Attack':
      return applyAttack(newState, data as { attacker_id: string; defender_id: string });

    case 'Block':
      return applyBlock(newState, data as { attacker_id: string; blocker_id: string });

    case 'TapPermanent':
      return updateCard(newState, (data as { card_id: string }).card_id, c => ({ ...c, tapped: true }));

    case 'UntapPermanent':
      return updateCard(newState, (data as { card_id: string }).card_id, c => ({ ...c, tapped: false }));

    case 'DamageMarked':
      return updateCard(newState, (data as { card_id: string }).card_id, c => ({
        ...c,
        damage: (data as { damage: number }).damage,
      }));

    case 'SummoningSickness':
      return updateCard(newState, (data as { card_id: string }).card_id, c => ({
        ...c,
        summoningSick: (data as { has_sickness: boolean }).has_sickness,
      }));

    case 'FaceDown':
      return updateCard(newState, (data as { card_id: string }).card_id, c => ({ ...c, faceDown: true }));

    case 'FaceUp':
      return updateCard(newState, (data as { card_id: string }).card_id, c => ({ ...c, faceDown: false }));

    case 'Attach': {
      const { card_id, attached_to_id } = data as { card_id: string; attached_to_id: string };
      return updateCard(newState, card_id, c => ({ ...c, attachedToId: attached_to_id }));
    }

    case 'Detach':
      return updateCard(newState, (data as { card_id: string }).card_id, c => ({ ...c, attachedToId: undefined }));

    case 'CounterUpdate': {
      const { card_id, counter_type, count } = data as { card_id: string; counter_type: string; count: number };
      return updateCard(newState, card_id, c => ({
        ...c,
        counters: [
          ...c.counters.filter(ct => ct.type !== counter_type),
          ...(count > 0 ? [{ type: counter_type, amount: count }] : []),
        ],
      }));
    }

    case 'PowerToughnessUpdate': {
      const { card_id, power, toughness } = data as { card_id: string; power: number; toughness: number };
      return updateCard(newState, card_id, c => ({ ...c, power, toughness }));
    }

    case 'TurnChange':
    case 'PhaseChange':
    case 'PassPriority':
    case 'Unknown':
      // Already handled by envelope update above
      return newState;

    default:
      return newState;
  }
}

// --- Action handlers ---

function applyDrawCard(state: BoardState, data: { player_id: string; card_id: string }, cardNames: Record<string, string>): BoardState {
  const card = { ...createCard(data.card_id, data.player_id), name: cardNames[data.card_id] };
  return addCardToZone(state, 'hand', data.player_id, card);
}

function applyPlayLand(state: BoardState, data: { player_id: string; card_id: string }, cardNames: Record<string, string>): BoardState {
  let s = removeCardFromAnyZone(state, data.card_id);
  const card = findCardAnywhere(state, data.card_id) ?? { ...createCard(data.card_id, data.player_id), name: cardNames[data.card_id] };
  return addCardToZone(s, 'battlefield', data.player_id, { ...card, owner: card.owner ?? data.player_id });
}

function applyCastSpell(state: BoardState, data: { player_id: string; card_id: string }): BoardState {
  let s = removeCardFromAnyZone(state, data.card_id);
  // Add to stack
  s = {
    ...s,
    stack: [...s.stack, { id: data.card_id, controller: data.player_id }],
  };
  return s;
}

function applyActivateAbility(state: BoardState, data: { player_id: string; card_id: string; ability_id: string }): BoardState {
  // ability_id goes on stack, card_id stays on battlefield
  return {
    ...state,
    stack: [...state.stack, { id: data.ability_id, controller: data.player_id }],
  };
}

function applyResolve(state: BoardState, data: { card_id: string }): BoardState {
  return {
    ...state,
    stack: state.stack.filter(s => s.id !== data.card_id),
  };
}

function applyZoneTransition(state: BoardState, data: { card_id: string; from_zone: string; to_zone: string; player_id?: string }, cardNames: Record<string, string>): BoardState {
  const existing = findCardAnywhere(state, data.card_id);
  let s = removeCardFromAnyZone(state, data.card_id);

  // Also remove from stack if leaving stack
  if (data.from_zone === 'stack') {
    s = { ...s, stack: s.stack.filter(so => so.id !== data.card_id) };
  }

  const card = existing ?? { ...createCard(data.card_id, data.player_id), name: cardNames[data.card_id] };
  const owner = data.player_id ?? card.owner;

  // Reset combat state when leaving battlefield
  const resetCard = data.from_zone === 'battlefield'
    ? { ...card, tapped: false, attacking: false, blocking: false, damage: 0, summoningSick: false, counters: [], attachedToId: undefined }
    : card;

  return addCardToZone(s, data.to_zone, owner, { ...resetCard, owner: owner ?? resetCard.owner });
}

function applyLifeChange(state: BoardState, data: { player_id: string; new_life: number }): BoardState {
  return {
    ...state,
    lifeTotals: {
      ...state.lifeTotals,
      [data.player_id]: data.new_life,
    },
  };
}

function applyAttack(state: BoardState, data: { attacker_id: string }): BoardState {
  return updateCard(state, data.attacker_id, c => ({ ...c, attacking: true, tapped: true }));
}

function applyBlock(state: BoardState, data: { attacker_id: string; blocker_id: string }): BoardState {
  return updateCard(state, data.blocker_id, c => ({ ...c, blocking: true }));
}

// --- Zone helpers ---

function getOrCreateZone(state: BoardState, zoneName: string, owner?: string): { state: BoardState; zone: Zone } {
  const existing = state.zones.find(z => z.name === zoneName && z.owner === owner);
  if (existing) return { state, zone: existing };

  const newZone: Zone = { name: zoneName, owner, cards: [] };
  const newState = { ...state, zones: [...state.zones, newZone] };
  return { state: newState, zone: newZone };
}

function addCardToZone(state: BoardState, zoneName: string, owner: string | undefined, card: CardState): BoardState {
  const { state: s } = getOrCreateZone(state, zoneName, owner);
  return {
    ...s,
    zones: s.zones.map(z =>
      z.name === zoneName && z.owner === owner
        ? { ...z, cards: [...z.cards, card] }
        : z
    ),
  };
}

function removeCardFromAnyZone(state: BoardState, cardId: string): BoardState {
  return {
    ...state,
    zones: state.zones.map(z => ({
      ...z,
      cards: z.cards.filter(c => c.id !== cardId),
    })),
  };
}

function findCardAnywhere(state: BoardState, cardId: string): CardState | undefined {
  for (const zone of state.zones) {
    const card = zone.cards.find(c => c.id === cardId);
    if (card) return card;
  }
  return undefined;
}

function updateCard(state: BoardState, cardId: string, updater: (card: CardState) => CardState): BoardState {
  return {
    ...state,
    zones: state.zones.map(z => ({
      ...z,
      cards: z.cards.map(c => c.id === cardId ? updater(c) : c),
    })),
  };
}
