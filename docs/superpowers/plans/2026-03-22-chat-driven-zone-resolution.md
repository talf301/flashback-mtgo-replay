# Chat-Driven Zone Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Parse MTGO chat log messages to resolve unknown `from_zone` values in zone transitions and emit new action types (Discard, Mill, CreateToken).

**Architecture:** New `src/chat.rs` module parses chat text into `ChatEvent` structs. The existing `ReplayTranslator` gains `ingest_chat()` to feed chat context, which is consumed during state diff processing to resolve `from_zone` and classify actions. `ThingState` gains an `is_token` field. Web viewer types and reconstructor are updated for the three new `ActionType` variants.

**Tech Stack:** Rust (string parsing, no new dependencies), TypeScript/Vitest (web viewer)

**Spec:** `docs/superpowers/specs/2026-03-22-chat-driven-zone-resolution-design.md`

---

### Task 1: Add new ActionType variants to Rust schema

**Files:**
- Modify: `src/replay/schema.rs:22-52` (ActionType enum)
- Modify: `src/replay/schema.rs:201-334` (test_action_types test)

- [ ] **Step 1: Add Discard, Mill, CreateToken variants to ActionType enum**

In `src/replay/schema.rs`, add three variants after the `ZoneTransition` variant (line ~37):

```rust
    Discard { player_id: String, card_id: String },
    Mill { player_id: String, card_id: String },
    CreateToken { player_id: String, card_id: String, token_name: String },
```

- [ ] **Step 2: Add serialization test cases for new variants**

In `src/replay/schema.rs`, add to the `test_action_types` test's `actions` vec:

```rust
            ActionType::Discard {
                player_id: "p1".to_string(),
                card_id: "c1".to_string(),
            },
            ActionType::Mill {
                player_id: "p1".to_string(),
                card_id: "c1".to_string(),
            },
            ActionType::CreateToken {
                player_id: "p1".to_string(),
                card_id: "c1".to_string(),
                token_name: "Goblin Token".to_string(),
            },
```

- [ ] **Step 3: Run tests to verify serialization roundtrip**

Run: `cargo test --lib replay::schema`
Expected: All tests pass including the new variants.

- [ ] **Step 4: Add display arms for new variants in decode.rs**

In `src/bin/decode.rs`, in the `match &action.action_type` block (around line 694), add arms for the three new variants. Place them before the `ZoneTransition` arm:

```rust
                ActionType::Discard {
                    player_id,
                    card_id,
                } => {
                    println!(
                        "    {} discards {}",
                        player_id,
                        card_label(card_id, names)
                    );
                }
                ActionType::Mill {
                    player_id,
                    card_id,
                } => {
                    println!(
                        "    {} mills {}",
                        player_id,
                        card_label(card_id, names)
                    );
                }
                ActionType::CreateToken {
                    player_id,
                    card_id,
                    token_name,
                } => {
                    println!(
                        "    {} creates {} ({})",
                        player_id,
                        token_name,
                        card_label(card_id, names)
                    );
                }
```

- [ ] **Step 5: Verify decode.rs compiles**

Run: `cargo build --bin decode`
Expected: Compiles without errors.

- [ ] **Step 6: Commit**

```bash
git add src/replay/schema.rs src/bin/decode.rs
git commit -m "feat: add Discard, Mill, CreateToken action type variants"
```

---

### Task 2: Create chat parser module

**Files:**
- Create: `src/chat.rs`
- Modify: `src/lib.rs` (add `pub mod chat;`)

- [ ] **Step 1: Write failing tests for chat parser**

Create `src/chat.rs` with the types and test module but no implementation:

