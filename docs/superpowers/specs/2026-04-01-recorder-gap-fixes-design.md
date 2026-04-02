# Recorder Gap Fixes — Design Spec

Close all 12 known recorder issues to produce usable `.flashback` replay files from live MTGO games, plus add a bundled demo replay to the viewer.

## Context

The recorder scaffolding is complete — GameSessionManager, FileWriter, TrayApp, Settings, and 48 tests all work. But `MtgoClient.cs` has placeholder event handlers, the providers (snapshot, deck list, card catalog, players) are never wired, and the serialized output doesn't conform to the v3 schema. The web viewer is feature-complete.

All fixes target the recorder's MTGOSDK integration layer and serialization, plus one viewer-side addition (demo replay).

## Decisions

- **Fix MtgoClient in-place.** No new abstractions or helper classes. All SDK-touching code stays in one file.
- **Schema conformance fixes in serialization layer only.** Internal models keep idiomatic C# (camelCase, dictionaries). `FileWriter.ConvertToFileFormat()` handles the mapping to v3 on-disk format.
- **Counter counts via grouping repeated enum entries.** `GameCard.Counters` returns `IEnumerable<CardCounter>` (enum, no count). Group and count: `card.Counters.GroupBy(c => c).ToDictionary(g => g.Key.ToString(), g => g.Count())`. Fall back to count=1 per entry if the SDK doesn't repeat entries.
- **Delete `KNOWN_ISSUES.md`.** The two web viewer items are stale (counters implemented, dynamic zones by-design). The 12 recorder items are captured here.

## Section 1: Quick Fixes (Issues #5, #1)

### Version String (#5)

Change `ReplayData.Version` default from `"3.0"` to `"3"` in `recorder/Models/ReplayData.cs`. Same change in `ReplayFileFormat.Version` default in `recorder/Models/ReplayFormat.cs`. Update any tests asserting `"3.0"`.

### FileWriter Wiring (#1)

In `Program.cs`, create a `FileWriter` instance using `settings.OutputDirectory` and call `fileWriter.WriteReplay(replay)` inside the `onReplayComplete` callback, before `trayApp.OnGameSaved(replay)`:

```csharp
var fileWriter = new FileWriter(settings.OutputDirectory);

using var sessionManager = new GameSessionManager(
    client,
    onReplayComplete: replay =>
    {
        fileWriter.WriteReplay(replay);
        trayApp.OnGameSaved(replay);
    });
```

### Files changed

- `recorder/Models/ReplayData.cs` — version default
- `recorder/Models/ReplayFormat.cs` — version default
- `recorder/Program.cs` — FileWriter instantiation and call
- `recorder/Tests/FileWriterTests.cs` — update version assertions
- `recorder/Tests/GameSessionManagerTests.cs` — update version assertions if any

## Section 2: Schema Conformance (Issues #6, #7, #8)

All changes in the serialization layer. Internal models (`GameEvent`, `ReplayData`) stay as-is.

### Event Data Flattening (#6)

The current `EventPayloadFormat` wraps event fields under a `data` key:

```json
{"type": "event", "event": {"type": "ZoneTransition", "data": {"card_id": "..."}}}
```

The v3 schema expects fields at the top level of the event object:

```json
{"type": "event", "event": {"type": "ZoneTransition", "card_id": "..."}}
```

Replace the `Data` dictionary on `EventPayloadFormat` with `[JsonExtensionData]` or explicit nullable properties with `[JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]`. The simplest approach: use `[JsonExtensionData]` to merge the dictionary fields into the top level:

```csharp
public class EventPayloadFormat
{
    [JsonPropertyName("type")]
    public required string Type { get; init; }

    [JsonPropertyName("timestamp")]
    public string Timestamp { get; init; } = "";

    [JsonExtensionData]
    public Dictionary<string, JsonElement>? Fields { get; init; }
}
```

`FileWriter.ConvertTimeline()` converts the internal `Dictionary<string, object>` data into `Fields` with snake_case keys and resolved values.

### Field Naming (#7) and Zone Transition Fields (#8)

Handled in `FileWriter.ConvertTimeline()` during the data → Fields conversion. Mapping:

