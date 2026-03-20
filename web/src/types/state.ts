export interface BoardState {
  zones: Zone[];
  lifeTotals: Record<string, number>;
  turn: number;
  phase: string;
  activePlayer?: string;
  stack: StackObject[];
}

export interface Zone {
  name: string;
  owner?: string;
  cards: CardState[];
}

export interface CardState {
  id: string;
  name?: string;
  owner?: string;
  controller?: string;
  tapped: boolean;
  attacking: boolean;
  blocking: boolean;
  faceDown: boolean;
  summoningSick: boolean;
  power?: number;
  toughness?: number;
  damage: number;
  counters: Counter[];
  attachedToId?: string;
}

export interface Counter {
  type: string;
  amount: number;
}

export interface StackObject {
  id: string;
  controller?: string;
}

export function createEmptyBoardState(): BoardState {
  return {
    zones: [],
    lifeTotals: {},
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
    attacking: false,
    blocking: false,
    faceDown: false,
    summoningSick: false,
    damage: 0,
    counters: [],
  };
}
