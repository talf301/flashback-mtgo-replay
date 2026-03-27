# Scryfall Card Name Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve card names from MTGO texture IDs via Scryfall API so replay output goes from ~15% to ~99% card name coverage.

**Architecture:** Add a `src/scryfall.rs` module with a blocking HTTP client (`ureq`) that resolves MTGO IDs to card names. The collection endpoint (`POST /cards/collection`) handles IDs < 100,000 in batches of 75; individual GET requests handle IDs >= 100,000. Called in `decode_pipeline()` after all games are packaged but before returning the `ReplayFile`. A `--no-resolve` flag skips the network calls for testing/offline use. An in-memory cache deduplicates lookups across games in the same match.

**Tech Stack:** Rust, `ureq` (blocking HTTP), `serde_json` (already present)

---

### Task 1: Add `ureq` dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add ureq to dependencies**

In `Cargo.toml`, add under `[dependencies]`:

```toml
ureq = { version = "2", features = ["json"] }
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add ureq HTTP client dependency for Scryfall integration"
```

---

### Task 2: Create `src/scryfall.rs` module with batch resolution

**Files:**
- Create: `src/scryfall.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create the scryfall module with resolve function**

Create `src/scryfall.rs`:

```rust
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
```

- [ ] **Step 2: Export the module from lib.rs**

In `src/lib.rs`, add:

```rust
pub mod scryfall;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles (possibly with unused warnings, that's fine)

- [ ] **Step 4: Commit**

```bash
git add src/scryfall.rs src/lib.rs
git commit -m "feat: add scryfall module for MTGO ID to card name resolution"
```

---

### Task 3: Integrate Scryfall resolution into decode pipeline

**Files:**
- Modify: `src/bin/decode.rs`

The resolution runs once after all games are packaged, iterating over each game's `card_textures` to fill in missing `card_names`. A shared cache avoids re-fetching the same MTGO ID across games.

- [ ] **Step 1: Add `--no-resolve` flag parsing**

In `src/bin/decode.rs`, after the existing flag parsing (around line 46), add:

```rust
let no_resolve = args.iter().any(|a| a == "--no-resolve");
```

Also update the usage string to document the new flag:

```rust
eprintln!("  --no-resolve   Skip Scryfall card name lookups (offline mode)");
```

- [ ] **Step 2: Add Scryfall resolution after pipeline completes**

In `src/bin/decode.rs`, after `decode_pipeline()` returns the `ReplayFile` (around line 76) but before the JSON output, add the resolution step. Change:

```rust
let replay = decode_pipeline(messages);
```

to:

```rust
let mut replay = decode_pipeline(messages);

if !no_resolve {
    resolve_card_names(&mut replay);
}
```

Then add the `resolve_card_names` function and the import:

```rust
use flashback::scryfall;
```

```rust
/// Resolve missing card names from Scryfall using texture IDs.
fn resolve_card_names(replay: &mut ReplayFile) {
    // Collect all unique MTGO IDs that lack names across all games
    let mut ids_to_resolve: Vec<i32> = Vec::new();
    for game in &replay.games {
        for (thing_id, &mtgo_id) in &game.card_textures {
            if !game.card_names.contains_key(thing_id) {
                ids_to_resolve.push(mtgo_id);
            }
        }
    }
    ids_to_resolve.sort_unstable();
    ids_to_resolve.dedup();

    if ids_to_resolve.is_empty() {
        return;
    }

    eprintln!(
        "Resolving {} unique card IDs via Scryfall...",
        ids_to_resolve.len()
    );

    let mut cache = std::collections::HashMap::new();
    let resolved = scryfall::resolve_mtgo_ids(&ids_to_resolve, &mut cache);

    eprintln!(
        "Resolved {}/{} card names",
        resolved.len(),
        ids_to_resolve.len()
    );

    // Fill in card_names for each game
    for game in &mut replay.games {
        for (thing_id, &mtgo_id) in &game.card_textures {
            if !game.card_names.contains_key(thing_id) {
                if let Some(name) = resolved.get(&mtgo_id) {
                    game.card_names.insert(thing_id.clone(), name.clone());
                }
            }
        }
    }
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

- [ ] **Step 4: Manual test with golden file (online)**

Run: `cargo run --bin decode -- tests/fixtures/golden_v1.bin --text-log 2>&1 | head -40`

Expected: stderr shows "Resolving N unique card IDs via Scryfall..." and "Resolved M/N card names". The text log should show card names instead of `#123` placeholders.

- [ ] **Step 5: Manual test with --no-resolve**

Run: `cargo run --bin decode -- tests/fixtures/golden_v1.bin --no-resolve --text-log 2>&1 | head -40`

Expected: no Scryfall messages in stderr, output shows `#123` placeholders for unnamed cards.

- [ ] **Step 6: Commit**

```bash
git add src/bin/decode.rs
git commit -m "feat: integrate Scryfall card name resolution into decode pipeline"
```

---

### Task 4: Verify existing tests pass

The golden pipeline test (`tests/golden_pipeline.rs`) runs the decode pipeline directly via `run_pipeline()` — it never calls the `decode` binary, so Scryfall resolution doesn't run. The regression test (`test_golden_snapshot_regression`) only compares `ActionSnapshot` fields (turn, phase, active_player, action_type), not `card_names`. So no fixture regeneration is needed.

- [ ] **Step 1: Run all tests**

Run: `cargo test`
Expected: all tests pass with no changes to test code or fixtures

- [ ] **Step 2: If tests fail, investigate**

The Scryfall changes are isolated to `src/scryfall.rs` and `src/bin/decode.rs`. If tests fail, it's likely a compilation issue — fix and re-run.

---

### Task 5: Update KNOWN_ISSUES.md

**Files:**
- Modify: `KNOWN_ISSUES.md`

- [ ] **Step 1: Mark the card names issue as mostly fixed**

Update the "P1: Empty Card Names and Textures" section to reflect the new state: Scryfall resolution fills in ~99% of card names from texture IDs. The remaining gaps are tokens/emblems without Scryfall MTGO IDs, and `<strtable:N>` placeholders that get overwritten by Scryfall names when available.

- [ ] **Step 2: Commit**

```bash
git add KNOWN_ISSUES.md
git commit -m "docs: update known issues - card names now resolved via Scryfall"
```
