//! Game state board model.

use std::collections::{HashMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::protocol::opcodes;
use crate::protocol::statebuf::{PropertyValue, StateElement};

// ============================================================
// GamePhase
// ============================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GamePhase {
    Untap,
    Upkeep,
    Draw,
    PreCombatMain,
    BeginCombat,
    DeclareAttackers,
    DeclareBlockers,
    FirstStrikeDamage,
    CombatDamage,
    EndOfCombat,
    PostCombatMain,
    EndStep,
    Cleanup,
    Unknown(u8),
}

impl GamePhase {
    pub fn from_u8(v: u8) -> GamePhase {
        match v {
            0 => GamePhase::Untap,
            1 => GamePhase::Upkeep,
            2 => GamePhase::Draw,
            3 => GamePhase::PreCombatMain,
            4 => GamePhase::BeginCombat,
            5 => GamePhase::DeclareAttackers,
            6 => GamePhase::DeclareBlockers,
            7 => GamePhase::FirstStrikeDamage,
            8 => GamePhase::CombatDamage,
            9 => GamePhase::EndOfCombat,
            10 => GamePhase::PostCombatMain,
            11 => GamePhase::EndStep,
            12 => GamePhase::Cleanup,
            other => GamePhase::Unknown(other),
        }
    }
}

impl fmt::Display for GamePhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GamePhase::Untap => write!(f, "untap"),
            GamePhase::Upkeep => write!(f, "upkeep"),
            GamePhase::Draw => write!(f, "draw"),
            GamePhase::PreCombatMain => write!(f, "precombat_main"),
            GamePhase::BeginCombat => write!(f, "begin_combat"),
            GamePhase::DeclareAttackers => write!(f, "declare_attackers"),
            GamePhase::DeclareBlockers => write!(f, "declare_blockers"),
            GamePhase::FirstStrikeDamage => write!(f, "first_strike_damage"),
            GamePhase::CombatDamage => write!(f, "combat_damage"),
            GamePhase::EndOfCombat => write!(f, "end_of_combat"),
            GamePhase::PostCombatMain => write!(f, "postcombat_main"),
            GamePhase::EndStep => write!(f, "end_step"),
            GamePhase::Cleanup => write!(f, "cleanup"),
            GamePhase::Unknown(v) => write!(f, "unknown({})", v),
        }
    }
}

// ============================================================
// PlayerState
// ============================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerState {
    pub life: i32,
    pub hand_count: i32,
    pub library_count: i32,
    pub graveyard_count: i32,
}

impl Default for PlayerState {
    fn default() -> Self {
        PlayerState {
            life: 0,
            hand_count: 0,
            library_count: 0,
            graveyard_count: 0,
        }
    }
}

// ============================================================
// ThingState
// ============================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThingState {
    pub thing_id: u32,
    pub zone: i32,
    pub controller: i32,
    pub owner: i32,
    pub card_name: Option<String>,
    pub tapped: bool,
    pub attacking: bool,
    pub blocking: bool,
    pub power: i32,
    pub toughness: i32,
    pub damage: i32,
    pub summoning_sickness: bool,
    pub face_down: bool,
    pub attached_to_id: Option<u32>,
    pub plus_counters: i32,
    pub minus_counters: i32,
    pub loyalty: i32,
    pub src_thing_id: Option<u32>,
    pub from_zone: Option<i32>,
}

impl Default for ThingState {
    fn default() -> Self {
        ThingState {
            thing_id: 0,
            zone: 0,
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
}

// ============================================================
// GameState
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub game_id: u32,
    pub turn: i32,
    pub phase: GamePhase,
    pub active_player: usize,
    pub players: Vec<PlayerState>,
    pub things: HashMap<u32, ThingState>,
}

impl GameState {
    pub fn new(game_id: u32) -> Self {
        GameState {
            game_id,
            turn: 0,
            phase: GamePhase::Unknown(255),
            active_player: 0,
            players: Vec::new(),
            things: HashMap::new(),
        }
    }

