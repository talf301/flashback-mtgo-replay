export interface BoardState {
  zones: Zone[];
  lifeTotals: Record<string, number>;
  turn: number;
  phase: Phase;
  activePlayer?: string;
  stack: StackObject[];
}

export type Phase = 'beginning' | 'main1' | 'combat' | 'main2' | 'end';

export interface Zone {
  name: string;
  owner?: string;
  cards: CardRef[];
}

export interface CardRef {
  id: string;
  scryfall_id?: string;
  name?: string;
  owner?: string;
  is_face_down?: boolean;
  counters: Counter[];
}

export interface Counter {
  type: string;
  amount: number;
}

export interface StackObject {
  id: string;
  card_id: string;
  controller: string;
  targets: string[];
}

export function createEmptyBoardState(): BoardState {
  return {
    zones: [
      { name: 'battlefield', cards: [] },
      { name: 'hand', cards: [] },
      { name: 'graveyard', cards: [] },
      { name: 'exile', cards: [] },
    ],
    lifeTotals: {},
    turn: 1,
    phase: 'beginning',
    stack: [],
  };
}