```rust
//! MTGO chat log parser.
//!
//! Parses structured game log messages from `NEW_USER_CHAT` into `ChatEvent`s
//! that the translator uses to resolve unknown zone transitions and classify actions.

/// A card reference extracted from chat text in `@[Name@:textureId,thingId:@]` format.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatCardRef {
    pub name: String,
    pub texture_id: u32,
    pub thing_id: u32,
}

/// Position on library (top or bottom).
#[derive(Debug, Clone, PartialEq)]
pub enum LibPos {
    Top,
    Bottom,
}

/// A structured event parsed from an MTGO chat message.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatEvent {
    Discard { player: String, card: ChatCardRef },
    PutIntoGraveyard { player: String, card: ChatCardRef },
    Exile { player: String, card: ChatCardRef },
    PutOnLibrary { player: String, card: ChatCardRef, position: LibPos },
    CreateToken { player: String, source_card: ChatCardRef, token_name: String },
}

/// Extract all card references from a chat message.
/// Format: `@[CardName@:textureId,thingId:@]`
pub fn extract_card_refs(text: &str) -> Vec<ChatCardRef> {
    todo!()
}

/// Parse a chat message into a ChatEvent, if it matches a known pattern.
/// Returns None for unrecognized messages.
pub fn parse_chat(text: &str) -> Option<ChatEvent> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_card_refs_single() {
        let text = "TalTheTurtle discards @[Consign to Memory@:252250,421:@].";
        let refs = extract_card_refs(text);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0], ChatCardRef {
            name: "Consign to Memory".to_string(),
            texture_id: 252250,
            thing_id: 421,
        });
    }

    #[test]
    fn test_extract_card_refs_multiple() {
        let text = "TalTheTurtle counters @[Bowmasters@:224232,467:@] with @[Strix Serenade@:252318,468:@].";
        let refs = extract_card_refs(text);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].thing_id, 467);
        assert_eq!(refs[1].thing_id, 468);
    }

    #[test]
    fn test_extract_card_refs_none() {
        let text = "Turn 3: TalTheTurtle";
        let refs = extract_card_refs(text);
        assert!(refs.is_empty());
    }

    #[test]
    fn test_parse_discard() {
        let text = "TalTheTurtle discards @[Consign to Memory@:252250,421:@].";
        let event = parse_chat(text).unwrap();
        assert_eq!(event, ChatEvent::Discard {
            player: "TalTheTurtle".to_string(),
            card: ChatCardRef {
                name: "Consign to Memory".to_string(),
                texture_id: 252250,
                thing_id: 421,
            },
        });
    }

    #[test]
    fn test_parse_put_into_graveyard() {
        let text = "coreyabaker puts @[Orcish Bowmasters@:224232,283:@] into their graveyard.";
        let event = parse_chat(text).unwrap();
        assert_eq!(event, ChatEvent::PutIntoGraveyard {
            player: "coreyabaker".to_string(),
            card: ChatCardRef {
                name: "Orcish Bowmasters".to_string(),
                texture_id: 224232,
                thing_id: 283,
            },
        });
    }

    #[test]
    fn test_parse_exile() {
        let text = "TalTheTurtle exiles @[Fable of the Mirror-Breaker@:194420,458:@] with its own ability.";
        let event = parse_chat(text).unwrap();
        assert_eq!(event, ChatEvent::Exile {
            player: "TalTheTurtle".to_string(),
            card: ChatCardRef {
                name: "Fable of the Mirror-Breaker".to_string(),
                texture_id: 194420,
                thing_id: 458,
            },
        });
    }

    #[test]
    fn test_parse_put_on_library_bottom() {
        let text = "TalTheTurtle puts @[Phelia, Exuberant Shepherd@:252194,509:@] on bottom of their library.";
        let event = parse_chat(text).unwrap();
        assert_eq!(event, ChatEvent::PutOnLibrary {
            player: "TalTheTurtle".to_string(),
            card: ChatCardRef {
                name: "Phelia, Exuberant Shepherd".to_string(),
                texture_id: 252194,
                thing_id: 509,
            },
            position: LibPos::Bottom,
        });
    }

    #[test]
    fn test_parse_put_on_library_top() {
        let text = "coreyabaker puts @[Lightning Bolt@:100000,300:@] on top of their library.";
        let event = parse_chat(text).unwrap();
        assert_eq!(event, ChatEvent::PutOnLibrary {
            player: "coreyabaker".to_string(),
            card: ChatCardRef {
                name: "Lightning Bolt".to_string(),
                texture_id: 100000,
                thing_id: 300,
            },
            position: LibPos::Top,
        });
    }

    #[test]
    fn test_parse_create_token() {
        let text = "TalTheTurtle's @[Fable of the Mirror-Breaker@:194420,458:@] creates a Goblin Shaman Token.";
        let event = parse_chat(text).unwrap();
        assert_eq!(event, ChatEvent::CreateToken {
            player: "TalTheTurtle".to_string(),
            source_card: ChatCardRef {
                name: "Fable of the Mirror-Breaker".to_string(),
                texture_id: 194420,
                thing_id: 458,
            },
            token_name: "Goblin Shaman Token".to_string(),
        });
    }

    #[test]
    fn test_parse_unrecognized_returns_none() {
        assert!(parse_chat("Turn 3: TalTheTurtle").is_none());
        assert!(parse_chat("TalTheTurtle draws a card.").is_none());
        assert!(parse_chat("coreyabaker rolled a 2.").is_none());
        assert!(parse_chat("").is_none());
    }
}
```

