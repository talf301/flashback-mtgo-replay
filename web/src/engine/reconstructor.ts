import {
  BoardState,
  createEmptyBoardState,
  CardState,
  createCard,
  Zone,
  PlayerState,
} from '../types/state';
import {
  ReplayFile,
  TimelineEntry,
  SnapshotEntry,
  EventEntry,
  GameEvent,
  CardCatalog,
  CardObject,
} from '../types/replay';

/**
 * Reconstructs board state from the v3 snapshot+event timeline.
 *
 * Seeking works by binary-searching for the nearest snapshot at or before
 * the requested position, then replaying events forward from there.
 * Snapshots replace the entire BoardState; events apply incremental updates.
 */
export class Reconstructor {
  private timeline: TimelineEntry[] = [];
  private catalog: CardCatalog = {};
  /** Sorted indices of snapshot entries for binary search */
  private snapshotIndices: number[] = [];

  loadReplay(replay: ReplayFile): void {
    this.timeline = replay.timeline;
    this.catalog = replay.card_catalog;
    this.snapshotIndices = [];
    for (let i = 0; i < this.timeline.length; i++) {
      if (this.timeline[i].type === 'snapshot') {
        this.snapshotIndices.push(i);
      }
    }
  }

  /**
   * Reconstruct the board state at a given timeline position.
   * Position is 0-based; position 0 = state before any entries,
   * position N = state after applying entries [0..N-1].
   */
  reconstruct(position: number): BoardState {
    if (this.timeline.length === 0 || position <= 0) {
      return createEmptyBoardState();
    }

    const limit = Math.min(position, this.timeline.length);

    // Find the nearest snapshot at or before `limit - 1`
    const snapshotIdx = this.findNearestSnapshot(limit - 1);

    let state: BoardState;
    let startFrom: number;

    if (snapshotIdx !== -1) {
      const entry = this.timeline[snapshotIdx] as SnapshotEntry;
      state = snapshotToBoardState(entry, this.catalog);
      startFrom = snapshotIdx + 1;
    } else {
      state = createEmptyBoardState();
      startFrom = 0;
    }

    // Replay events forward from snapshot to the requested position
    for (let i = startFrom; i < limit; i++) {
      const entry = this.timeline[i];
      if (entry.type === 'snapshot') {
        state = snapshotToBoardState(entry, this.catalog);
      } else {
        state = applyEvent(state, entry, this.catalog);
      }
    }

    return state;
  }

  getCardName(cardId: string): string | undefined {
    // cardId is the instance id; we need to look through catalog by catalogId
    // But callers pass card instance IDs. We need to search the catalog differently.
    // The catalog is keyed by catalog_id. Cards reference catalog_id.
    // To look up by instance id, we'd need the current state. Just return from catalog directly.
    return this.catalog[cardId]?.name;
  }

  getCardNames(): Record<string, string> {
    const names: Record<string, string> = {};
    for (const [id, entry] of Object.entries(this.catalog)) {
      names[id] = entry.name;
    }
    return names;
  }

  getCatalog(): CardCatalog {
    return this.catalog;
  }

  getTimelineLength(): number {
    return this.timeline.length;
  }

  /** @deprecated Use getTimelineLength(). Kept for compatibility during migration. */
  getActionCount(): number {
    return this.timeline.length;
  }

