//! Scryfall API client for resolving MTGO texture IDs to card names.
//!
//! Uses the /cards/collection bulk endpoint for IDs < 100,000 and
//! individual /cards/mtgo/{id} requests for larger IDs.

use std::collections::HashMap;
use std::thread;
use std::time::Duration;

const USER_AGENT: &str = "flashback-mtgo-replay/0.1.0";

/// Resolve a set of MTGO IDs to card names via the Scryfall API.
///
/// - IDs < 100,000 are batched into POST /cards/collection requests (max 75 per request).
/// - IDs >= 100,000 are fetched individually via GET /cards/mtgo/{id}.
/// - Failed lookups are silently skipped (tokens, emblems, etc.).
/// - Results are cached in `cache` so repeated calls across games don't re-fetch.
pub fn resolve_mtgo_ids(
    mtgo_ids: &[i32],
    cache: &mut HashMap<i32, String>,
) -> HashMap<i32, String> {
    let mut result = HashMap::new();
    let mut to_fetch_batch: Vec<i32> = Vec::new();
    let mut to_fetch_individual: Vec<i32> = Vec::new();

    for &id in mtgo_ids {
        if let Some(name) = cache.get(&id) {
            result.insert(id, name.clone());
        } else if id < 100_000 {
            to_fetch_batch.push(id);
        } else {
            to_fetch_individual.push(id);
        }
    }

    // Batch fetch via /cards/collection (max 75 per request)
    for chunk in to_fetch_batch.chunks(75) {
        if let Some(names) = fetch_collection_batch(chunk) {
            for (id, name) in &names {
                cache.insert(*id, name.clone());
                result.insert(*id, name.clone());
            }
        }
        if chunk.len() == 75 {
            thread::sleep(Duration::from_millis(100));
        }
    }

    // Individual fetch for large IDs
    for &id in &to_fetch_individual {
        if let Some(name) = fetch_individual(id) {
            cache.insert(id, name.clone());
            result.insert(id, name);
        }
        thread::sleep(Duration::from_millis(100));
    }

    result
}

/// POST /cards/collection with mtgo_id identifiers.
/// Returns a map of mtgo_id -> card name for cards found.
fn fetch_collection_batch(ids: &[i32]) -> Option<HashMap<i32, String>> {
    let identifiers: Vec<serde_json::Value> = ids
        .iter()
        .map(|&id| serde_json::json!({ "mtgo_id": id }))
        .collect();

    let body = serde_json::json!({ "identifiers": identifiers });

    let resp = ureq::post("https://api.scryfall.com/cards/collection")
        .set("Content-Type", "application/json")
        .set("User-Agent", USER_AGENT)
        .send_string(&body.to_string())
        .ok()?;

    let json: serde_json::Value = resp.into_json().ok()?;
    let data = json.get("data")?.as_array()?;

    let mut result = HashMap::new();
    for card in data {
        if let (Some(mtgo_id), Some(name)) = (
            card.get("mtgo_id").and_then(|v| v.as_i64()),
            card.get("name").and_then(|v| v.as_str()),
        ) {
            result.insert(mtgo_id as i32, name.to_string());
        }
    }
    Some(result)
}

/// GET /cards/mtgo/{id} for a single card.
fn fetch_individual(id: i32) -> Option<String> {
    let url = format!("https://api.scryfall.com/cards/mtgo/{}", id);
    let resp = ureq::get(&url)
        .set("User-Agent", USER_AGENT)
        .call()
        .ok()?;
    let json: serde_json::Value = resp.into_json().ok()?;
    json.get("name").and_then(|v| v.as_str()).map(|s| s.to_string())
}