- [ ] **Step 2: Register the module in lib.rs**

In `src/lib.rs`, add `pub mod chat;` after the existing module declarations.

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib chat`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 4: Implement extract_card_refs**

Replace the `todo!()` in `extract_card_refs` with:

```rust
pub fn extract_card_refs(text: &str) -> Vec<ChatCardRef> {
    let mut refs = Vec::new();
    let mut search_from = 0;
    while let Some(start) = text[search_from..].find("@[") {
        let abs_start = search_from + start;
        // Find the closing :@]
        if let Some(end) = text[abs_start..].find(":@]") {
            let abs_end = abs_start + end + 3; // include ":@]"
            let inner = &text[abs_start + 2..abs_start + end]; // between @[ and :@]
            // Format: "CardName@:textureId,thingId"
            if let Some(at_colon) = inner.find("@:") {
                let name = &inner[..at_colon];
                let ids = &inner[at_colon + 2..];
                if let Some(comma) = ids.find(',') {
                    let texture_id = ids[..comma].parse::<u32>().ok();
                    let thing_id = ids[comma + 1..].parse::<u32>().ok();
                    if let (Some(tex), Some(tid)) = (texture_id, thing_id) {
                        refs.push(ChatCardRef {
                            name: name.to_string(),
                            texture_id: tex,
                            thing_id: tid,
                        });
                    }
                }
            }
            search_from = abs_end;
        } else {
            break;
        }
    }
    refs
}
```

- [ ] **Step 5: Run card ref extraction tests**

Run: `cargo test --lib chat::tests::test_extract_card_refs`
Expected: All 3 `extract_card_refs` tests pass.

- [ ] **Step 6: Implement parse_chat**

Replace the `todo!()` in `parse_chat` with:

```rust
pub fn parse_chat(text: &str) -> Option<ChatEvent> {
    let refs = extract_card_refs(text);

    // "{player} discards @[Card@:tex,id:@]."
    if let Some(pos) = text.find(" discards @[") {
        let player = text[..pos].to_string();
        let card = refs.into_iter().next()?;
        return Some(ChatEvent::Discard { player, card });
    }

    // "{player} puts @[Card@:tex,id:@] into their graveyard."
    if let Some(pos) = text.find(" puts @[") {
        if text.contains("into their graveyard") {
            let player = text[..pos].to_string();
            let card = refs.into_iter().next()?;
            return Some(ChatEvent::PutIntoGraveyard { player, card });
        }
        // "{player} puts @[Card@:tex,id:@] on top/bottom of their library."
        if text.contains("of their library") {
            let player = text[..pos].to_string();
            let card = refs.into_iter().next()?;
            let position = if text.contains("on top of") {
                LibPos::Top
            } else {
                LibPos::Bottom
            };
            return Some(ChatEvent::PutOnLibrary { player, card, position });
        }
    }

    // "{player} exiles @[Card@:tex,id:@]"
    if let Some(pos) = text.find(" exiles @[") {
        let player = text[..pos].to_string();
        let card = refs.into_iter().next()?;
        return Some(ChatEvent::Exile { player, card });
    }

    // "{player}'s @[Card@:tex,id:@] creates a {TokenName}."
    if text.contains(" creates a ") {
        if let Some(apos_pos) = text.find("'s @[") {
            let player = text[..apos_pos].to_string();
            let source_card = refs.into_iter().next()?;
            // Token name is between "creates a " and the trailing "."
            if let Some(creates_pos) = text.find(" creates a ") {
                let after_creates = &text[creates_pos + 11..];
                let token_name = after_creates.trim_end_matches('.').to_string();
                return Some(ChatEvent::CreateToken { player, source_card, token_name });
            }
        }
    }

    None
}
```

- [ ] **Step 7: Run all chat parser tests**

Run: `cargo test --lib chat`
Expected: All 9 tests pass.

- [ ] **Step 8: Commit**

```bash
git add src/chat.rs src/lib.rs
git commit -m "feat: add chat parser module for MTGO game log events"
```

---

### Task 3: Add is_token field to ThingState

**Files:**
- Modify: `src/state.rs:124-173` (ThingState struct and Default impl)
- Modify: `src/state.rs:260-354` (property application match arm)
- Modify: `src/translator.rs:615-638` (default_thing test helper)

- [ ] **Step 1: Add is_token field to ThingState**

In `src/state.rs`, add to the `ThingState` struct after `from_zone`:

```rust
    pub is_token: bool,
