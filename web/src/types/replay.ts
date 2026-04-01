/**
 * Replay file types for v3 format.
 *
 * v3 uses a single timeline array with interleaved snapshot and event entries.
 * Card metadata is stored once in the card_catalog and referenced by ID.
 * No externally-tagged Rust serde enums — events use a simple { type, ...fields } shape.
 */

// --- Top-Level Structure ---

export interface ReplayFile {
  version: string;
  header: ReplayHeader;
  timeline: TimelineEntry[];
  card_catalog: CardCatalog;
}

export interface ReplayHeader {
  game_id: number;
  players: PlayerInfo[];
  format: string;
  start_time: string;
  end_time?: string | null;
  result: GameResult;
  complete: boolean;
  decklist: Decklist;
  sideboard_changes: SideboardChanges | null;
}

export interface PlayerInfo {
  name: string;
  seat: number;
}

export interface GameResult {
  winner: string;
  reason: string;
}

export interface Decklist {
  mainboard: string[];
  sideboard: string[];
}

export interface SideboardChanges {
  in: string[];
  out: string[];
}

// --- Timeline ---

export type TimelineEntry = SnapshotEntry | EventEntry;

interface TimelineEntryBase {
  turn: number;
  phase: string;
  active_player: string;
}

export interface SnapshotEntry extends TimelineEntryBase {
  type: 'snapshot';
  state: SnapshotState;
}

export interface EventEntry extends TimelineEntryBase {
  type: 'event';
  event: GameEvent;
}

// --- Snapshot State ---

export interface SnapshotState {
  players: PlayerSnapshot[];
  active_player: string;
  priority_player: string;
}

export interface PlayerSnapshot {
  name: string;
  seat: number;
  life: number;
  mana_pool: ManaPool;
  zones: Record<string, ZoneSnapshot>;
}

export interface ManaPool {
  W?: number;
  U?: number;
  B?: number;
  R?: number;
  G?: number;
  C?: number;
}

export interface ZoneSnapshot {
  cards: CardObject[];
  count?: number;
}

export interface CardObject {
  id: string;
  catalog_id: string;
  tapped?: boolean;
  flipped?: boolean;
  face_down?: boolean;
  power?: number;
  toughness?: number;
  damage?: number;
  counters?: Record<string, number>;
  attachments?: string[];
  combat_status?: CombatStatus;
  summoning_sickness?: boolean;
  controller?: string;
}

export interface CombatStatus {
  attacking?: boolean;
  blocking?: boolean;
  attack_target?: string;
  block_target?: string;
}

// --- Event Types ---

export type GameEvent =
  | { type: 'DrawCard'; player: string; card_id: string }
  | { type: 'PlayLand'; player: string; card_id: string }
  | { type: 'CastSpell'; player: string; card_id: string; source_zone?: string; ability_text?: string }
  | { type: 'ActivateAbility'; player: string; card_id: string; ability_text?: string }
  | { type: 'Resolve'; card_id: string }
  | { type: 'ZoneTransition'; card_id: string; from_zone: string; to_zone: string; player?: string }
  | { type: 'Attack'; attacker_id: string; target?: string }
  | { type: 'Block'; blocker_id: string; attacker_id: string }
  | { type: 'LifeChange'; player: string; old_life: number; new_life: number; source?: string }
  | { type: 'TapPermanent'; card_id: string }
  | { type: 'UntapPermanent'; card_id: string }
  | { type: 'DamageMarked'; card_id: string; damage: number }
  | { type: 'SummoningSickness'; card_id: string; has_sickness: boolean }
  | { type: 'FaceDown'; card_id: string }
  | { type: 'FaceUp'; card_id: string }
  | { type: 'Attach'; card_id: string; attached_to_id: string }
  | { type: 'Detach'; card_id: string }
  | { type: 'CounterUpdate'; card_id: string; counter_type: string; count: number }
  | { type: 'PowerToughnessUpdate'; card_id: string; power: number; toughness: number }
  | { type: 'Discard'; player: string; card_id: string }
  | { type: 'Mill'; player: string; card_id: string }
  | { type: 'CreateToken'; player: string; card_id: string; token_name: string }
  | { type: 'TurnChange'; turn: number; player: string }
  | { type: 'PhaseChange'; phase: string }
  | { type: 'PassPriority'; player: string };

// --- Card Catalog ---

export type CardCatalog = Record<string, CardCatalogEntry>;

export interface CardCatalogEntry {
  name: string;
  mana_cost: string;
  type_line: string;
}

// --- Utility Functions ---

/** Get the winner name from a game result, or undefined if no winner */
export function getWinnerId(result: GameResult): string | undefined {
  return result.winner || undefined;
}

/** Get human-readable result string */
export function getResultLabel(result: GameResult): string {
  if (result.winner && result.reason) {
    return `${result.winner} wins (${result.reason})`;
  }
  if (result.winner) {
    return `Winner: ${result.winner}`;
  }
  return result.reason || 'Unknown';
}
