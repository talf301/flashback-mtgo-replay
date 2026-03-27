# Deduplicate PhaseChange Events Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate duplicate PhaseChange actions emitted when interleaved MTGO state updates repeat the same phase within a turn.

**Architecture:** Add a `last_emitted_phase` field to `ReplayTranslator` that tracks the most recently emitted phase independently of `prev` state. PhaseChange is only emitted when the phase differs from this field. The field resets on turn change and game reset. The display-layer dedup band-aid in `decode.rs` is then removed.

**Tech Stack:** Rust, existing test harness

---

### Task 1: Add `last_emitted_phase` field and dedup logic

**Files:**
- Modify: `src/translator.rs:22-32` (struct fields)
- Modify: `src/translator.rs:35-42` (constructor)
- Modify: `src/translator.rs:55-60` (reset method)
- Modify: `src/translator.rs:67-115` (process method)
- Test: `src/translator.rs` (inline tests)

- [ ] **Step 1: Write the failing test**

Add a test to `src/translator.rs` that feeds two consecutive states with the same phase (simulating interleaved updates) and asserts only one PhaseChange is emitted:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_duplicate_phase_change_suppressed -- --nocapture 2>&1 | tail -20`
Expected: FAIL — the second `process()` call emits a duplicate PhaseChange because `prev.phase` was overwritten.

- [ ] **Step 3: Add `last_emitted_phase` field to struct**

In `src/translator.rs`, add the field to `ReplayTranslator`:

```rust
pub struct ReplayTranslator {
    prev: Option<GameState>,
    player_names: Vec<String>,
    start_time: Option<DateTime<Utc>>,
    things_seen_on_stack: HashSet<u32>,
    last_known_zones: HashMap<u32, i32>,
    /// Tracks the last phase for which a PhaseChange action was emitted,
    /// so interleaved state updates at the same phase don't produce duplicates.
    last_emitted_phase: Option<GamePhase>,
}
```

Initialize in `new()`:

```rust
last_emitted_phase: None,
```

Clear in `reset()`:

```rust
self.last_emitted_phase = None;
```

- [ ] **Step 4: Add dedup logic in `process()`**

In `process()`, after `diff()` returns `actions`, scan for TurnChange and PhaseChange to update `last_emitted_phase`. Also gate PhaseChange emission by comparing against `last_emitted_phase`.

The cleanest approach: modify the PhaseChange condition in `diff()` to also take a `last_emitted_phase` parameter. In `diff()` at line 183, change:

```rust
if new.phase != prev.phase
    && (turn_changed || new.phase.ordinal() > prev.phase.ordinal())
```

to:

```rust
let dominated_by_prev = self.last_emitted_phase.as_ref()
    .map_or(false, |emitted| *emitted == new.phase);
if new.phase != prev.phase
    && !dominated_by_prev
    && (turn_changed || new.phase.ordinal() > prev.phase.ordinal())
```

Since `diff()` takes `&self`, it can read `last_emitted_phase` without mutation.

Then in `process()`, after computing `actions` but before returning, update `last_emitted_phase`:

```rust
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
```

Place this block in `process()` right after the `let actions = ...` block (after line 100) and before the `last_known_zones` update (line 103).

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test test_duplicate_phase_change_suppressed -- --nocapture 2>&1 | tail -20`
Expected: PASS

- [ ] **Step 6: Run all translator tests**

Run: `cargo test translator -- --nocapture 2>&1 | tail -30`
Expected: All existing tests pass (turn_and_phase_change, phase_regression, full_state, etc.)

- [ ] **Step 7: Commit**

```bash
git add src/translator.rs
git commit -m "fix: deduplicate PhaseChange events via last_emitted_phase tracking"
```

---

### Task 2: Remove display-layer dedup band-aid from decode.rs

**Files:**
- Modify: `src/bin/decode.rs:694-701`

- [ ] **Step 1: Simplify the PhaseChange display arm**

In `src/bin/decode.rs`, the `ActionType::PhaseChange` match arm (around line 695) currently deduplicates against `last_phase`. Simplify it to just update and print unconditionally:

```rust
ActionType::PhaseChange { phase } => {
    last_phase = phase.clone();
    println!("  [{}]", phase);
}
```

- [ ] **Step 2: Run golden pipeline test**

Run: `cargo test golden -- --nocapture 2>&1 | tail -30`
Expected: PASS — action count may decrease slightly due to fewer duplicate PhaseChanges. If the golden test has a hard-coded action count, update it.

- [ ] **Step 3: Run full decode on golden file to inspect output**

Run: `cargo run --bin decode -- test_data/golden_v1.bin 2>&1 | head -80`
Expected: No duplicate phase headers within a turn.

- [ ] **Step 4: Commit**

```bash
git add src/bin/decode.rs
git commit -m "refactor: remove display-layer PhaseChange dedup (now handled at source)"
```

---

### Task 3: Update KNOWN_ISSUES.md

**Files:**
- Modify: `KNOWN_ISSUES.md:44-58`

- [ ] **Step 1: Mark the issue as fixed**

Change the P2 section header and content:

```markdown
## ~~P2: Duplicate PhaseChange Events~~ — FIXED

Fixed by tracking `last_emitted_phase` in `ReplayTranslator`, independent of the `prev` state snapshot. PhaseChange is only emitted when advancing to a phase that differs from the last emitted one. Resets on turn change and game reset.
```

- [ ] **Step 2: Commit**

```bash
git add KNOWN_ISSUES.md
git commit -m "docs: mark duplicate PhaseChange events as fixed"
```