```

In the `Default` impl, add after `from_zone: None,`:

```rust
            is_token: false,
```

- [ ] **Step 2: Add IS_TOKEN property handling in apply_elements**

In `src/state.rs`, in the property match block (around line 348, after the `SRC_THING_ID` arm), add:

```rust
                            opcodes::IS_TOKEN => {
                                if let PropertyValue::Int(v) = value {
                                    thing.is_token = *v != 0;
                                }
                            }
```

- [ ] **Step 3: Update default_thing in translator tests**

In `src/translator.rs`, in the `default_thing` test helper, add `is_token: false,` after `from_zone: None,`.

- [ ] **Step 4: Run all tests to verify nothing breaks**

Run: `cargo test`
Expected: All tests pass. The new field is backward-compatible (defaults to false).

- [ ] **Step 5: Commit**

```bash
git add src/state.rs src/translator.rs
git commit -m "feat: track IS_TOKEN property on ThingState"
```

---

### Task 4: Integrate chat context into the translator

**Files:**
- Modify: `src/translator.rs` (ReplayTranslator struct, new/reset, ingest_chat, process diff logic)

- [ ] **Step 1: Write failing test for chat-driven discard**

In `src/translator.rs` tests module, add:

```rust
    #[test]
    fn test_chat_discard() {
        let mut translator = ReplayTranslator::new();
        translator.set_player_names(vec!["Alice".into(), "Bob".into()]);

        // Initial state: no card 421 visible (it's in hand, but hand contents
        // aren't always in the state — cards appear as new things when discarded)
        let s1 = make_state(1, 1, GamePhase::PreCombatMain);
        translator.process(&s1, false);

        // Chat arrives: "Alice discards @[Bolt@:100,421:@]."
        translator.ingest_chat("Alice discards @[Bolt@:100,421:@].");

        // State: card 421 appears in graveyard (zone 3) as a new thing
        let mut s2 = make_state(1, 1, GamePhase::PreCombatMain);
        let mut t = default_thing(421, 3); // graveyard
        t.from_zone = Some(-1); // moved from somewhere
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

        // Chat: surveil puts card into graveyard (card never seen before → library source)
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

        // Chat: token creation
        translator.ingest_chat("Alice's @[Fable@:194420,458:@] creates a Goblin Shaman Token.");

        // State: new token appears on battlefield
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

        // Card 510 was on battlefield — tracked by last_known_zones
        let mut s1 = make_state(1, 1, GamePhase::PreCombatMain);
        s1.things.insert(510, default_thing(510, ZONE_BATTLEFIELD));
        translator.process(&s1, false);

        // Full state prune removes card 510 from state (but last_known_zones retains it)
        let s2 = make_state(1, 1, GamePhase::PreCombatMain);
        translator.process(&s2, true);

        // Chat: exile from battlefield
        translator.ingest_chat("Alice exiles @[Subtlety@:181014,510:@].");

        // State: card reappears in exile (zone 4) as a NEW thing (not in prev)
        let mut s3 = make_state(1, 1, GamePhase::PreCombatMain);
        let mut t = default_thing(510, 4); // exile
        t.from_zone = Some(-1);
        s3.things.insert(510, t);
        let actions = translator.process(&s3, false);

        assert!(actions.iter().any(|a| matches!(
            &a.action_type,
            ActionType::ZoneTransition { from_zone, to_zone, .. }
                if from_zone == "battlefield" && to_zone == "exile"
        )), "Expected ZoneTransition with from_zone=battlefield, got: {:?}", actions.iter().map(|a| &a.action_type).collect::<Vec<_>>());
    }

    #[test]
    fn test_last_known_zones_fallback() {
        let mut translator = ReplayTranslator::new();
        translator.set_player_names(vec!["Alice".into(), "Bob".into()]);

        // Card 600 was on battlefield (tracked by last_known_zones)
        let mut s1 = make_state(1, 1, GamePhase::PreCombatMain);
        s1.things.insert(600, default_thing(600, ZONE_BATTLEFIELD));
        translator.process(&s1, false);

        // Full state prune removes card 600
        let s2 = make_state(1, 1, GamePhase::PreCombatMain);
        translator.process(&s2, true);

        // Card 600 reappears in graveyard with no chat context
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib translator::tests::test_chat`
Expected: FAIL — `ingest_chat` method doesn't exist yet.

- [ ] **Step 3: Add chat_context and pending_tokens fields to ReplayTranslator**

In `src/translator.rs`, add imports at the top:

```rust
use std::collections::VecDeque;
use crate::chat::{self, ChatEvent};
```

Add fields to `ReplayTranslator` struct:

```rust
    /// Per-thing-id chat context for zone resolution and action classification.
    chat_context: HashMap<u32, ChatEvent>,
    /// Pending token creation events keyed by token name.
    pending_tokens: VecDeque<(String, ChatEvent)>,
```

Initialize in `new()`:

```rust
            chat_context: HashMap::new(),
            pending_tokens: VecDeque::new(),
```

Clear in `reset()`:

```rust
        self.chat_context.clear();
        self.pending_tokens.clear();
```

- [ ] **Step 4: Implement ingest_chat method**

Add to the `impl ReplayTranslator` block:

```rust
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
```

- [ ] **Step 5: Update the "new thing" branch in diff() to use chat context**

In `src/translator.rs`, replace the catch-all `else if moved && zone != ZONE_LIBRARY` branch (lines ~562-575) AND its preceding branches with chat-aware logic. The full replacement for the `} else {` block starting at line 485 should be:

Replace the block from `} else {` (line 485) through the closing `}` before `// from_zone == None` comment (line 575) with:

