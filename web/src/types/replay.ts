export interface ReplayFile {
  version: string;
  header: ReplayHeader;
  actions: ReplayAction[];
}

export interface ReplayHeader {
  game_id: string;
  format: string;
  start_time: string;
  end_time?: string;
  players: PlayerInfo[];
  result?: GameResult;
}

export interface PlayerInfo {
  id: string;
  name: string;
  deck_hash?: string;
}

export interface GameResult {
  winner: string;
  reason: string;
}

export interface ReplayAction {
  timestamp: string;
  turn?: number;
  phase?: string;
  active_player?: string;
  action_type: ActionType;
}

export type ActionType =
  | { type: 'DrawCard'; card_id: string }
  | { type: 'PlayLand'; card_id: string }
  | { type: 'CastSpell'; card_id: string; targets: string[] }
  | { type: 'ActivateAbility'; card_id: string; ability_id: string }
  | { type: 'Attack'; attacker_id: string; defender_id: string }
  | { type: 'Block'; blocker_id: string; attacker_id: string }
  | { type: 'Damage'; target_id: string; amount: number }
  | { type: 'LifeChange'; player_id: string; old: number; new: number }
  | { type: 'PassPriority' }
  | { type: 'ResolveSpell'; card_id: string }
  | { type: 'TokenCreate'; token_id: string; card_id: string }
  | { type: 'ZoneChange'; card_id: string; from: string; to: string }
  | { type: 'CounterAdd'; card_id: string; counter_type: string; amount: number }
  | { type: 'GameEnd'; winner: string };
