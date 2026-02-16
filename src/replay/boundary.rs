use crate::protocol::GameEvent;
use std::ops::Range;

/// Check if an event marks the start of a game
pub fn is_game_start_event(event: &GameEvent) -> bool {
    matches!(event, GameEvent::GameStart { .. })
}

/// Check if an event marks the end of a game
pub fn is_game_end_event(event: &GameEvent) -> bool {
    matches!(event, GameEvent::GameEnd { .. })
}

/// Find all game boundaries (start and end indices) in an event stream
/// Returns a list of (start_index, end_index) ranges for each complete game
/// 
/// # Arguments
/// * `events` - Slice of game events in chronological order
/// 
/// # Returns
/// Vector of ranges, where each range contains the indices of events belonging to one game
pub fn find_game_boundaries(events: &[GameEvent]) -> Vec<Range<usize>> {
    let mut boundaries = Vec::new();
    let mut current_start: Option<usize> = None;
    
    for (index, event) in events.iter().enumerate() {
        if is_game_start_event(event) {
            current_start = Some(index);
        } else if is_game_end_event(event) {
            if let Some(start) = current_start {
                boundaries.push(start..=index);
                current_start = None;
            }
        }
    }
    
    // Convert inclusive ranges to exclusive ranges
    boundaries
        .into_iter()
        .map(|range| *range.start()..*range.end() + 1)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_events() -> Vec<GameEvent> {
        vec![
            GameEvent::GameStart { game_id: "game1".to_string() },
            GameEvent::DrawCard {
                player_id: "p1".to_string(),
                card_id: "c1".to_string(),
            },
            GameEvent::DrawCard {
                player_id: "p2".to_string(),
                card_id: "c2".to_string(),
            },
            GameEvent::PlayLand {
                player_id: "p1".to_string(),
                card_id: "c3".to_string(),
            },
            GameEvent::CastSpell {
                player_id: "p1".to_string(),
                card_id: "c4".to_string(),
            },
            GameEvent::GameEnd { winner: "p1".to_string() },
            GameEvent::GameStart { game_id: "game2".to_string() },
            GameEvent::DrawCard {
                player_id: "p1".to_string(),
                card_id: "c5".to_string(),
            },
            GameEvent::DrawCard {
                player_id: "p2".to_string(),
                card_id: "c6".to_string(),
            },
            GameEvent::GameEnd { winner: "p2".to_string() },
        ]
    }
    
    #[test]
    fn test_is_game_start_event() {
        let start = GameEvent::GameStart { game_id: "test".to_string() };
        let not_start = GameEvent::DrawCard {
            player_id: "p1".to_string(),
            card_id: "c1".to_string(),
        };
        
        assert!(is_game_start_event(&start));
        assert!(!is_game_start_event(&not_start));
    }
    
    #[test]
    fn test_is_game_end_event() {
        let end = GameEvent::GameEnd { winner: "p1".to_string() };
        let not_end = GameEvent::CastSpell {
            player_id: "p1".to_string(),
            card_id: "c1".to_string(),
        };
        
        assert!(is_game_end_event(&end));
        assert!(!is_game_end_event(&not_end));
    }
    
    #[test]
    fn test_find_game_boundaries_single_game() {
        let events = vec![
            GameEvent::GameStart { game_id: "game1".to_string() },
            GameEvent::DrawCard {
                player_id: "p1".to_string(),
                card_id: "c1".to_string(),
            },
            GameEvent::GameEnd { winner: "p1".to_string() },
        ];
        
        let boundaries = find_game_boundaries(&events);
        
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0], 0..3); // indices 0, 1, 2
    }
    
    #[test]
    fn test_find_game_boundaries_multiple_games() {
        let events = create_test_events();
        
        let boundaries = find_game_boundaries(&events);
        
        assert_eq!(boundaries.len(), 2);
        assert_eq!(boundaries[0], 0..6); // first game: indices 0-5
        assert_eq!(boundaries[1], 6..10); // second game: indices 6-9
    }
    
    #[test]
    fn test_find_game_boundaries_incomplete_game() {
        let events = vec![
            GameEvent::GameStart { game_id: "game1".to_string() },
            GameEvent::DrawCard {
                player_id: "p1".to_string(),
                card_id: "c1".to_string(),
            },
            // No game end - should not be included
        ];
        
        let boundaries = find_game_boundaries(&events);
        
        assert_eq!(boundaries.len(), 0); // No complete games
    }
    
    #[test]
    fn test_find_game_boundaries_no_games() {
        let events = vec![
            GameEvent::DrawCard {
                player_id: "p1".to_string(),
                card_id: "c1".to_string(),
            },
            GameEvent::PlayLand {
                player_id: "p1".to_string(),
                card_id: "c2".to_string(),
            },
        ];
        
        let boundaries = find_game_boundaries(&events);
        
        assert_eq!(boundaries.len(), 0);
    }
    
    #[test]
    fn test_find_game_boundaries_end_without_start() {
        let events = vec![
            GameEvent::DrawCard {
                player_id: "p1".to_string(),
                card_id: "c1".to_string(),
            },
            GameEvent::GameEnd { winner: "p1".to_string() },
            // Game end without start should not create a boundary
        ];
        
        let boundaries = find_game_boundaries(&events);
        
        assert_eq!(boundaries.len(), 0);
    }
    
    #[test]
    fn test_find_game_boundaries_nested_games() {
        let events = vec![
            GameEvent::GameStart { game_id: "game1".to_string() },
            GameEvent::DrawCard {
                player_id: "p1".to_string(),
                card_id: "c1".to_string(),
            },
            GameEvent::GameStart { game_id: "game2".to_string() }, // Unexpected nested start
            GameEvent::DrawCard {
                player_id: "p2".to_string(),
                card_id: "c2".to_string(),
            },
            GameEvent::GameEnd { winner: "p2".to_string() }, // Ends game2
            GameEvent::GameEnd { winner: "p1".to_string() }, // Ends game1
        ];
        
        let boundaries = find_game_boundaries(&events);
        // With nested starts, the second start resets, so only the second
        // game (from the second start) gets a proper boundary
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0], 2..5); // game2: indices 2-4 (from second start to first end)
    }
}