```rust
            } else {
                // New thing — not in prev state.  Use from_zone to distinguish
                // real zone transitions from visibility-only appearances.
                //
                // from_zone == Some(-1): thing actually moved from another zone
                //   (the element had a non-trivial from_zone object reference).
                // from_zone == None: first-time visibility, no zone transition.
                let moved = new_thing.from_zone == Some(-1);
                let zone = new_thing.zone;

                // Check chat context for this thing (authoritative source)
                let chat_event = self.chat_context.remove(thing_id);

                // Resolve source zone: chat > last_known_zones > "unknown"
                let resolved_from_zone = if let Some(ref evt) = chat_event {
                    match evt {
                        ChatEvent::Discard { .. } => Some(ZONE_HAND),
                        ChatEvent::PutIntoGraveyard { .. } => {
                            // If we saw it somewhere, use that; otherwise assume library
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
```

Note: This uses `continue` statements, so the outer loop over `new.things` must be a `for` loop (it already is — the `for (thing_id, new_thing) in &new.things` at the start of the diff method).

- [ ] **Step 6: Run the new translator tests**

Run: `cargo test --lib translator::tests::test_chat`
Expected: All 5 new tests pass.

- [ ] **Step 7: Run ALL existing tests to verify no regressions**

Run: `cargo test`
Expected: All tests pass (existing translator tests still work).

- [ ] **Step 8: Commit**

```bash
git add src/translator.rs
git commit -m "feat: integrate chat context into translator for zone resolution"
```

---

### Task 5: Wire up ingest_chat in the decode pipeline

**Files:**
- Modify: `src/bin/decode.rs:443-459` (UserChat match arm)

- [ ] **Step 1: Add ingest_chat call to UserChat handler**