| Internal key | On-disk key |
|---|---|
| `cardId` | `card_id` |
| `cardName` | `card_name` |
| `playerSeat` | `player` (resolved to player name via header) |
| `ownerSeat` | `owner` (resolved to player name via header) |
| `sourceZone` | `from_zone` |
| `destinationZone` | `to_zone` |
| `oldLife` | `old_life` |
| `newLife` | `new_life` |
| `abilityText` | `ability_text` |
| `actionType` | `action_type` |

Seat-to-name resolution uses `ReplayData.Header.Players` — `ConvertTimeline` receives the header for this lookup.

### Files changed

- `recorder/Models/ReplayFormat.cs` — `EventPayloadFormat` restructured
- `recorder/FileWriter.cs` — `ConvertTimeline()` field mapping logic
- `recorder/Tests/FileWriterTests.cs` — update assertions for flat structure and snake_case

## Section 3: Player & Format Population (Issues #2, #9)

### Players (#2)

Add a `Players` property to `GameStatusChangeEventArgs` in `IMtgoClient.cs`:

```csharp
public class GameStatusChangeEventArgs : EventArgs
{
    public required GameStatus Status { get; init; }
    public int GameId { get; init; }
    public string? WinnerName { get; init; }
    public string? Reason { get; init; }
    public List<PlayerInfo>? Players { get; init; }  // NEW
    public string? Format { get; init; }              // NEW (issue #9)
}
```

In `MtgoClient.OnSdkGameJoined`, populate from `game.Players`:

```csharp
Players = game.Players.Select((p, i) => new PlayerInfo
{
    Name = p.Name,
    Seat = i
}).ToList()
```

`GameSessionManager.StartNewGame` sets `_currentSession.Players` from the event args.

### Format (#9)

The `EventManager.GameJoined` callback signature is `(Event playerEvent, Game game)`. The `Event` parameter has `Format.Name`. Set `Format = playerEvent.Format?.Name` on the `GameStatusChangeEventArgs`. `GameSessionManager` passes it through to `GameSession` and it flows into `ReplayHeader.Format`.

### Files changed

- `recorder/IMtgoClient.cs` — add `Players` and `Format` to `GameStatusChangeEventArgs`
- `recorder/MtgoClient.cs` — populate both in `OnSdkGameJoined`
- `recorder/GameSessionManager.cs` — read from event args in `StartNewGame`
- `recorder/Tests/GameSessionManagerTests.cs` — update test event args, verify players/format flow

## Section 4: Event Handler Fixes (Issue #3)

Four handlers in `MtgoClient.SubscribeToGameEvents` need real SDK values.

### OnTurnChange

Read from the game instance directly:

```csharp
game.CurrentTurnChanged += (GameEventArgs args) =>
{
    OnTurnChange?.Invoke(this, new TurnChangeEventArgs
    {
        TurnNumber = game.CurrentTurn,
        ActivePlayerSeat = GetSeatIndex(game, game.ActivePlayer),
        ActivePlayerName = game.ActivePlayer?.Name ?? "",
    });
};
```

### OnGamePhaseChange

`CurrentPlayerPhase` is a record with `ActivePlayer` and `CurrentPhase`:

```csharp
game.OnGamePhaseChange += (CurrentPlayerPhase phase) =>
{
    OnGamePhaseChange?.Invoke(this, new GamePhaseChangeEventArgs
    {
        Phase = phase.CurrentPhase.ToString(),
        ActivePlayerSeat = GetSeatIndex(game, phase.ActivePlayer),
    });
};
```

### OnLifeChange

Add a `Dictionary<string, int> _previousLife` field on MtgoClient. Initialize from `game.Players` in `OnSdkGameJoined`. On each event, emit cached previous value, then update cache:

```csharp
game.OnLifeChange += (GamePlayer player) =>
{
    _previousLife.TryGetValue(player.Name, out var oldLife);
    OnLifeChange?.Invoke(this, new LifeChangeEventArgs
    {
        PlayerSeat = GetSeatIndex(game, player),
        OldLife = oldLife,
        NewLife = player.Life,
        Source = null,  // SDK doesn't expose life change source
    });
    _previousLife[player.Name] = player.Life;
};
```

### OnGameAction

Check for `CardAction` subclass to get the source card:

```csharp
game.OnGameAction += (GameAction action) =>
{
    var cardAction = action as CardAction;
    OnGameAction?.Invoke(this, new GameActionEventArgs
    {
        ActionType = action.Type.ToString(),
        CardId = cardAction?.Card?.Id ?? action.ActionId,
        CardName = cardAction?.Card?.Name ?? action.Name,
        PlayerSeat = cardAction?.Card != null
            ? GetSeatIndex(game, cardAction.Card.Controller)
            : 0,
        AbilityText = action.Name,
        SourceZone = cardAction?.Card?.Zone?.ToString(),
    });
};
```

