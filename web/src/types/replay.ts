/**
 * Replay file types matching Rust serde output format.
 *
 * Rust serializes enums as externally tagged: {"VariantName": {fields}}
 * We parse these into a normalized {type, ...fields} shape for easier use.
 */

// --- Raw format (matches Rust JSON output) ---

export interface ReplayFile {
  header: ReplayHeader;
  games: GameReplay[];
  metadata: Record<string, string>;
}

export interface ReplayHeader {
  format: string;
  start_time: string;
  end_time?: string | null;
  players: PlayerInfo[];
}

export interface GameHeader {
  game_id: string;
  players: PlayerInfo[];
  result: RawGameResult;
}

export interface GameReplay {
  game_number: number;
  header: GameHeader;
  actions: RawReplayAction[];
  card_names?: Record<string, string>;
  card_textures?: Record<string, number>;
}

export interface PlayerInfo {
  player_id: string;
  name: string;
  life_total: number;
}

/** Rust enum: {"Win": {"winner_id": "..."}} | "Draw" | "Incomplete" */
export type RawGameResult =
  | { Win: { winner_id: string } }
  | 'Draw'
  | 'Incomplete';

/** Raw action as it comes from Rust JSON */
export interface RawReplayAction {
  timestamp?: string;
  turn: number;
  phase: string;
  active_player: string;
  action_type: RawActionType;
}

/**
 * Rust's externally-tagged enum serialization.
 * Each variant is an object with a single key = variant name.
 */
export type RawActionType =
  | { DrawCard: { player_id: string; card_id: string } }
  | { PlayLand: { player_id: string; card_id: string } }
  | { CastSpell: { player_id: string; card_id: string } }
  | { ActivateAbility: { player_id: string; card_id: string; ability_id: string } }
  | { Attack: { attacker_id: string; defender_id: string } }
  | { Block: { attacker_id: string; blocker_id: string } }
  | { Resolve: { card_id: string } }
  | { LifeChange: { player_id: string; old_life: number; new_life: number } }
  | { ZoneTransition: { card_id: string; from_zone: string; to_zone: string; player_id?: string } }
  | { Discard: { player_id: string; card_id: string } }
  | { Mill: { player_id: string; card_id: string } }
  | { CreateToken: { player_id: string; card_id: string; token_name: string } }
  | { TapPermanent: { card_id: string } }
  | { UntapPermanent: { card_id: string } }
  | { DamageMarked: { card_id: string; damage: number } }
  | { SummoningSickness: { card_id: string; has_sickness: boolean } }
  | { FaceDown: { card_id: string } }
  | { FaceUp: { card_id: string } }
  | { Attach: { card_id: string; attached_to_id: string } }
  | { Detach: { card_id: string } }
  | { CounterUpdate: { card_id: string; counter_type: string; count: number } }
  | { PowerToughnessUpdate: { card_id: string; power: number; toughness: number } }
  | { PassPriority: { player_id: string } }
  | { PhaseChange: { phase: string } }
  | { TurnChange: { turn: number; player_id: string } }
  | { Unknown: { description: string } };

// --- Normalized format (easier to work with in TS) ---

export interface ReplayAction {
  timestamp?: string;
  turn: number;
  phase: string;
  active_player: string;
  type: string;
  data: Record<string, unknown>;
}

/** Parse a raw externally-tagged action_type into {type, data} */
export function parseActionType(raw: RawActionType): { type: string; data: Record<string, unknown> } {
  const key = Object.keys(raw)[0] as string;
  const data = (raw as Record<string, Record<string, unknown>>)[key];
  return { type: key, data: data ?? {} };
}

/** Convert a raw action to a normalized action */
export function normalizeAction(raw: RawReplayAction): ReplayAction {
  const { type, data } = parseActionType(raw.action_type);
  return {
    timestamp: raw.timestamp,
    turn: raw.turn,
    phase: raw.phase,
    active_player: raw.active_player,
    type,
    data,
  };
}

/** Get the winner ID from a raw game result, or undefined */
export function getWinnerId(result: RawGameResult): string | undefined {
  if (typeof result === 'object' && 'Win' in result) {
    return result.Win.winner_id;
  }
  return undefined;
}

/** Get human-readable result string */
export function getResultLabel(result: RawGameResult): string {
  if (typeof result === 'string') return result;
  if ('Win' in result) return `Winner: ${result.Win.winner_id}`;
  return 'Unknown';
}