    pub fn apply_elements(&mut self, elements: &[StateElement], is_full_state: bool) {
        let mut seen_thing_ids: HashSet<u32> = HashSet::new();

        for element in elements {
            match element {
                StateElement::PlayerStatus(ps) => {
                    // Extend players vec if needed
                    let count = ps.life.len();
                    while self.players.len() < count {
                        self.players.push(PlayerState::default());
                    }
                    for i in 0..count {
                        if i < self.players.len() {
                            self.players[i].life = ps.life[i] as i32;
                            if i < ps.hand_count.len() {
                                self.players[i].hand_count = ps.hand_count[i] as i32;
                            }
                            if i < ps.library_count.len() {
                                self.players[i].library_count = ps.library_count[i] as i32;
                            }
                            if i < ps.graveyard_count.len() {
                                self.players[i].graveyard_count = ps.graveyard_count[i] as i32;
                            }
                        }
                    }
                    self.active_player = ps.active_player as usize;
                }
                StateElement::TurnStep(ts) => {
                    self.turn = ts.turn_number;
                    self.phase = GamePhase::from_u8(ts.phase);
                }
                StateElement::Thing(te) => {
                    // Extract thing_id from THINGNUMBER property
                    let thing_id = match te.props.get(&opcodes::THINGNUMBER) {
                        Some(PropertyValue::Int(v)) => *v as u32,
                        _ => {
                            tracing::warn!("Thing element without THINGNUMBER property, skipping");
                            continue;
                        }
                    };

                    seen_thing_ids.insert(thing_id);

                    let is_existing = self.things.contains_key(&thing_id);

                    let thing = self.things.entry(thing_id).or_insert_with(|| {
                        let mut t = ThingState::default();
                        t.thing_id = thing_id;
                        t
                    });

                    // Track zone changes for from_zone
                    let old_zone = thing.zone;

                    // Apply properties
                    for (key, value) in &te.props {
                        match *key {
                            opcodes::ZONE => {
                                if let PropertyValue::Int(v) = value {
                                    thing.zone = *v;
                                }
                            }
                            opcodes::CONTROLLER => {
                                if let PropertyValue::Int(v) = value {
                                    thing.controller = *v;
                                }
                            }
                            opcodes::OWNER => {
                                if let PropertyValue::Int(v) = value {
                                    thing.owner = *v;
                                }
                            }
                            opcodes::CARDNAME_STRING => {
                                if let PropertyValue::Str(s) = value {
                                    thing.card_name = Some(s.clone());
                                }
                            }
                            opcodes::TAPPED => {
                                if let PropertyValue::Int(v) = value {
                                    thing.tapped = *v != 0;
                                }
                            }
                            opcodes::ATTACKING => {
                                if let PropertyValue::Int(v) = value {
                                    thing.attacking = *v != 0;
                                }
                            }
                            opcodes::BLOCKING => {
                                if let PropertyValue::Int(v) = value {
                                    thing.blocking = *v != 0;
                                }
                            }
                            opcodes::POWER => {
                                if let PropertyValue::Int(v) = value {
                                    thing.power = *v;
                                }
                            }
                            opcodes::TOUGHNESS => {
                                if let PropertyValue::Int(v) = value {
                                    thing.toughness = *v;
                                }
                            }
                            opcodes::DAMAGE => {
                                if let PropertyValue::Int(v) = value {
                                    thing.damage = *v;
                                }
                            }
                            opcodes::SUMMONING_SICK => {
                                if let PropertyValue::Int(v) = value {
                                    thing.summoning_sickness = *v != 0;
                                }
                            }
                            opcodes::FACE_DOWN => {
                                if let PropertyValue::Int(v) = value {
                                    thing.face_down = *v != 0;
                                }
                            }
                            opcodes::ATTACHED_TO_ID => {
                                if let PropertyValue::Int(v) = value {
                                    thing.attached_to_id = if *v != 0 { Some(*v as u32) } else { None };
                                }
                            }
                            opcodes::PLUS_ONE_PLUS_ONE_COUNTERS => {
                                if let PropertyValue::Int(v) = value {
                                    thing.plus_counters = *v;
                                }
                            }
                            opcodes::MINUS_ONE_MINUS_ONE_COUNTERS => {
                                if let PropertyValue::Int(v) = value {
                                    thing.minus_counters = *v;
                                }
                            }
                            opcodes::LOYALTY_COUNTERS => {
                                if let PropertyValue::Int(v) = value {
                                    thing.loyalty = *v;
                                }
                            }
                            opcodes::SRC_THING_ID => {
                                if let PropertyValue::Int(v) = value {
                                    thing.src_thing_id = if *v != 0 { Some(*v as u32) } else { None };
                                }
                            }
                            _ => {
                                // Ignore other properties
                            }
                        }
                    }

                    // Track from_zone: if existing thing and zone changed
                    if is_existing && thing.zone != old_zone {
                        thing.from_zone = Some(old_zone);
                    }
                    // New things: from_zone stays None (set by default)
                }
                StateElement::Other { .. } => {
                    // Silently skip
                }
            }
        }

        // Full-state pruning
        if is_full_state {
            self.things.retain(|id, _| seen_thing_ids.contains(id));
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::statebuf::{
        PlayerStatusElement, PropertyValue, StateElement, ThingElement,
    };
    use std::collections::HashMap;

    #[test]
    fn test_apply_player_status() {
        let mut state = GameState::new(1);

        let ps = StateElement::PlayerStatus(PlayerStatusElement {
            life: vec![20, 18],
            hand_count: vec![7, 6],
            library_count: vec![53, 52],
            graveyard_count: vec![0, 2],
            time_left: vec![1500, 1400],
            active_player: 1,
        });

        state.apply_elements(&[ps], false);

        assert_eq!(state.players.len(), 2);
        assert_eq!(state.players[0].life, 20);
        assert_eq!(state.players[1].life, 18);
        assert_eq!(state.players[0].hand_count, 7);
        assert_eq!(state.players[1].hand_count, 6);
        assert_eq!(state.players[0].library_count, 53);
        assert_eq!(state.players[1].library_count, 52);
        assert_eq!(state.players[0].graveyard_count, 0);
        assert_eq!(state.players[1].graveyard_count, 2);
        assert_eq!(state.active_player, 1);
    }

    #[test]
    fn test_apply_thing_new() {
        let mut state = GameState::new(1);

        let mut props = HashMap::new();
        props.insert(opcodes::THINGNUMBER, PropertyValue::Int(42));
        props.insert(opcodes::ZONE, PropertyValue::Int(3));
        props.insert(opcodes::CONTROLLER, PropertyValue::Int(1));
        props.insert(opcodes::OWNER, PropertyValue::Int(1));
        props.insert(
            opcodes::CARDNAME_STRING,
            PropertyValue::Str("Lightning Bolt".to_string()),
        );
        props.insert(opcodes::POWER, PropertyValue::Int(0));
        props.insert(opcodes::TOUGHNESS, PropertyValue::Int(0));
        props.insert(opcodes::TAPPED, PropertyValue::Int(0));

        let te = StateElement::Thing(ThingElement { from_zone: 0, props });

        state.apply_elements(&[te], false);

        assert_eq!(state.things.len(), 1);
        let thing = state.things.get(&42).unwrap();
        assert_eq!(thing.thing_id, 42);
        assert_eq!(thing.zone, 3);
        assert_eq!(thing.controller, 1);
        assert_eq!(thing.owner, 1);
        assert_eq!(thing.card_name, Some("Lightning Bolt".to_string()));
        assert!(!thing.tapped);
        assert_eq!(thing.from_zone, None);
    }

    #[test]
    fn test_apply_thing_update() {
        let mut state = GameState::new(1);

        // Insert initial thing
        let mut props1 = HashMap::new();
        props1.insert(opcodes::THINGNUMBER, PropertyValue::Int(10));
        props1.insert(opcodes::ZONE, PropertyValue::Int(1)); // library
        props1.insert(opcodes::CONTROLLER, PropertyValue::Int(0));
        props1.insert(
            opcodes::CARDNAME_STRING,
            PropertyValue::Str("Grizzly Bears".to_string()),
        );
        props1.insert(opcodes::POWER, PropertyValue::Int(2));
        props1.insert(opcodes::TOUGHNESS, PropertyValue::Int(2));

        let te1 = StateElement::Thing(ThingElement {
            from_zone: 0,
            props: props1,
        });
        state.apply_elements(&[te1], false);

        // Update: move to battlefield (zone 5), tap it
        let mut props2 = HashMap::new();
        props2.insert(opcodes::THINGNUMBER, PropertyValue::Int(10));
        props2.insert(opcodes::ZONE, PropertyValue::Int(5)); // battlefield
        props2.insert(opcodes::TAPPED, PropertyValue::Int(1));

        let te2 = StateElement::Thing(ThingElement {
            from_zone: 1,
            props: props2,
        });
        state.apply_elements(&[te2], false);

        let thing = state.things.get(&10).unwrap();
        assert_eq!(thing.zone, 5);
        assert!(thing.tapped);
        // from_zone should track the old zone
        assert_eq!(thing.from_zone, Some(1));
        // card_name should be preserved from the first apply
        assert_eq!(thing.card_name, Some("Grizzly Bears".to_string()));
        // power/toughness preserved
        assert_eq!(thing.power, 2);
        assert_eq!(thing.toughness, 2);
    }

    #[test]
    fn test_full_state_pruning() {
        let mut state = GameState::new(1);

        // Insert two things
        for id in [100, 200] {
            let mut props = HashMap::new();
            props.insert(opcodes::THINGNUMBER, PropertyValue::Int(id));
            props.insert(opcodes::ZONE, PropertyValue::Int(1));
            let te = StateElement::Thing(ThingElement { from_zone: 0, props });
            state.apply_elements(&[te], false);
        }

        assert_eq!(state.things.len(), 2);

        // Full-state update that only includes thing 100
        let mut props = HashMap::new();
        props.insert(opcodes::THINGNUMBER, PropertyValue::Int(100));
        props.insert(opcodes::ZONE, PropertyValue::Int(1));
        let te = StateElement::Thing(ThingElement { from_zone: 0, props });
        state.apply_elements(&[te], true);

        // Thing 200 should be pruned
        assert_eq!(state.things.len(), 1);
        assert!(state.things.contains_key(&100));
        assert!(!state.things.contains_key(&200));
    }
}
