# Known Issues and Limitations

Analysis based on decoding `golden_v1.bin` — a 12MB captured MTGO stream containing a 3-game Modern Bo3 match (10,195 messages → 623 actions → 11 turns across 3 games).

---

## Card Name Coverage

**Problem:** Only 25% of cards have names in the decoded output (46/187 unique card IDs).

**Root cause:** MTGO sends `CARDNAME_STRING` as a property on `ThingElement` objects, but only when the property is present in the state update. Cards that exist from the start of the capture (already on the battlefield or in hand when the first full-state snapshot arrives) often do not include the name property. It appears to be sent only when the card first enters the game state or when its properties change.

**What has names:**
- Fetchlands (Scalding Tarn, Flooded Strand, etc.) — re-created when cracked, so the new object includes the name
- Triggered abilities ("Triggered ability from Ragavan, Nimble Pilferer") — new stack objects always include the name
- Cards cast/played after the capture started

**What doesn't have names:**
- Cards on the battlefield at the start of the capture
- Cards in hand at the start of the capture
- Opening hand cards in general

**Additionally:** 9 cards have `<strtable:N>` placeholder names. The MTGO protocol uses a string table where `length == 0xFFFF` means "look up index N in a shared string table." We don't have the string table contents.

### Possible fixes

1. **CARDTEXTURE_NUMBER mapping**: Every `ThingElement` includes a `CARDTEXTURE_NUMBER` property (card art ID). Building a mapping from texture IDs to Scryfall card names would provide 100% name coverage. This requires:
   - Extracting the texture → card mapping from MTGO's data files, or
   - Building the mapping empirically by correlating known names with their texture IDs

2. **String table extraction**: Decompile the MTGO client to find how the string table is populated. It may be sent in a login/session message or loaded from local data files.

3. **Capture from game start**: If the capture hook is active before the game begins, the initial full-state snapshot may include all card names. This needs testing.

---

## Multi-Game Sessions

**Problem:** A Bo3 match produces all 3 games concatenated into a single flat action list. The viewer treats them as one continuous game.

**What happens:**
- Game 1: 262 actions, 9 turns
- Game 2: 98 actions, 4 turns
- Game 3: 263 actions, 11 turns
- Total: 623 actions displayed as a single replay

**Root cause:** The decoder processes `GameOver` messages and resets state, but all actions go into one `Vec<ReplayAction>`. The `ReplayFile` format supports only one game per file.

### Fix needed

- Detect game boundaries (GameOver messages, turn counter resets)
- Output separate `ReplayFile` per game, or add a game boundary marker to the action list
- The web viewer already handles turn/phase changes — it just needs game separators

---

## Empty Replay Header

**Problem:** The decoded replay has `game_id: "unknown"`, no players, and `result: "Incomplete"`.

**Root cause:** The pipeline builds the header from `final_state` at the end of processing. But `game_state` is reset to `None` on each `GameOver`, so after a 3-game match there's no state left.

### Fix needed

- Capture header info (game_id, player names, life totals) from the first game's state before it gets reset
- Detect game result from `GameResultsMessage` (opcode 4485) — wire layout needs decompiling
- Player names require `GsPlayerOrderMessage` (opcode 1155) — wire layout also undocumented

---

## Missing Action Types

**Not yet decoded:**

| Action | Why missing |
|--------|-------------|
| Token creation | Tokens appear as new Things with `IS_TOKEN` set, but the translator doesn't distinguish them from regular cards |
| Discard | Hand → Graveyard zone transitions exist but aren't labeled as "discard" specifically |
| Scry/Surveil | Library manipulation isn't visible (cards in library zone don't have individual identity until drawn) |
| Mulligan | Happens before game state tracking begins |
| Mana tapping | Taps are tracked but not correlated with mana production |

---

## StateBuf Diff Edge Cases

**7 decode errors** in the golden file — all "diff tail without prior state." These occur when the first `GamePlayStatusMessage` after a game reset is a diff (not a full state). The processor can't apply a diff without a prior state to diff against.

**Impact:** A few state updates are skipped at the start of games 2 and 3. Some early-game actions may be missed.

### Fix needed

- After a `GameOver` / state reset, buffer diff messages until a full-state (Head+Tail without diff flag) arrives
- Or: send a synthetic "request full state" if the first message is a diff

---

## Phase/Turn Tracking

Some actions in game 1 appear at `turn: 0, phase: "unknown(255)"` — these are from the initial state snapshot before any TurnStep element has been received. The web viewer handles this gracefully (displays the raw phase string).

Turn 1 shows duplicate phase changes (upkeep→precombat_main appears twice in game 1). This likely happens because two consecutive state updates both change the phase, producing two PhaseChange actions. Not harmful but noisy.

---

## Performance

The 623-action replay reconstructs quickly (sub-second), but the reconstructor replays all actions from the start for every step. For very large replays, incremental state caching would help. Not a concern at current scale.

---

## Web Viewer Gaps

- No Scryfall integration for card images yet (card IDs are MTGO thing IDs, not Scryfall IDs — needs CARDTEXTURE_NUMBER mapping)
- Zones are created dynamically from actions — if no action references a zone, it won't appear
- No deck list display
- No game result display (result is always "Incomplete")
- Combat is tracked per-card (attacking/blocking flags) but no combat grouping (attacker→blocker pairs)
