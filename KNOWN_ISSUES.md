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

## ~~P1: active_player Tracking~~ — FIXED

**Was:** Every action showed `active_player: "player_0"`. Priority-based signals (`prompted_player`, `player_waiting_for`) tracked who had priority, not whose turn it was — unreliable due to player-configured stop settings.

**Fixed:** Two new game-level messages decoded:
1. **`MASTER_USER_LIST` (opcode 4356)** provides the seat index → player name mapping (e.g., seat 0 = "coreyabaker", seat 1 = "TalTheTurtle").
2. **`NEW_USER_CHAT` (opcode 4355)** contains the in-game chat log, including authoritative `"Turn N: PlayerName"` messages that definitively identify whose turn it is.

The decode pipeline now uses chat-based turn ownership instead of priority signals. Player names are resolved to seat indices via the UserList mapping. The `PlayerStatus.active_player` byte (always 0 in practice) and `TurnStep.prompted_player` are no longer used for turn tracking.

---

## ~~P2: Duplicate PhaseChange Events~~ — FIXED

Fixed by tracking `last_emitted_phase` in `ReplayTranslator`, independent of the `prev` state snapshot. PhaseChange is only emitted when advancing to a phase that differs from the last emitted one. Resets on turn change and game reset.

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
