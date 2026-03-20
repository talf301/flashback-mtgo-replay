//! State-diff to ReplayAction translation.
//!
//! Compares consecutive `GameState` snapshots and emits `ReplayAction`s
//! describing what changed between them.

use std::collections::HashSet;

use chrono::{DateTime, Utc};

use crate::replay::schema::{ActionType, ReplayAction};
use crate::state::GameState;

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
}

impl ReplayTranslator {
    pub fn new() -> Self {
        ReplayTranslator {
            prev: None,
            player_names: Vec::new(),
            start_time: None,
            things_seen_on_stack: HashSet::new(),
        }
    }

    pub fn set_player_names(&mut self, names: Vec<String>) {
        self.player_names = names;
    }

    pub fn set_start_time(&mut self, t: DateTime<Utc>) {
        self.start_time = Some(t);
    }

    /// Reset translator state for a new game in a multi-game session.
    pub fn reset(&mut self) {
        self.prev = None;
        self.start_time = None;
        self.player_names.clear();
        self.things_seen_on_stack.clear();
    }

    /// Process a new game state, emitting actions for everything that changed.
    pub fn process(&mut self, new_state: &GameState) -> Vec<ReplayAction> {
        // Record stack occupants BEFORE evaluating diffs
        for (thing_id, thing) in &new_state.things {
            if thing.zone == ZONE_STACK {
                self.things_seen_on_stack.insert(*thing_id);
            }
        }

        let actions = if let Some(ref prev) = self.prev {
            self.diff(prev, new_state)
        } else {
            // First state — no diff possible
            Vec::new()
        };

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

    fn diff(&self, prev: &GameState, new: &GameState) -> Vec<ReplayAction> {
        let mut actions = Vec::new();

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

        // Phase change
        if new.phase != prev.phase {
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
                // New thing — appeared in this state for the first time.
                // ThingElement.from_zone is a zone object reference (not a CardZone enum),
                // so we rely on the current zone to infer what happened.
                let zone = new_thing.zone;

                if zone == ZONE_HAND {
                    // New thing in hand → draw
                    actions.push(self.make_action(
                        new,
                        ActionType::DrawCard {
                            player_id: self.player_name(new_thing.controller as usize),
                            card_id: card_id.clone(),
                        },
                    ));
                } else if zone == ZONE_STACK {
                    // New thing on stack → cast or activate
                    if let Some(src_id) = new_thing.src_thing_id {
                        if new.things.get(&src_id).map_or(false, |src| {
                            src.zone == ZONE_BATTLEFIELD
                        }) {
                            actions.push(self.make_action(
                                new,
                                ActionType::ActivateAbility {
                                    player_id: self
                                        .player_name(new_thing.controller as usize),
                                    card_id: src_id.to_string(),
                                    ability_id: card_id.clone(),
                                },
                            ));
                        } else {
                            actions.push(self.make_action(
                                new,
                                ActionType::CastSpell {
                                    player_id: self
                                        .player_name(new_thing.controller as usize),
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
                } else if zone == ZONE_BATTLEFIELD
                    && !self.things_seen_on_stack.contains(thing_id)
                {
                    // New thing on battlefield, never seen on stack → play land
                    actions.push(self.make_action(
                        new,
                        ActionType::PlayLand {
                            player_id: self.player_name(new_thing.controller as usize),
                            card_id: card_id.clone(),
                        },
                    ));
                } else if zone == ZONE_BATTLEFIELD {
                    // New thing on battlefield, was on stack → resolved spell
                    actions.push(self.make_action(
                        new,
                        ActionType::ZoneTransition {
                            card_id: card_id.clone(),
                            from_zone: "stack".to_string(),
                            to_zone: "battlefield".to_string(),
                            player_id: Some(
                                self.player_name(new_thing.controller as usize),
                            ),
                        },
                    ));
                }
                // Other zones (graveyard, exile, etc.) for new things are silently skipped
                // as they typically represent game setup or tokens being created
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
        }
    }

    #[test]
    fn test_turn_and_phase_change() {
        let mut translator = ReplayTranslator::new();

        let s1 = make_state(1, 1, GamePhase::PreCombatMain);
        translator.process(&s1);

        let s2 = make_state(1, 2, GamePhase::Upkeep);
        let actions = translator.process(&s2);

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
        translator.process(&s1);

        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        s2.players[1].life = 17;
        let actions = translator.process(&s2);

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
        translator.process(&s1);

        let mut s2 = make_state(1, 1, GamePhase::Draw);
        let mut t = default_thing(10, ZONE_HAND);
        t.from_zone = None; // from_zone was cleared by process()
        s2.things.insert(10, t);
        // Simulate zone change: the old thing was in library, now in hand
        // We need the prev state to have zone=library
        let actions = translator.process(&s2);

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
        translator.process(&s1);

        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        s2.things.insert(20, default_thing(20, ZONE_BATTLEFIELD));
        let actions = translator.process(&s2);

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
        translator.process(&s1);

        // Step 2: thing goes to stack (cast spell)
        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        s2.things.insert(30, default_thing(30, ZONE_STACK));
        translator.process(&s2);

        // Step 3: thing resolves to battlefield
        let mut s3 = make_state(1, 1, GamePhase::PreCombatMain);
        s3.things.insert(30, default_thing(30, ZONE_BATTLEFIELD));
        let actions = translator.process(&s3);

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
        translator.process(&s1);

        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        let mut t = default_thing(50, ZONE_BATTLEFIELD);
        t.tapped = true;
        s2.things.insert(50, t);
        let actions = translator.process(&s2);

        assert!(actions
            .iter()
            .any(|a| matches!(&a.action_type, ActionType::TapPermanent { card_id } if card_id == "50")));
    }

    #[test]
    fn test_counter_update() {
        let mut translator = ReplayTranslator::new();

        let mut s1 = make_state(1, 1, GamePhase::PreCombatMain);
        s1.things.insert(60, default_thing(60, ZONE_BATTLEFIELD));
        translator.process(&s1);

        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        let mut t = default_thing(60, ZONE_BATTLEFIELD);
        t.plus_counters = 3;
        s2.things.insert(60, t);
        let actions = translator.process(&s2);

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
        translator.process(&s1);

        translator.reset();

        assert!(translator.prev.is_none());
        assert!(translator.player_names.is_empty());
        assert!(translator.start_time.is_none());
        assert!(translator.things_seen_on_stack.is_empty());
    }
}
