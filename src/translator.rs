//! State-diff to ReplayAction translation.
//!
//! Compares consecutive `GameState` snapshots and emits `ReplayAction`s
//! describing what changed between them.

use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{DateTime, Utc};

use crate::chat::{self, ChatEvent};
use crate::replay::schema::{ActionType, ReplayAction};
use crate::state::{GamePhase, GameState};

/// MTGO CardZone constants (from decompiled client, verified against golden file).
/// Note: the spec documentation listed different values (1=Hand, 2=Library, 7=Battlefield)
/// but the actual wire data uses: 0=Battlefield, 1=Hand, 2=Library, 8=Stack.
const ZONE_BATTLEFIELD: i32 = 0;
const ZONE_HAND: i32 = 1;
const ZONE_LIBRARY: i32 = 2;
const ZONE_STACK: i32 = 8;

/// Translates state diffs into replay actions.
pub struct ReplayTranslator {
    prev: Option<GameState>,
    player_names: Vec<String>,
    start_time: Option<DateTime<Utc>>,
    things_seen_on_stack: HashSet<u32>,
    /// Tracks the last known zone of every thing_id ever seen, surviving
    /// full-state prunes.  When a thing disappears from the game state
    /// (pruned by a full state) and later reappears at a different zone,
    /// we can still detect the zone change.
    last_known_zones: HashMap<u32, i32>,
    /// Tracks the last phase for which we emitted a PhaseChange action.
    /// Prevents duplicate PhaseChange events caused by interleaved MTGO
    /// state updates from both players' perspectives during the same phase.
    /// Cleared on TurnChange so the first phase of each turn is always emitted.
    last_emitted_phase: Option<GamePhase>,
    /// Per-thing-id chat context for zone resolution and action classification.
    chat_context: HashMap<u32, ChatEvent>,
    /// Pending token creation events keyed by token name.
    pending_tokens: VecDeque<(String, ChatEvent)>,
}

impl ReplayTranslator {
    pub fn new() -> Self {
        ReplayTranslator {
            prev: None,
            player_names: Vec::new(),
            start_time: None,
            things_seen_on_stack: HashSet::new(),
            last_known_zones: HashMap::new(),
            last_emitted_phase: None,
            chat_context: HashMap::new(),
            pending_tokens: VecDeque::new(),
        }
    }

    pub fn set_player_names(&mut self, names: Vec<String>) {
        self.player_names = names;
    }

    pub fn set_start_time(&mut self, t: DateTime<Utc>) {
        self.start_time = Some(t);
    }

    /// Feed a chat message into the translator's context.
    /// Call this for every UserChat message before processing state diffs.
    pub fn ingest_chat(&mut self, text: &str) {
        if let Some(event) = chat::parse_chat(text) {
            match &event {
                ChatEvent::CreateToken { token_name, .. } => {
                    self.pending_tokens.push_back((token_name.clone(), event));
                }
                ChatEvent::Discard { card, .. }
                | ChatEvent::PutIntoGraveyard { card, .. }
                | ChatEvent::Exile { card, .. }
                | ChatEvent::PutOnLibrary { card, .. } => {
                    self.chat_context.insert(card.thing_id, event);
                }
            }
        }
    }

    /// Reset translator state for a new game in a multi-game session.
    /// Player names are preserved across games since they don't change in a match.
    pub fn reset(&mut self) {
        self.prev = None;
        self.start_time = None;
        self.things_seen_on_stack.clear();
        self.last_known_zones.clear();
        self.last_emitted_phase = None;
        self.chat_context.clear();
        self.pending_tokens.clear();
    }

    /// Process a new game state, emitting actions for everything that changed.
    ///
    /// When `is_full_state` is true, diff is still run for existing things
    /// (to catch zone changes) but new-thing actions are suppressed — full
    /// state snapshots reveal previously-hidden cards that are not new plays.
    pub fn process(&mut self, new_state: &GameState, is_full_state: bool) -> Vec<ReplayAction> {
        // Record stack occupants BEFORE evaluating diffs
        for (thing_id, thing) in &new_state.things {
            if thing.zone == ZONE_STACK {
                self.things_seen_on_stack.insert(*thing_id);
            }
        }

        // Detect phase regression: the phase moved backward within the
        // same turn, likely due to interleaved MTGO state updates.
        let phase_regressed = if let Some(ref prev) = self.prev {
            let same_turn = new_state.turn == prev.turn;
            let regresses = new_state.phase.ordinal() < prev.phase.ordinal()
                && new_state.phase.ordinal() != 255
                && prev.phase.ordinal() != 255;
            same_turn && regresses
        } else {
            false
        };

        // Suppress all actions during pre-game setup (turn 0, unknown phase).
        // These are initial state snapshots revealing existing board state,
        // not actual game actions.  We still update prev/zones below so
        // diffing works once real gameplay begins.
        let in_pregame = new_state.turn == 0 && new_state.phase.ordinal() == 255;

        let actions = if in_pregame {
            Vec::new()
        } else if let Some(prev) = self.prev.take() {
            let result = self.diff(&prev, new_state, is_full_state, phase_regressed);
            self.prev = Some(prev);
            result
        } else {
            // First state — no diff possible
            Vec::new()
        };

        // Update last_emitted_phase tracking
        for action in &actions {
            match &action.action_type {
                ActionType::TurnChange { .. } => {
                    self.last_emitted_phase = None;
                }
                ActionType::PhaseChange { .. } => {
                    self.last_emitted_phase = Some(new_state.phase.clone());
                }
                _ => {}
            }
        }

        // Update last_known_zones for every thing in the new state.
        for (thing_id, thing) in &new_state.things {
            self.last_known_zones.insert(*thing_id, thing.zone);
        }

        // Clear from_zone on all things after diffing (we clone for storage)
        let mut stored = new_state.clone();
        for thing in stored.things.values_mut() {
            thing.from_zone = None;
        }
        self.prev = Some(stored);

        actions
    }

