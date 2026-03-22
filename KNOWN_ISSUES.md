# Known Issues and Limitations

Analysis based on decoding `golden_v1.bin` — a 12MB captured MTGO stream containing a 3-game Modern Bo3 match (10,195 messages → 550 actions across 3 games).

---

## ~~P0: 16 Phantom Players~~ — FIXED

Fixed by filtering `populated_players` to only include seats with nonzero life/hand/library.

---

## ~~P1: Turn 0 Bogus Actions~~ — FIXED

Fixed by suppressing all action emission in the translator while `turn == 0 && phase == Unknown(255)`. State tracking still runs so diffing works once real gameplay begins at turn 1.

---

## ~~P1: Empty Card Names and Textures~~ — MOSTLY FIXED

**Was:** `card_names` were empty for ~85% of cards. The MTGO `CARDNAME_STRING` property is only sent when a card first enters the game state or when properties change — cards present from the start of capture never received names.

**Fixed:** The decode pipeline now resolves card names from MTGO texture IDs via the Scryfall API (`POST /cards/collection` for batch, `GET /cards/mtgo/{id}` for large IDs). Coverage went from ~15% to ~99% across all games.

**Remaining gaps:**
- ~6 MTGO IDs (tokens, emblems) have no Scryfall entry — these remain unnamed
- `<strtable:N>` placeholders from the protocol's string table are overwritten by Scryfall names when the card has a texture ID; a few without texture IDs persist
- Use `--no-resolve` flag to skip Scryfall lookups for offline/testing use

---

## P1: active_player Tracking — PARTIALLY FIXED

**Was:** Every action showed `active_player: "player_0"`.

**Fixed:** Two issues resolved:
1. **PlayerStatus background_image_names** were parsed as a length-prefixed string array, but are actually 16 fixed-size 29-byte slots. The bogus "count" consumed all remaining bytes, making the ActivePlayer byte unreachable. Fixed to skip 16×29 bytes. (The ActivePlayer byte here is always 0 anyway.)
2. **TurnStep.prompted_player** at byte offset 24 now parsed correctly. Values 0/1 indicate which player has priority; 255 means no active prompt. Used to update `active_player` in game state.

**Remaining limitation:** `active_player` only changes when a player is explicitly prompted (combat phases, responses). During non-interactive phases, it defaults to the last prompted player (usually player_0). MTGO bundles both players' turns into a single turn number, with a phase regression marking the boundary. The `prompted_player` field is the only per-player signal in the protocol — no explicit "whose turn is it" field has been found.

---

## P2: Duplicate PhaseChange Events

**Problem:** The same phase gets emitted multiple times within a turn.

**Example from Game 1, Turn 1:**
```
[7]  precombat_main PhaseChange precombat_main
[8]  precombat_main PlayLand player_1 card=423
[9]  precombat_main PhaseChange precombat_main   ← duplicate
[10] precombat_main PlayLand player_0 card=425
```

**Root cause:** Multiple state updates arrive during the same phase. Each one where `new.phase != prev.phase` emits a PhaseChange — but `prev` reflects the *last processed state*, not the *last emitted phase*. Interleaved updates from both players' perspectives cause the phase to flip back and forth.

**Fix needed:** Track last-emitted phase separately from prev state, and only emit PhaseChange when advancing to a genuinely new phase.

---

## P2: Misclassified Zone Transitions

**Problem:** Zone transitions from unknown source zones show `from_zone: "unknown"`.

**Example:** `ZoneTransition card=431 from="unknown" to="revealed"`

**Root cause:** When a thing appears for the first time with `from_zone = Some(-1)` (the sentinel for "moved from somewhere"), we know it moved but don't know the source zone type. The `from_zone` field on `ThingElement` is an object reference, not a zone enum.

---

## P3: StateBuf Diff Edge Cases

7 decode errors in the golden file — "diff tail without prior state." These occur when the first `GamePlayStatusMessage` after a game reset is a diff. The processor can't apply a diff without a prior state.

**Impact:** A few state updates are skipped at the start of games 2 and 3.

---

## P3: Missing Action Types

| Action | Why missing |
|--------|-------------|
| Token creation | Tokens appear as new Things with `IS_TOKEN` set, but translator doesn't distinguish them |
| Discard | Hand → Graveyard transitions exist but aren't labeled as "discard" |
| Scry/Surveil | Library manipulation isn't visible |
| Mulligan | Happens before game state tracking begins |
| Mana tapping | Taps tracked but not correlated with mana production |

---

## P3: Web Viewer Gaps

- No Scryfall integration for card images (needs CARDTEXTURE_NUMBER → Scryfall mapping)
- Zones are created dynamically from actions
- No deck list display
- Combat grouping missing (attacker→blocker pairs)