### Known limitation

`LifeChangeEventArgs.Source` stays null — the SDK doesn't expose what caused the life change.

### Files changed

- `recorder/MtgoClient.cs` — all four handlers rewritten, `_previousLife` field added

## Section 5: Card Catalog Population (Issue #4)

### Approach

Build the catalog incrementally as cards are encountered. Add a `Dictionary<string, CardCatalogEntry> _cardCatalog` field on MtgoClient, cleared at each game start.

Add a private method:

```csharp
private void TryCaptureCardMetadata(GameCard card)
{
    var id = card.Id.ToString();
    if (_cardCatalog.ContainsKey(id)) return;

    var def = card.Definition;
    _cardCatalog[id] = new CardCatalogEntry
    {
        Name = card.Name,
        ManaCost = def?.ManaCost,
        TypeLine = def != null
            ? string.Join(" ", def.Types) + (def.Subtypes.Count > 0
                ? " \u2014 " + string.Join(" ", def.Subtypes)
                : "")
            : null,
    };
}
```

Call `TryCaptureCardMetadata` from within MtgoClient's SDK callback closures (which have access to the `GameCard` objects), before emitting the DTO events:
- `OnZoneChange` closure — has the `GameCard card` parameter
- `OnGameAction` closure — has `CardAction.Card` when available
- `CaptureSnapshot()` — iterates all visible cards on the board

### Integration with GameSessionManager

Add a `GetCardCatalog()` method to `IMtgoClient`:

```csharp
Dictionary<string, CardCatalogEntry> GetCardCatalog();
```

`GameSessionManager.AssembleReplay` calls `_client.GetCardCatalog()` to populate `ReplayData.CardCatalog`. This requires adding the `IMtgoClient` reference to `AssembleReplay` (currently static — make it an instance method, or pass the catalog as a parameter to `EndCurrentGame`).

### Files changed

- `recorder/MtgoClient.cs` — `_cardCatalog` field, `TryCaptureCardMetadata`, `GetCardCatalog()`
- `recorder/IMtgoClient.cs` — `GetCardCatalog()` method on interface
- `recorder/GameSessionManager.cs` — call `GetCardCatalog()` during assembly
- `recorder/Tests/MockMtgoClient.cs` — implement `GetCardCatalog()` stub
- `recorder/Tests/GameSessionManagerTests.cs` — verify catalog flows through

## Section 6: Snapshot Provider (Issue #11)

The most substantial piece. Reads full board state from the `Game` instance.

### Wiring

Store the `Game` reference as a field on MtgoClient (set in `OnSdkGameJoined`). Add a `CaptureSnapshot(int turn)` method that returns `Dictionary<string, object>` matching the v3 snapshot state schema. Wire as the `snapshotProvider` delegate in `Program.cs`.

### What it reads

**Per player** (from `game.Players`):
- `player.Life`
- `player.ManaPool` — iterate `Mana` entries, group by `Color` to produce `{W: n, U: n, B: n, R: n, G: n, C: n}`
- Zones via `game.GetGameZone(player, CardZone.Hand)`, `.Battlefield`, `.Graveyard`, `.Exile`
- Library: count only (`player.LibraryCount`)

**Per card** in each zone:
- `card.Id.ToString()` — instance ID
- `card.Id.ToString()` — catalog_id (same, since catalog is keyed by instance ID)
- `card.IsTapped`
- `card.IsFlipped` — maps to `face_down` in output (MTGO uses "flipped" for face-down cards)
- `card.Power`, `card.Toughness` — current values
- `card.Damage`
- `card.Counters` — `card.Counters.GroupBy(c => c).ToDictionary(g => g.Key.ToString(), g => g.Count())`
- `card.Associations` filtered for `CardAssociation.EquippedTo` / `EquippedWith` — attachment IDs
- `card.IsAttacking`, `card.IsBlocking`, `card.AttackingOrders`, `card.BlockingOrders` — combat status
- `card.HasSummoningSickness`
- `card.Controller.Name` — only included if different from `card.Owner.Name`

Also calls `TryCaptureCardMetadata(card)` for every card encountered.

