/**
 * Board state types for the v3 replay viewer.
 *
 * Represents the reconstructed game state at any point in time.
 * Updated from v1 to include: mana pool, typed counters, combat targets,
 * attachment lists, summoning sickness, and controller tracking.
 */

export interface BoardState {
  zones: Zone[];
  players: PlayerState[];
  turn: number;
  phase: string;
  activePlayer?: string;
  priorityPlayer?: string;
  stack: StackObject[];
}

export interface PlayerState {
  name: string;
  seat: number;
  life: number;
  manaPool: ManaPool;
}

export interface ManaPool {
  W: number;
  U: number;
  B: number;
  R: number;
  G: number;
  C: number;
}

export interface Zone {
  name: string;
  owner?: string;
  cards: CardState[];
  count?: number;
}

export interface CardState {
  id: string;
  catalogId?: string;
  name?: string;
  owner?: string;
  controller?: string;
  tapped: boolean;
  flipped: boolean;
  faceDown: boolean;
  summoningSick: boolean;
  power?: number;
  toughness?: number;
  damage: number;
  counters: Record<string, number>;
  attachments: string[];
  combatStatus: CombatStatus;
}

export interface CombatStatus {
  attacking: boolean;
  blocking: boolean;
  attackTarget?: string;
  blockTarget?: string;
}

export interface StackObject {
  id: string;
  controller?: string;
}

export function createEmptyManaPool(): ManaPool {
  return { W: 0, U: 0, B: 0, R: 0, G: 0, C: 0 };
}

export function createEmptyBoardState(): BoardState {
  return {
    zones: [],
    players: [],
    turn: 0,
    phase: '',
    stack: [],
  };
}

export function createCard(id: string, owner?: string): CardState {
  return {
    id,
    owner,
    tapped: false,
    flipped: false,
    faceDown: false,
    summoningSick: false,
    damage: 0,
    counters: {},
    attachments: [],
    combatStatus: { attacking: false, blocking: false },
  };
}