    fn player_name(&self, index: usize) -> String {
        self.player_names
            .get(index)
            .cloned()
            .unwrap_or_else(|| format!("player_{}", index))
    }

    fn zone_name(zone: i32) -> &'static str {
        match zone {
            ZONE_BATTLEFIELD => "battlefield",
            ZONE_HAND => "hand",
            ZONE_LIBRARY => "library",
            3 => "graveyard",
            4 => "exile",
            ZONE_STACK => "stack",
            9 => "command",
            16 => "revealed",
            20 => "nowhere",
            21 => "sideboard",
            24 => "effects",
            _ => "unknown",
        }
    }

    fn make_action(&self, new_state: &GameState, action_type: ActionType) -> ReplayAction {
        let timestamp = self.start_time.unwrap_or_else(Utc::now);
        ReplayAction {
            timestamp,
            turn: new_state.turn,
            phase: new_state.phase.to_string(),
            active_player: self.player_name(new_state.active_player),
            action_type,
        }
    }

    /// Diff two states.
    ///
    /// `suppress_new_things`: when true, skip action generation for things
    ///     that appear for the first time (not in prev).  Used for full-state
    ///     snapshots where "new" things are just becoming visible.
    /// `suppress_phase`: when true, skip TurnChange / PhaseChange / LifeChange
    ///     because the state update regressed the phase within a turn.
    fn diff(
        &mut self,
        prev: &GameState,
        new: &GameState,
        suppress_new_things: bool,
        suppress_phase: bool,
    ) -> Vec<ReplayAction> {
        let mut actions = Vec::new();

        if !suppress_phase {
            // Turn change
            if new.turn > prev.turn {
                actions.push(self.make_action(
                    new,
                    ActionType::TurnChange {
                        turn: new.turn,
                        player_id: self.player_name(new.active_player),
                    },
                ));
            }

            // Phase change — only emit when advancing forward within a turn
            // (or when the turn changed, which resets the phase sequence).
            // Also skip if we already emitted this exact phase (dedup for
            // interleaved MTGO state updates).
            let turn_changed = new.turn > prev.turn;
            let already_emitted = self.last_emitted_phase.as_ref()
                .map_or(false, |emitted| *emitted == new.phase);
            if new.phase != prev.phase
                && !already_emitted
                && (turn_changed || new.phase.ordinal() > prev.phase.ordinal())
            {
                actions.push(self.make_action(
                    new,
                    ActionType::PhaseChange {
                        phase: new.phase.to_string(),
                    },
                ));
            }

            // Life changes
            let player_count = prev.players.len().min(new.players.len());
            for i in 0..player_count {
                if new.players[i].life != prev.players[i].life {
                    actions.push(self.make_action(
                        new,
                        ActionType::LifeChange {
                            player_id: self.player_name(i),
                            old_life: prev.players[i].life,
                            new_life: new.players[i].life,
                        },
                    ));
                }
            }
        }

        // Thing diffs — iterate in sorted order so output is deterministic.
        let mut thing_ids: Vec<u32> = new.things.keys().copied().collect();
        thing_ids.sort_unstable();
        for thing_id in &thing_ids {
            let new_thing = &new.things[thing_id];
            let card_id = thing_id.to_string();

            if let Some(old_thing) = prev.things.get(thing_id) {
                // Existing thing — check for changes

                // Zone transition
                if new_thing.zone != old_thing.zone {
                    let from = old_thing.zone;
                    let to = new_thing.zone;

                    if from == ZONE_LIBRARY && to == ZONE_HAND {
                        // Draw
                        actions.push(self.make_action(
                            new,
                            ActionType::DrawCard {
                                player_id: self.player_name(new_thing.controller as usize),
                                card_id: card_id.clone(),
                            },
                        ));
                    } else if from == ZONE_HAND
                        && to == ZONE_BATTLEFIELD
                        && !self.things_seen_on_stack.contains(thing_id)
                    {
                        // Play land
                        actions.push(self.make_action(
                            new,
                            ActionType::PlayLand {
                                player_id: self.player_name(new_thing.controller as usize),
                                card_id: card_id.clone(),
                            },
                        ));
                    } else if from == ZONE_HAND && to == ZONE_STACK {
                        // Check for activated ability vs cast spell
                        if let Some(src_id) = new_thing.src_thing_id {
                            if new.things.get(&src_id).map_or(false, |src| {
                                src.zone == ZONE_BATTLEFIELD
                            }) {
                                actions.push(self.make_action(
                                    new,
                                    ActionType::ActivateAbility {
                                        player_id: self.player_name(new_thing.controller as usize),
                                        card_id: src_id.to_string(),
                                        ability_id: card_id.clone(),
                                    },
                                ));
                            } else {
                                actions.push(self.make_action(
                                    new,
                                    ActionType::CastSpell {
                                        player_id: self.player_name(new_thing.controller as usize),
                                        card_id: card_id.clone(),
                                    },
                                ));
                            }
                        } else {
                            actions.push(self.make_action(
                                new,
                                ActionType::CastSpell {
                                    player_id: self.player_name(new_thing.controller as usize),
                                    card_id: card_id.clone(),
                                },
                            ));
                        }
                    } else if from == ZONE_STACK && to != ZONE_BATTLEFIELD {
                        // Left stack to non-battlefield
                        actions.push(self.make_action(
                            new,
                            ActionType::Resolve {
                                card_id: card_id.clone(),
                            },
                        ));
                    } else {
                        // General zone transition (includes stack→battlefield)
                        actions.push(self.make_action(
                            new,
                            ActionType::ZoneTransition {
                                card_id: card_id.clone(),
                                from_zone: Self::zone_name(from).to_string(),
                                to_zone: Self::zone_name(to).to_string(),
                                player_id: Some(
                                    self.player_name(new_thing.controller as usize),
                                ),
                            },
                        ));
                    }
                }

                // Attacking
                if new_thing.attacking && !old_thing.attacking {
                    actions.push(self.make_action(
                        new,
                        ActionType::Attack {
                            attacker_id: card_id.clone(),
                            defender_id: self.player_name(
                                // Defender is the other player
                                if new_thing.controller as usize == 0 {
                                    1
                                } else {
                                    0
                                },
                            ),
                        },
                    ));
                }

                // Blocking
                if new_thing.blocking && !old_thing.blocking {
                    actions.push(self.make_action(
                        new,
                        ActionType::Block {
                            attacker_id: String::new(), // TODO: determine actual attacker
                            blocker_id: card_id.clone(),
                        },
                    ));
                }

                // Tapped
                if new_thing.tapped != old_thing.tapped {
                    if new_thing.tapped {
                        actions.push(self.make_action(
                            new,
                            ActionType::TapPermanent {
                                card_id: card_id.clone(),
                            },
                        ));
                    } else {
                        actions.push(self.make_action(
                            new,
                            ActionType::UntapPermanent {
                                card_id: card_id.clone(),
                            },
                        ));
                    }
                }

                // Damage
                if new_thing.damage != old_thing.damage {
                    actions.push(self.make_action(
                        new,
                        ActionType::DamageMarked {
                            card_id: card_id.clone(),
                            damage: new_thing.damage,
                        },
                    ));
                }

                // Summoning sickness
                if new_thing.summoning_sickness != old_thing.summoning_sickness {
                    actions.push(self.make_action(
                        new,
                        ActionType::SummoningSickness {
                            card_id: card_id.clone(),
                            has_sickness: new_thing.summoning_sickness,
                        },
                    ));
                }

                // Face down
                if new_thing.face_down != old_thing.face_down {
                    if new_thing.face_down {
                        actions.push(self.make_action(
                            new,
                            ActionType::FaceDown {
                                card_id: card_id.clone(),
                            },
                        ));
                    } else {
                        actions.push(self.make_action(
                            new,
                            ActionType::FaceUp {
                                card_id: card_id.clone(),
                            },
                        ));
                    }
                }

                // Attached
                if new_thing.attached_to_id != old_thing.attached_to_id {
                    if let Some(target_id) = new_thing.attached_to_id {
                        actions.push(self.make_action(
                            new,
                            ActionType::Attach {
                                card_id: card_id.clone(),
                                attached_to_id: target_id.to_string(),
                            },
                        ));
                    } else {
                        actions.push(self.make_action(
                            new,
                            ActionType::Detach {
                                card_id: card_id.clone(),
                            },
                        ));
                    }
                }

                // +1/+1 counters
                if new_thing.plus_counters != old_thing.plus_counters {
                    actions.push(self.make_action(
                        new,
                        ActionType::CounterUpdate {
                            card_id: card_id.clone(),
                            counter_type: "+1/+1".to_string(),
                            count: new_thing.plus_counters,
                        },
                    ));
                }

                // -1/-1 counters
                if new_thing.minus_counters != old_thing.minus_counters {
                    actions.push(self.make_action(
                        new,
                        ActionType::CounterUpdate {
                            card_id: card_id.clone(),
                            counter_type: "-1/-1".to_string(),
                            count: new_thing.minus_counters,
                        },
                    ));
                }

                // Loyalty
                if new_thing.loyalty != old_thing.loyalty {
                    actions.push(self.make_action(
                        new,
                        ActionType::CounterUpdate {
                            card_id: card_id.clone(),
                            counter_type: "loyalty".to_string(),
                            count: new_thing.loyalty,
                        },
                    ));
                }

                // Power/toughness
                if new_thing.power != old_thing.power
                    || new_thing.toughness != old_thing.toughness
                {
                    actions.push(self.make_action(
                        new,
                        ActionType::PowerToughnessUpdate {
                            card_id: card_id.clone(),
                            power: new_thing.power,
                            toughness: new_thing.toughness,
                        },
                    ));
                }
            } else {
                // New thing — not in prev state.  Use from_zone to distinguish
                // real zone transitions from visibility-only appearances.
                let moved = new_thing.from_zone == Some(-1);
                let zone = new_thing.zone;

                // Check chat context for this thing (authoritative source)
                let chat_event = self.chat_context.remove(thing_id);

                // Resolve source zone: chat > last_known_zones > unknown
                let resolved_from_zone = if let Some(ref evt) = chat_event {
                    match evt {
                        ChatEvent::Discard { .. } => Some(ZONE_HAND),
                        ChatEvent::PutIntoGraveyard { .. } => {
                            Some(self.last_known_zones.get(thing_id).copied().unwrap_or(ZONE_LIBRARY))
                        }
                        ChatEvent::Exile { .. } | ChatEvent::PutOnLibrary { .. } => {
                            self.last_known_zones.get(thing_id).copied()
                        }
                        ChatEvent::CreateToken { .. } => None,
                    }
                } else {
                    self.last_known_zones.get(thing_id).copied()
                };

                // Token creation check
                if zone == ZONE_BATTLEFIELD && new_thing.is_token {
                    if let Some(pos) = self.pending_tokens.iter().position(|(name, _)| {
                        new_thing.card_name.as_deref() == Some(name.as_str())
                    }) {
                        let (token_name, _) = self.pending_tokens.remove(pos).unwrap();
                        actions.push(self.make_action(
                            new,
                            ActionType::CreateToken {
                                player_id: self.player_name(new_thing.controller as usize),
                                card_id: card_id.clone(),
                                token_name,
                            },
                        ));
                        continue;
                    }
                }

                // Chat-driven action classification
                if let Some(ref evt) = chat_event {
                    match evt {
                        ChatEvent::Discard { .. } => {
                            actions.push(self.make_action(
                                new,
                                ActionType::Discard {
                                    player_id: self.player_name(new_thing.controller as usize),
                                    card_id: card_id.clone(),
                                },
                            ));
                            continue;
                        }
                        ChatEvent::PutIntoGraveyard { .. } => {
                            let from = resolved_from_zone.unwrap_or(ZONE_LIBRARY);
                            if from == ZONE_LIBRARY {
                                actions.push(self.make_action(
                                    new,
                                    ActionType::Mill {
                                        player_id: self.player_name(new_thing.controller as usize),
                                        card_id: card_id.clone(),
                                    },
                                ));
                            } else {
                                actions.push(self.make_action(
                                    new,
                                    ActionType::ZoneTransition {
                                        card_id: card_id.clone(),
                                        from_zone: Self::zone_name(from).to_string(),
                                        to_zone: Self::zone_name(zone).to_string(),
                                        player_id: Some(self.player_name(new_thing.controller as usize)),
                                    },
                                ));
                            }
                            continue;
                        }
                        _ => {
                            // Exile, PutOnLibrary → fall through to zone transition with resolved from
                        }
                    }
                }

                // Original logic with resolved from_zone
                if zone == ZONE_HAND && moved {
                    actions.push(self.make_action(
                        new,
                        ActionType::DrawCard {
                            player_id: self.player_name(new_thing.controller as usize),
                            card_id: card_id.clone(),
                        },
                    ));
                } else if zone == ZONE_STACK && moved {
                    if let Some(src_id) = new_thing.src_thing_id {
                        if new.things.get(&src_id).map_or(false, |src| {
                            src.zone == ZONE_BATTLEFIELD
                        }) {
                            actions.push(self.make_action(
                                new,
                                ActionType::ActivateAbility {
                                    player_id: self.player_name(new_thing.controller as usize),
                                    card_id: src_id.to_string(),
                                    ability_id: card_id.clone(),
                                },
                            ));
                        } else {
                            actions.push(self.make_action(
                                new,
                                ActionType::CastSpell {
                                    player_id: self.player_name(new_thing.controller as usize),
                                    card_id: card_id.clone(),
                                },
                            ));
                        }
                    } else {
                        actions.push(self.make_action(
                            new,
                            ActionType::CastSpell {
                                player_id: self.player_name(new_thing.controller as usize),
                                card_id: card_id.clone(),
                            },
                        ));
                    }
                } else if zone == ZONE_BATTLEFIELD && moved {
                    if self.things_seen_on_stack.contains(thing_id) {
                        actions.push(self.make_action(
                            new,
                            ActionType::ZoneTransition {
                                card_id: card_id.clone(),
                                from_zone: "stack".to_string(),
                                to_zone: "battlefield".to_string(),
                                player_id: Some(self.player_name(new_thing.controller as usize)),
                            },
                        ));
                    } else {
                        actions.push(self.make_action(
                            new,
                            ActionType::PlayLand {
                                player_id: self.player_name(new_thing.controller as usize),
                                card_id: card_id.clone(),
                            },
                        ));
                    }
                } else if moved && zone != ZONE_LIBRARY {
                    let from_name = resolved_from_zone
                        .map(|z| Self::zone_name(z))
                        .unwrap_or("unknown");
                    actions.push(self.make_action(
                        new,
                        ActionType::ZoneTransition {
                            card_id: card_id.clone(),
                            from_zone: from_name.to_string(),
                            to_zone: Self::zone_name(zone).to_string(),
                            player_id: Some(self.player_name(new_thing.controller as usize)),
                        },
                    ));
                }
                // from_zone == None (no movement) or zone == library:
                // visibility change or game setup, not a real action.
            }
        }

        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{GamePhase, GameState, PlayerState, ThingState};
    use std::collections::HashMap;

    fn make_state(game_id: u32, turn: i32, phase: GamePhase) -> GameState {
        GameState {
            game_id,
            turn,
            phase,
            active_player: 0,
            players: vec![
                PlayerState {
                    life: 20,
                    hand_count: 7,
                    library_count: 53,
                    graveyard_count: 0,
                },
                PlayerState {
                    life: 20,
                    hand_count: 7,
                    library_count: 53,
                    graveyard_count: 0,
                },
            ],
            things: HashMap::new(),
        }
    }

    fn default_thing(thing_id: u32, zone: i32) -> ThingState {
        ThingState {
            thing_id,
            zone,
            controller: 0,
            owner: 0,
            card_name: None,
            card_texture_number: None,
            tapped: false,
            attacking: false,
            blocking: false,
            power: 0,
            toughness: 0,
            damage: 0,
            summoning_sickness: false,
            face_down: false,
            attached_to_id: None,
            plus_counters: 0,
            minus_counters: 0,
            loyalty: 0,
            src_thing_id: None,
            from_zone: None,
            is_token: false,
        }
    }

    #[test]
    fn test_turn_and_phase_change() {
        let mut translator = ReplayTranslator::new();

        let s1 = make_state(1, 1, GamePhase::PreCombatMain);
        translator.process(&s1, false);

        let s2 = make_state(1, 2, GamePhase::Upkeep);
        let actions = translator.process(&s2, false);

        let types: Vec<_> = actions.iter().map(|a| &a.action_type).collect();
        assert!(types.iter().any(|a| matches!(a, ActionType::TurnChange { turn: 2, .. })));
        assert!(types
            .iter()
            .any(|a| matches!(a, ActionType::PhaseChange { phase } if phase == "upkeep")));
    }

    #[test]
    fn test_life_change() {
        let mut translator = ReplayTranslator::new();
        translator.set_player_names(vec!["Alice".into(), "Bob".into()]);

        let s1 = make_state(1, 1, GamePhase::PreCombatMain);
        translator.process(&s1, false);

        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        s2.players[1].life = 17;
        let actions = translator.process(&s2, false);

        assert_eq!(actions.len(), 1);
        match &actions[0].action_type {
            ActionType::LifeChange {
                player_id,
                old_life,
                new_life,
            } => {
                assert_eq!(player_id, "Bob");
                assert_eq!(*old_life, 20);
                assert_eq!(*new_life, 17);
            }
            other => panic!("Expected LifeChange, got {:?}", other),
        }
    }

    #[test]
    fn test_draw_card() {
        let mut translator = ReplayTranslator::new();

        let mut s1 = make_state(1, 1, GamePhase::Draw);
        s1.things.insert(10, default_thing(10, ZONE_LIBRARY));
        translator.process(&s1, false);

        let mut s2 = make_state(1, 1, GamePhase::Draw);
        let mut t = default_thing(10, ZONE_HAND);
        t.from_zone = None; // from_zone was cleared by process()
        s2.things.insert(10, t);
        // Simulate zone change: the old thing was in library, now in hand
        // We need the prev state to have zone=library
        let actions = translator.process(&s2, false);

        assert!(actions.iter().any(|a| matches!(
            &a.action_type,
            ActionType::DrawCard { card_id, .. } if card_id == "10"
        )));
    }

    #[test]
    fn test_play_land() {
        let mut translator = ReplayTranslator::new();

        let mut s1 = make_state(1, 1, GamePhase::PreCombatMain);
        s1.things.insert(20, default_thing(20, ZONE_HAND));
        translator.process(&s1, false);

        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        s2.things.insert(20, default_thing(20, ZONE_BATTLEFIELD));
        let actions = translator.process(&s2, false);

        assert!(actions.iter().any(|a| matches!(
            &a.action_type,
            ActionType::PlayLand { card_id, .. } if card_id == "20"
        )));
    }

    #[test]
    fn test_cast_spell_not_play_land() {
        let mut translator = ReplayTranslator::new();

        // Step 1: thing in hand
        let mut s1 = make_state(1, 1, GamePhase::PreCombatMain);
        s1.things.insert(30, default_thing(30, ZONE_HAND));
        translator.process(&s1, false);

        // Step 2: thing goes to stack (cast spell)
        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        s2.things.insert(30, default_thing(30, ZONE_STACK));
        translator.process(&s2, false);

        // Step 3: thing resolves to battlefield
        let mut s3 = make_state(1, 1, GamePhase::PreCombatMain);
        s3.things.insert(30, default_thing(30, ZONE_BATTLEFIELD));
        let actions = translator.process(&s3, false);

        // Should be ZoneTransition (stack→battlefield), NOT PlayLand
        assert!(actions.iter().any(|a| matches!(
            &a.action_type,
            ActionType::ZoneTransition { from_zone, to_zone, .. }
                if from_zone == "stack" && to_zone == "battlefield"
        )));
        assert!(!actions
            .iter()
            .any(|a| matches!(&a.action_type, ActionType::PlayLand { .. })));
    }

    #[test]
    fn test_tap_untap() {
        let mut translator = ReplayTranslator::new();

        let mut s1 = make_state(1, 1, GamePhase::PreCombatMain);
        s1.things.insert(50, default_thing(50, ZONE_BATTLEFIELD));
        translator.process(&s1, false);

        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        let mut t = default_thing(50, ZONE_BATTLEFIELD);
        t.tapped = true;
        s2.things.insert(50, t);
        let actions = translator.process(&s2, false);

        assert!(actions
            .iter()
            .any(|a| matches!(&a.action_type, ActionType::TapPermanent { card_id } if card_id == "50")));
    }

    #[test]
    fn test_counter_update() {
        let mut translator = ReplayTranslator::new();

        let mut s1 = make_state(1, 1, GamePhase::PreCombatMain);
        s1.things.insert(60, default_thing(60, ZONE_BATTLEFIELD));
        translator.process(&s1, false);

        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        let mut t = default_thing(60, ZONE_BATTLEFIELD);
        t.plus_counters = 3;
        s2.things.insert(60, t);
        let actions = translator.process(&s2, false);

        assert!(actions.iter().any(|a| matches!(
            &a.action_type,
            ActionType::CounterUpdate { card_id, counter_type, count }
                if card_id == "60" && counter_type == "+1/+1" && *count == 3
        )));
    }

    #[test]
    fn test_reset() {
        let mut translator = ReplayTranslator::new();
        translator.set_player_names(vec!["A".into()]);
        translator.set_start_time(Utc::now());

        let s1 = make_state(1, 1, GamePhase::Draw);
        translator.process(&s1, false);

        translator.reset();

        assert!(translator.prev.is_none());
        assert_eq!(translator.player_names, vec!["A".to_string()]); // preserved across reset
        assert!(translator.start_time.is_none());
        assert!(translator.things_seen_on_stack.is_empty());
    }

    #[test]
    fn test_full_state_suppresses_new_things_but_keeps_phase() {
        let mut translator = ReplayTranslator::new();

        let s1 = make_state(1, 1, GamePhase::PreCombatMain);
        translator.process(&s1, false);

        // Full state with new things and a phase change — new things suppressed,
        // but phase change and existing-thing diffs still emitted.
        let mut s2 = make_state(1, 1, GamePhase::PostCombatMain);
        s2.things.insert(100, default_thing(100, ZONE_BATTLEFIELD));
        s2.things.insert(101, default_thing(101, ZONE_HAND));
        let actions = translator.process(&s2, true);
        // Phase change IS emitted
        assert!(actions.iter().any(|a| matches!(
            &a.action_type,
            ActionType::PhaseChange { phase } if phase == "postcombat_main"
        )));
        // But new things are NOT emitted as DrawCard/PlayLand
        assert!(!actions.iter().any(|a| matches!(&a.action_type, ActionType::PlayLand { .. })));
        assert!(!actions.iter().any(|a| matches!(&a.action_type, ActionType::DrawCard { .. })));
    }

    /// New things appearing on the battlefield should NOT produce PlayLand.
    /// They are typically the opponent's existing permanents becoming visible
    /// for the first time, not actual land plays.
    #[test]
    fn test_new_thing_on_battlefield_does_not_produce_play_land() {
        let mut translator = ReplayTranslator::new();

        let s1 = make_state(1, 1, GamePhase::PreCombatMain);
        translator.process(&s1, false);

        // A batch of new things appear on battlefield (opponent board revealed)
        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        s2.things.insert(200, default_thing(200, ZONE_BATTLEFIELD));
        s2.things.insert(201, default_thing(201, ZONE_BATTLEFIELD));
        s2.things.insert(202, default_thing(202, ZONE_BATTLEFIELD));
        let actions = translator.process(&s2, false);

        assert!(
            !actions.iter().any(|a| matches!(&a.action_type, ActionType::PlayLand { .. })),
            "New things on battlefield should not generate PlayLand, got: {:?}",
            actions.iter().map(|a| &a.action_type).collect::<Vec<_>>()
        );
        assert!(
            !actions.iter().any(|a| matches!(&a.action_type, ActionType::ZoneTransition { .. })),
            "New things on battlefield should not generate ZoneTransition"
        );
    }

    /// A real land play (hand → battlefield for an existing thing) should
    /// still produce PlayLand.
    #[test]
    fn test_existing_thing_hand_to_battlefield_still_produces_play_land() {
        let mut translator = ReplayTranslator::new();

        let mut s1 = make_state(1, 1, GamePhase::PreCombatMain);
        s1.things.insert(300, default_thing(300, ZONE_HAND));
        translator.process(&s1, false);

        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        s2.things.insert(300, default_thing(300, ZONE_BATTLEFIELD));
        let actions = translator.process(&s2, false);

        assert!(
            actions.iter().any(|a| matches!(
                &a.action_type,
                ActionType::PlayLand { card_id, .. } if card_id == "300"
            )),
            "Hand→battlefield zone change should still produce PlayLand"
        );
    }

    /// Duplicate PhaseChange events are suppressed when interleaved MTGO state
    /// updates cause the same phase transition to appear multiple times.
    #[test]
    fn test_duplicate_phase_change_suppressed() {
        let mut translator = ReplayTranslator::new();

        // Initial state
        let s1 = make_state(1, 1, GamePhase::Upkeep);
        translator.process(&s1, false);

        // State update advances to precombat_main — should emit PhaseChange
        let s2 = make_state(1, 1, GamePhase::PreCombatMain);
        let actions = translator.process(&s2, false);
        assert!(
            actions.iter().any(|a| matches!(&a.action_type, ActionType::PhaseChange { phase } if phase == "precombat_main")),
            "First precombat_main should emit PhaseChange"
        );

        // Another state update still at precombat_main — should NOT emit PhaseChange
        let mut s3 = make_state(1, 1, GamePhase::PreCombatMain);
        s3.players[0].life = 18; // some other change to make the state different
        let actions = translator.process(&s3, false);
        assert!(
            !actions.iter().any(|a| matches!(&a.action_type, ActionType::PhaseChange { .. })),
            "Duplicate precombat_main should not emit PhaseChange, got: {:?}",
            actions.iter().map(|a| &a.action_type).collect::<Vec<_>>()
        );
        // But the life change should still be emitted
        assert!(
            actions.iter().any(|a| matches!(&a.action_type, ActionType::LifeChange { .. })),
            "Life change should still be emitted"
        );
    }

    /// Phase regressions suppress phase/life metadata but still process
    /// thing diffs (zone changes, taps, etc.) so real game actions aren't lost.
    #[test]
    fn test_phase_regression_suppresses_phase_but_keeps_thing_diffs() {
        let mut translator = ReplayTranslator::new();

        let mut s1 = make_state(1, 1, GamePhase::PostCombatMain);
        s1.things.insert(400, default_thing(400, ZONE_BATTLEFIELD));
        translator.process(&s1, false);

        // State update that regresses the phase (postcombat_main → upkeep).
        // Phase/turn/life actions suppressed, but thing diffs still processed.
        let mut s2 = make_state(1, 1, GamePhase::Upkeep);
        let mut t = default_thing(400, ZONE_BATTLEFIELD);
        t.tapped = true;
        s2.things.insert(400, t);
        let actions = translator.process(&s2, false);

        // No PhaseChange emitted (phase went backward)
        assert!(
            !actions.iter().any(|a| matches!(&a.action_type, ActionType::PhaseChange { .. })),
            "Phase regression should not emit PhaseChange"
        );
        // But the tap IS emitted (thing diff still runs)
        assert!(
            actions.iter().any(|a| matches!(&a.action_type, ActionType::TapPermanent { .. })),
            "Thing diffs should still be processed during phase regression"
        );
    }

    #[test]
    fn test_chat_discard() {
        let mut translator = ReplayTranslator::new();
        translator.set_player_names(vec!["Alice".into(), "Bob".into()]);

        // Initial state: no card 421 visible (it's in hand but hand contents
        // aren't always in the state — cards appear as new things when discarded)
        let s1 = make_state(1, 1, GamePhase::PreCombatMain);
        translator.process(&s1, false);

        // Chat arrives before state diff
        translator.ingest_chat("Alice discards @[Bolt@:100,421:@].");

        // State: card 421 appears in graveyard as a new thing
        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        let mut t = default_thing(421, 3); // graveyard
        t.from_zone = Some(-1);
        s2.things.insert(421, t);
        let actions = translator.process(&s2, false);

        assert!(actions.iter().any(|a| matches!(
            &a.action_type,
            ActionType::Discard { player_id, card_id }
                if player_id == "Alice" && card_id == "421"
        )), "Expected Discard action, got: {:?}", actions.iter().map(|a| &a.action_type).collect::<Vec<_>>());
    }

    #[test]
    fn test_chat_mill() {
        let mut translator = ReplayTranslator::new();
        translator.set_player_names(vec!["Alice".into(), "Bob".into()]);

        let s1 = make_state(1, 1, GamePhase::PreCombatMain);
        translator.process(&s1, false);

        // Chat: surveil puts card into graveyard (card never seen → library source)
        translator.ingest_chat("Alice puts @[Bolt@:100,283:@] into their graveyard.");

        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        let mut t = default_thing(283, 3); // graveyard
        t.from_zone = Some(-1);
        s2.things.insert(283, t);
        let actions = translator.process(&s2, false);

        assert!(actions.iter().any(|a| matches!(
            &a.action_type,
            ActionType::Mill { player_id, card_id }
                if player_id == "Alice" && card_id == "283"
        )), "Expected Mill action, got: {:?}", actions.iter().map(|a| &a.action_type).collect::<Vec<_>>());
    }

    #[test]
    fn test_chat_create_token() {
        let mut translator = ReplayTranslator::new();
        translator.set_player_names(vec!["Alice".into(), "Bob".into()]);

        let s1 = make_state(1, 1, GamePhase::PreCombatMain);
        translator.process(&s1, false);

        translator.ingest_chat("Alice's @[Fable@:194420,458:@] creates a Goblin Shaman Token.");

        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        let mut t = default_thing(461, ZONE_BATTLEFIELD);
        t.card_name = Some("Goblin Shaman Token".to_string());
        t.is_token = true;
        t.from_zone = Some(-1);
        s2.things.insert(461, t);
        let actions = translator.process(&s2, false);

        assert!(actions.iter().any(|a| matches!(
            &a.action_type,
            ActionType::CreateToken { player_id, token_name, .. }
                if player_id == "Alice" && token_name == "Goblin Shaman Token"
        )), "Expected CreateToken action, got: {:?}", actions.iter().map(|a| &a.action_type).collect::<Vec<_>>());
    }

    #[test]
    fn test_chat_resolves_unknown_from_zone() {
        let mut translator = ReplayTranslator::new();
        translator.set_player_names(vec!["Alice".into(), "Bob".into()]);

        // Card 510 on battlefield — tracked by last_known_zones
        let mut s1 = make_state(1, 1, GamePhase::PreCombatMain);
        s1.things.insert(510, default_thing(510, ZONE_BATTLEFIELD));
        translator.process(&s1, false);

        // Full state prune removes card 510 (but last_known_zones retains it)
        let s2 = make_state(1, 1, GamePhase::PreCombatMain);
        translator.process(&s2, true);

        // Chat: exile
        translator.ingest_chat("Alice exiles @[Subtlety@:181014,510:@].");

        // Card reappears in exile as NEW thing (not in prev)
        let mut s3 = make_state(1, 1, GamePhase::PreCombatMain);
        let mut t = default_thing(510, 4); // exile
        t.from_zone = Some(-1);
        s3.things.insert(510, t);
        let actions = translator.process(&s3, false);

        assert!(actions.iter().any(|a| matches!(
            &a.action_type,
            ActionType::ZoneTransition { from_zone, to_zone, .. }
                if from_zone == "battlefield" && to_zone == "exile"
        )), "Expected from_zone=battlefield, got: {:?}", actions.iter().map(|a| &a.action_type).collect::<Vec<_>>());
    }

    #[test]
    fn test_last_known_zones_fallback() {
        let mut translator = ReplayTranslator::new();
        translator.set_player_names(vec!["Alice".into(), "Bob".into()]);

        // Card 600 on battlefield
        let mut s1 = make_state(1, 1, GamePhase::PreCombatMain);
        s1.things.insert(600, default_thing(600, ZONE_BATTLEFIELD));
        translator.process(&s1, false);

        // Full state prune
        let s2 = make_state(1, 1, GamePhase::PreCombatMain);
        translator.process(&s2, true);

        // Card 600 reappears in graveyard with NO chat context
        let mut s3 = make_state(1, 1, GamePhase::PreCombatMain);
        let mut t = default_thing(600, 3); // graveyard
        t.from_zone = Some(-1);
        s3.things.insert(600, t);
        let actions = translator.process(&s3, false);

        assert!(actions.iter().any(|a| matches!(
            &a.action_type,
            ActionType::ZoneTransition { from_zone, to_zone, .. }
                if from_zone == "battlefield" && to_zone == "graveyard"
        )), "Expected from_zone=battlefield via last_known_zones, got: {:?}", actions.iter().map(|a| &a.action_type).collect::<Vec<_>>());
    }
}