**Shared zones** via `game.SharedZones` for exile (if shared) and stack.

**Active/priority player** from `game.ActivePlayer?.Name` and `game.PriorityPlayer?.Name`.

### Output structure

```
{
  "players": [
    {
      "name": "Alice",
      "seat": 0,
      "life": 20,
      "mana_pool": { "W": 0, "U": 0, "B": 0, "R": 0, "G": 0, "C": 0 },
      "zones": {
        "hand": { "cards": [...], "count": 3 },
        "battlefield": { "cards": [...] },
        "graveyard": { "cards": [...] },
        "exile": { "cards": [...] },
        "library": { "cards": [], "count": 45 }
      }
    }
  ],
  "active_player": "Alice",
  "priority_player": "Alice"
}
```

Each card object in a zone:

```
{
  "id": "12345",
  "catalog_id": "12345",
  "tapped": false,
  "flipped": false,
  "face_down": false,
  "power": 2,
  "toughness": 2,
  "damage": 0,
  "counters": { "+1/+1": 2 },
  "attachments": ["67890"],
  "combat_status": { "attacking": true, "target": "Bob" },
  "summoning_sickness": false,
  "controller": "Bob"
}
```

### Files changed

- `recorder/MtgoClient.cs` — `_currentGame` field, `CaptureSnapshot()` method
- `recorder/IMtgoClient.cs` — `CaptureSnapshot(int turn)` on interface (or just wire via delegate)
- `recorder/Program.cs` — wire snapshot provider delegate
- `recorder/Tests/MockMtgoClient.cs` — implement stub if added to interface

## Section 7: Deck List Capture (Issue #10)

### Approach

Store the `Event` reference from `EventManager.GameJoined` as a field on MtgoClient. Add a `CaptureDeckList()` method:

```csharp
public DeckList? CaptureDeckList()
{
    var deck = _currentEvent?.RegisteredDeck;
    if (deck == null) return null;

    return new DeckList
    {
        Mainboard = deck.Mainboard.Select(c => c.Name).ToList(),
        Sideboard = deck.Sideboard.Select(c => c.Name).ToList(),
    };
}
```

Wire in `Program.cs` as the `deckListProvider` delegate:

```csharp
using var sessionManager = new GameSessionManager(
    client,
    onReplayComplete: replay => { ... },
    deckListProvider: _ => client.CaptureDeckList(),
    snapshotProvider: (gameId, turn) => client.CaptureSnapshot(turn));
```

The `Deck` type's exact property names for mainboard/sideboard need verification at implementation time — the SDK may use `Cards`, `MainDeck`, or similar.

Sideboard changes for games 2+ are handled by comparing deck lists between games — already supported structurally by `GameSessionManager` and the viewer.

### Files changed

- `recorder/MtgoClient.cs` — `_currentEvent` field, `CaptureDeckList()`
- `recorder/IMtgoClient.cs` — `CaptureDeckList()` on interface (or wire via delegate)
- `recorder/Program.cs` — wire deck list provider delegate

## Section 8: Default Demo Replay (Issue #12)

### Approach

A viewer feature. Ship a hand-crafted `.flashback` file as a static asset.

### The fixture

A realistic ~50-action game between "Alice" and "Bob" in Modern format. Contents:
- Header with players, format, timestamps, result (Alice wins)
- Deck list (15-20 representative Modern cards)
- 2-3 snapshots (turns 1, 3, 5)
- ~50 events: draws, land plays, spell casts, attacks/blocks, life changes, zone transitions
- Card catalog with ~15-20 entries (real card names, mana costs, type lines)

Located at `web/public/demo.flashback`.

### UI integration

In `FileLoader.tsx`, add a "Load Demo Replay" link/button in the empty/welcome state alongside the existing drag-and-drop area. Clicking it fetches `/demo.flashback` and loads it through the same `ReplayFile` parsing path as any user-provided file.

### Files changed

- `web/public/demo.flashback` — new fixture file
- `web/src/components/FileLoader.tsx` — "Load Demo" button
- `web/src/components/FileLoader.test.tsx` — test for demo loading

## KNOWN_ISSUES.md Cleanup

Delete `KNOWN_ISSUES.md` from the repo root. The two web viewer items are stale (counters are implemented, dynamic zones is by-design). All 12 recorder items are captured in this spec.

Update `DEVELOPMENT.md` if it references `KNOWN_ISSUES.md`.