  /**
   * Binary search for the largest snapshot index <= maxIndex.
   * Returns -1 if no snapshot exists at or before maxIndex.
   */
  private findNearestSnapshot(maxIndex: number): number {
    const indices = this.snapshotIndices;
    if (indices.length === 0) return -1;

    let lo = 0;
    let hi = indices.length - 1;
    let result = -1;

    while (lo <= hi) {
      const mid = (lo + hi) >>> 1;
      if (indices[mid] <= maxIndex) {
        result = indices[mid];
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }

    return result;
  }
}

// --- Snapshot → BoardState conversion ---

function snapshotToBoardState(entry: SnapshotEntry, catalog: CardCatalog): BoardState {
  const snap = entry.state;
  const zones: Zone[] = [];
  const players: PlayerState[] = [];

  for (const playerSnap of snap.players) {
    players.push({
      name: playerSnap.name,
      seat: playerSnap.seat,
      life: playerSnap.life,
      manaPool: {
        W: playerSnap.mana_pool.W ?? 0,
        U: playerSnap.mana_pool.U ?? 0,
        B: playerSnap.mana_pool.B ?? 0,
        R: playerSnap.mana_pool.R ?? 0,
        G: playerSnap.mana_pool.G ?? 0,
        C: playerSnap.mana_pool.C ?? 0,
      },
    });

    for (const [zoneName, zoneSnap] of Object.entries(playerSnap.zones)) {
      zones.push({
        name: zoneName,
        owner: playerSnap.name,
        cards: zoneSnap.cards.map(c => cardObjectToCardState(c, playerSnap.name, catalog)),
        count: zoneSnap.count,
      });
    }
  }

  return {
    zones,
    players,
    turn: entry.turn,
    phase: entry.phase,
    activePlayer: snap.active_player,
    priorityPlayer: snap.priority_player,
    stack: [],
  };
}

function cardObjectToCardState(obj: CardObject, owner: string, catalog: CardCatalog): CardState {
  const catalogEntry = catalog[obj.catalog_id];
  return {
    id: obj.id,
    catalogId: obj.catalog_id,
    name: catalogEntry?.name,
    owner,
    controller: obj.controller ?? owner,
    tapped: obj.tapped ?? false,
    flipped: obj.flipped ?? false,
    faceDown: obj.face_down ?? false,
    summoningSick: obj.summoning_sickness ?? false,
    power: obj.power,
    toughness: obj.toughness,
    damage: obj.damage ?? 0,
    counters: obj.counters ?? {},
    attachments: obj.attachments ?? [],
    combatStatus: {
      attacking: obj.combat_status?.attacking ?? false,
      blocking: obj.combat_status?.blocking ?? false,
      attackTarget: obj.combat_status?.attack_target,
      blockTarget: obj.combat_status?.block_target,
    },
  };
}

// --- Event application ---

function applyEvent(state: BoardState, entry: EventEntry, catalog: CardCatalog): BoardState {
  // Update turn/phase/active_player from event envelope
  let newState: BoardState = {
    ...state,
    turn: entry.turn,
    phase: entry.phase,
    activePlayer: entry.active_player,
    zones: state.zones.map(z => ({ ...z, cards: [...z.cards] })),
    stack: [...state.stack],
    players: state.players.map(p => ({ ...p })),
  };

  return applyGameEvent(newState, entry.event, catalog);
}

function applyGameEvent(state: BoardState, event: GameEvent, catalog: CardCatalog): BoardState {
  switch (event.type) {
    case 'DrawCard': {
      const card = createCardFromCatalog(event.card_id, event.player, catalog);
      return addCardToZone(state, 'hand', event.player, card);
    }

    case 'PlayLand': {
      let s = removeCardFromAnyZone(state, event.card_id);
      const existing = findCardAnywhere(state, event.card_id);
      const card = existing ?? createCardFromCatalog(event.card_id, event.player, catalog);
      return addCardToZone(s, 'battlefield', event.player, { ...card, owner: card.owner ?? event.player });
    }

    case 'CastSpell': {
      let s = removeCardFromAnyZone(state, event.card_id);
      return {
        ...s,
        stack: [...s.stack, { id: event.card_id, controller: event.player }],
      };
    }

    case 'ActivateAbility':
      // The ability goes on the stack; the permanent stays
      return {
        ...state,
        stack: [...state.stack, { id: `ability-${event.card_id}`, controller: event.player }],
      };

    case 'Resolve':
      return {
        ...state,
        stack: state.stack.filter(s => s.id !== event.card_id),
      };

    case 'ZoneTransition': {
      const existing = findCardAnywhere(state, event.card_id);
      let s = removeCardFromAnyZone(state, event.card_id);

      if (event.from_zone === 'stack') {
        s = { ...s, stack: s.stack.filter(so => so.id !== event.card_id) };
      }

      const card = existing ?? createCardFromCatalog(event.card_id, event.player, catalog);
      const owner = event.player ?? card.owner;

      // Reset transient state when leaving battlefield
      const resetCard = event.from_zone === 'battlefield'
        ? {
            ...card,
            tapped: false,
            damage: 0,
            summoningSick: false,
            counters: {},
            attachments: [],
            combatStatus: { attacking: false, blocking: false },
          }
        : card;

      return addCardToZone(s, event.to_zone, owner, { ...resetCard, owner: owner ?? resetCard.owner });
    }

    case 'LifeChange':
      return {
        ...state,
        players: state.players.map(p =>
          p.name === event.player ? { ...p, life: event.new_life } : p
        ),
      };

    case 'Attack':
      return updateCard(state, event.attacker_id, c => ({
        ...c,
        tapped: true,
        combatStatus: { ...c.combatStatus, attacking: true, attackTarget: event.target },
      }));

    case 'Block':
      return updateCard(state, event.blocker_id, c => ({
        ...c,
        combatStatus: { ...c.combatStatus, blocking: true, blockTarget: event.attacker_id },
      }));

    case 'TapPermanent':
      return updateCard(state, event.card_id, c => ({ ...c, tapped: true }));

    case 'UntapPermanent':
      return updateCard(state, event.card_id, c => ({ ...c, tapped: false }));

    case 'DamageMarked':
      return updateCard(state, event.card_id, c => ({ ...c, damage: event.damage }));

    case 'SummoningSickness':
      return updateCard(state, event.card_id, c => ({ ...c, summoningSick: event.has_sickness }));

    case 'FaceDown':
      return updateCard(state, event.card_id, c => ({ ...c, faceDown: true }));

    case 'FaceUp':
      return updateCard(state, event.card_id, c => ({ ...c, faceDown: false }));

    case 'Attach':
      return updateCard(state, event.card_id, c => ({
        ...c,
        attachments: [...c.attachments, event.attached_to_id],
      }));

    case 'Detach':
      return updateCard(state, event.card_id, c => ({
        ...c,
        attachments: [],
      }));

    case 'CounterUpdate':
      return updateCard(state, event.card_id, c => {
        const counters = { ...c.counters };
        if (event.count > 0) {
          counters[event.counter_type] = event.count;
        } else {
          delete counters[event.counter_type];
        }
        return { ...c, counters };
      });

    case 'PowerToughnessUpdate':
      return updateCard(state, event.card_id, c => ({
        ...c,
        power: event.power,
        toughness: event.toughness,
      }));

    case 'Discard':
      return applyGameEvent(state, {
        type: 'ZoneTransition',
        card_id: event.card_id,
        from_zone: 'hand',
        to_zone: 'graveyard',
        player: event.player,
      }, catalog);

    case 'Mill':
      return applyGameEvent(state, {
        type: 'ZoneTransition',
        card_id: event.card_id,
        from_zone: 'library',
        to_zone: 'graveyard',
        player: event.player,
      }, catalog);

    case 'CreateToken': {
      const tokenCard = {
        ...createCard(event.card_id, event.player),
        name: event.token_name,
      };
      return addCardToZone(state, 'battlefield', event.player, tokenCard);
    }

    case 'TurnChange':
    case 'PhaseChange':
    case 'PassPriority':
      // Already handled by envelope update
      return state;

    default:
      return state;
  }
}

// --- Helpers ---

function createCardFromCatalog(cardId: string, owner: string | undefined, catalog: CardCatalog): CardState {
  // cardId is the instance ID; we need to find its catalog entry
  // For events, the card_id is the instance id. The catalog is keyed by catalog_id.
  // We don't have the catalog_id mapping from events alone, so we create a basic card.
  // The catalog_id linkage is established when cards appear in snapshots.
  const card = createCard(cardId, owner);
  // Try direct lookup (works when instance id matches catalog id)
  const entry = catalog[cardId];
  if (entry) {
    return { ...card, catalogId: cardId, name: entry.name };
  }
  return card;
}

function addCardToZone(state: BoardState, zoneName: string, owner: string | undefined, card: CardState): BoardState {
  // Ensure zone exists
  const hasZone = state.zones.some(z => z.name === zoneName && z.owner === owner);
  const zones = hasZone
    ? state.zones
    : [...state.zones, { name: zoneName, owner, cards: [] } as Zone];

  return {
    ...state,
    zones: zones.map(z =>
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
