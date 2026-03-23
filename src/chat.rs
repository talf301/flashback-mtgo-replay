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
    let mut refs = Vec::new();
    let mut search_from = 0;
    while let Some(start) = text[search_from..].find("@[") {
        let abs_start = search_from + start;
        if let Some(end) = text[abs_start..].find(":@]") {
            let abs_end = abs_start + end + 3;
            let inner = &text[abs_start + 2..abs_start + end];
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

/// Parse a chat message into a ChatEvent, if it matches a known pattern.
/// Returns None for unrecognized messages.
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
            if let Some(creates_pos) = text.find(" creates a ") {
                let after_creates = &text[creates_pos + 11..];
                let token_name = after_creates.trim_end_matches('.').to_string();
                return Some(ChatEvent::CreateToken { player, source_card, token_name });
            }
        }
    }

    None
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