In `src/bin/decode.rs`, in the `GameMessage::UserChat` match arm (line ~443), add `translator.ingest_chat(text);` right after the `tracing::debug!` line and **before** the turn-ownership parsing:

```rust
                    GameMessage::UserChat { ref text } => {
                        tracing::debug!("CHAT: {}", text);
                        translator.ingest_chat(text);
                        // Parse "Turn N: PlayerName" to set active player
```

- [ ] **Step 2: Build and run the full pipeline on golden file**

Run: `cargo run --bin decode -- tests/fixtures/golden_v1.bin 2>/dev/null | tail -5`
Expected: Compiles and runs without errors. Output shows game summary.

- [ ] **Step 3: Verify unknown from_zone count reduced**

Run: `cargo run --bin decode -- tests/fixtures/golden_v1.bin 2>/dev/null | grep -c '"unknown"'`
Expected: Count should be significantly lower than the previous 37. Ideally close to 0.

- [ ] **Step 4: Verify new action types appear in output**

Run: `cargo run --bin decode -- tests/fixtures/golden_v1.bin 2>/dev/null | grep -E "discards|mills|creates"`
Expected: Output shows discard, mill, and/or create token actions.

- [ ] **Step 5: Commit**

```bash
git add src/bin/decode.rs
git commit -m "feat: wire chat context into decode pipeline"
```

---

### Task 6: Update golden test fixtures

**Files:**
- Modify: `tests/fixtures/golden_v1_replay.json` (regenerate)
- Modify: `tests/fixtures/golden_game3_replay.json` (regenerate if needed)
- Modify: any golden test code that asserts on specific action counts or types

- [ ] **Step 1: Check what golden tests exist**

Run: `cargo test golden 2>&1 | head -30`
Look at test names and whether they assert on specific action types or counts.

- [ ] **Step 2: Regenerate golden replay files**

Run: `cargo run --bin decode -- tests/fixtures/golden_v1.bin > /tmp/new_golden.json 2>/dev/null`
Then compare: `diff <(python3 -m json.tool tests/fixtures/golden_v1_replay.json) <(python3 -m json.tool /tmp/new_golden.json) | head -80`

Review the diff to confirm:
- `from_zone: "unknown"` replaced with actual zones
- New `Discard`, `Mill`, `CreateToken` variants appear where expected
- No unexpected changes

- [ ] **Step 3: Update golden fixture files**

Copy the regenerated files:
```bash
cargo run --bin decode -- tests/fixtures/golden_v1.bin > tests/fixtures/golden_v1_replay.json 2>/dev/null
```

If `golden_game3_replay.json` is a subset/different format, regenerate it too using the appropriate command.

- [ ] **Step 4: Run all tests to verify golden fixtures match**

Run: `cargo test`
Expected: All tests pass with updated fixtures.

- [ ] **Step 5: Commit**

```bash
git add tests/fixtures/
git commit -m "chore: regenerate golden fixtures with chat-driven zone resolution"
```

---

### Task 7: Update web viewer types and reconstructor

**Files:**
- Modify: `web/src/types/replay.ts:62-85` (RawActionType union)
- Modify: `web/src/engine/reconstructor.ts:91-176` (applyAction switch)
- Modify: `web/src/engine/reconstructor.test.ts` (new test cases)

- [ ] **Step 1: Add new variants to RawActionType**

In `web/src/types/replay.ts`, add to the `RawActionType` union (after the `ZoneTransition` line):

```typescript
  | { Discard: { player_id: string; card_id: string } }
  | { Mill: { player_id: string; card_id: string } }
  | { CreateToken: { player_id: string; card_id: string; token_name: string } }
```

- [ ] **Step 2: Add case handlers in reconstructor.ts**

In `web/src/engine/reconstructor.ts`, in the `applyAction` switch statement, add cases before the `default` case:

```typescript
    case 'Discard':
      return applyZoneTransition(newState, {
        card_id: (data as { card_id: string }).card_id,
        from_zone: 'hand',
        to_zone: 'graveyard',
        player_id: (data as { player_id: string }).player_id,
      }, cardNames);

    case 'Mill':
      return applyZoneTransition(newState, {
        card_id: (data as { card_id: string }).card_id,
        from_zone: 'library',
        to_zone: 'graveyard',
        player_id: (data as { player_id: string }).player_id,
      }, cardNames);

    case 'CreateToken': {
      const { player_id, card_id, token_name } = data as { player_id: string; card_id: string; token_name: string };
      const tokenCard = { ...createCard(card_id, player_id), name: token_name };
      return addCardToZone(newState, 'battlefield', player_id, tokenCard);
    }
```

