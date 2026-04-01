# Known Issues and Limitations

## Architecture Transition

The project has transitioned from a Rust-based network protocol decode pipeline to a C#/MTGOSDK-based recorder. The previous known issues related to the Rust decode pipeline (protocol decoding, StateBuf diffs, chat-based enrichment, phantom players, etc.) are no longer applicable.

See [the redesign spec](docs/superpowers/specs/2026-03-31-flashback-mtgosdk-redesign.md) for the new architecture.

---

## Web Viewer Gaps

- No counter type display on cards
- Zones are created dynamically from actions

---

## Recorder (In Development)

The recorder scaffolding is in place (interfaces, session management, file writer, tray app) but cannot produce usable replay files yet. The gaps below must be closed before end-to-end recording works.

### Critical — no output without these

1. **FileWriter never called.** `Program.cs` assembles `ReplayData` in the `onReplayComplete` callback but never instantiates or calls `FileWriter`. Replays are silently discarded.

2. **Players list never populated.** `GameSession.Players` is initialized as an empty list and nothing ever adds to it. `MtgoClient.OnSdkGameJoined` should extract player info from `game.Players` and emit an event (or the session manager should query it). Output will always have `players: []`, violating the schema.

3. **MtgoClient event handlers have placeholder values:**
   - `OnLifeChange`: `OldLife` and `NewLife` are both set to `player.Life` (current value), so the delta is always 0. Need to cache previous life totals per player.
   - `OnTurnChange` (`CurrentTurnChanged`): `TurnNumber`, `ActivePlayerSeat`, and `ActivePlayerName` are all hardcoded to `0`/`""`. `GameEventArgs` doesn't directly expose these; need to query `game` state.
   - `OnGamePhaseChange`: `ActivePlayerSeat` is hardcoded to `0`. Need to track active player from turn change events.
   - `OnGameAction`: `PlayerSeat` is hardcoded to `0`, `AbilityText` and `SourceZone` are always `null`. The SDK's `GameAction` type doesn't have `Card`/`Player`/`AbilityText` properties — these need different extraction logic.

4. **No card catalog population.** `ReplayData.CardCatalog` defaults to empty. No code reads card metadata (name, mana cost, type line) from MTGOSDK's `GameCard.Definition` or similar.

### Schema conformance issues

5. **Version string mismatch.** `ReplayData.Version` defaults to `"3.0"` but `v3.schema.json` requires exactly `"3"`.

6. **Event data structure mismatch.** The code wraps event payload fields under a `data` key (`{"type": "event", "event": {"type": "...", "data": {...}}}`). The schema expects fields at the top level of the `event` object (`{"type": "ZoneTransition", "card_id": "...", "from_zone": "..."}`).

7. **Event field naming.** Code uses camelCase (`cardId`, `cardName`, `playerSeat`, `ownerSeat`) and seat numbers. Schema uses snake_case (`card_id`, `player`) and player names.

8. **Zone transition field names.** Code emits `sourceZone`/`destinationZone`; schema expects `from_zone`/`to_zone`.

### Missing data extraction

9. **No game format extraction.** `ReplayHeader.Format` is never set. MTGOSDK's `Event` object (passed to `GameJoined` callback) likely has format info.

10. **No deck list capture.** The `deckListProvider` delegate exists in `GameSessionManager` but is never wired in `Program.cs`. No code reads deck data from MTGOSDK.

11. **No snapshot provider.** The `snapshotProvider` delegate exists but is never wired. Turn-start snapshots will have empty state. A real implementation needs to read life totals, mana pools, and zone contents (with card objects including tap state, power/toughness, counters, etc.) from the `Game` instance.

### Not blocking but worth noting

12. **No mock/demo mode.** The recorder requires a running MTGO process. There's a `MockMtgoClient` for tests but no CLI flag to generate sample replays for viewer development.