- [ ] **Step 3: Add test cases for new action types**

In `web/src/engine/reconstructor.test.ts`, add tests:

```typescript
  it('should handle Discard action', () => {
    const actions: RawReplayAction[] = [
      {
        turn: 1,
        phase: 'main1',
        active_player: 'player1',
        action_type: { DrawCard: { player_id: 'player1', card_id: 'card1' } },
      },
      {
        turn: 1,
        phase: 'main1',
        active_player: 'player1',
        action_type: { Discard: { player_id: 'player1', card_id: 'card1' } },
      },
    ];
    const rec = new Reconstructor();
    rec.loadReplay(makeReplay(actions));
    const state = rec.reconstruct(2);
    expect(state.zones.find(z => z.name === 'hand')?.cards ?? []).toHaveLength(0);
    expect(state.zones.find(z => z.name === 'graveyard')?.cards).toHaveLength(1);
  });

  it('should handle Mill action', () => {
    const actions: RawReplayAction[] = [
      {
        turn: 1,
        phase: 'main1',
        active_player: 'player1',
        action_type: { Mill: { player_id: 'player1', card_id: 'card1' } },
      },
    ];
    const rec = new Reconstructor();
    rec.loadReplay(makeReplay(actions));
    const state = rec.reconstruct(1);
    expect(state.zones.find(z => z.name === 'graveyard')?.cards).toHaveLength(1);
  });

  it('should handle CreateToken action', () => {
    const actions: RawReplayAction[] = [
      {
        turn: 1,
        phase: 'main1',
        active_player: 'player1',
        action_type: { CreateToken: { player_id: 'player1', card_id: 'token1', token_name: 'Goblin Token' } },
      },
    ];
    const rec = new Reconstructor();
    rec.loadReplay(makeReplay(actions));
    const state = rec.reconstruct(1);
    const bf = state.zones.find(z => z.name === 'battlefield');
    expect(bf?.cards).toHaveLength(1);
    expect(bf?.cards[0].name).toBe('Goblin Token');
  });
```

- [ ] **Step 4: Run web tests**

Run: `cd web && npx vitest run`
Expected: All tests pass including the 3 new ones.

- [ ] **Step 5: Commit**

```bash
git add web/src/types/replay.ts web/src/engine/reconstructor.ts web/src/engine/reconstructor.test.ts
git commit -m "feat: add Discard, Mill, CreateToken support to web viewer"
```

---

### Task 8: Update KNOWN_ISSUES.md

**Files:**
- Modify: `KNOWN_ISSUES.md`

- [ ] **Step 1: Mark P2 zone transitions as fixed**

In `KNOWN_ISSUES.md`, change the `## P2: Misclassified Zone Transitions` heading to `## ~~P2: Misclassified Zone Transitions~~ — FIXED` and replace the body with:

```markdown
Fixed by parsing MTGO chat log messages (`NEW_USER_CHAT`) into a per-thing-id context that the translator consults when resolving `from_zone` for new things. Chat events (discard, exile, graveyard, etc.) provide authoritative source zone information. `last_known_zones` fallback resolves remaining cases for things previously tracked but pruned from state.
```

- [ ] **Step 2: Update P3 Missing Action Types to reflect new variants**

In the P3 Missing Action Types table, strike through the resolved rows and update them:

```markdown
| ~~Token creation~~ | ~~FIXED — chat "creates a" pattern → `CreateToken` action~~ |
| ~~Discard~~ | ~~FIXED — chat "discards" pattern → `Discard` action; also added `Mill` for library → graveyard (surveil/mill) via chat "puts X into their graveyard"~~ |
```

Keep the remaining unresolved items (Scry/Surveil, Mulligan, Mana tapping) as-is.

- [ ] **Step 3: Commit**

```bash
git add KNOWN_ISSUES.md
git commit -m "docs: mark zone transitions and missing action types as fixed"
```
