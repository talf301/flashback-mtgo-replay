using FlashbackRecorder;
using FlashbackRecorder.Models;
using Xunit;

namespace FlashbackRecorder.Tests;

public class GameSessionManagerTests
{
    private readonly MockMtgoClient _client = new();
    private readonly List<ReplayData> _replays = new();

    private GameSessionManager CreateManager(
        Func<int, DeckList?>? deckListProvider = null,
        Func<int, int, Dictionary<string, object>>? snapshotProvider = null)
    {
        return new GameSessionManager(
            _client,
            replay => _replays.Add(replay),
            deckListProvider,
            snapshotProvider);
    }

    private static List<PlayerInfo> TestPlayers() => new()
    {
        new PlayerInfo { Name = "Alice", Seat = 0 },
        new PlayerInfo { Name = "Bob", Seat = 1 },
    };

    // ── Normal game lifecycle ──

    [Fact]
    public void NormalGameLifecycle_ProducesCompleteReplay()
    {
        using var manager = CreateManager();

        // Start game
        _client.SimulateGameStart(gameId: 1001, players: TestPlayers(), format: "Modern");
        Assert.NotNull(manager.CurrentSession);
        Assert.Equal(1001, manager.CurrentSession!.GameId);
        Assert.Equal(1, manager.CurrentSession.GameNumber);

        // Turn 1
        _client.SimulateTurnChange(1, activePlayerSeat: 0, "Alice");
        _client.SimulatePhaseChange("precombat_main", 0);
        _client.SimulateGameAction("CastSpell", 42, "Lightning Bolt", 0);
        _client.SimulateLifeChange(1, 20, 17, "Lightning Bolt");

        // Turn 2
        _client.SimulateTurnChange(2, activePlayerSeat: 1, "Bob");
        _client.SimulateZoneChange(99, "Mountain", "Library", "Hand", 1);

        // End game
        _client.SimulateGameEnd(1001, winner: "Alice", reason: "concession");

        Assert.Single(_replays);
        var replay = _replays[0];
        Assert.Equal("3.0", replay.Version);
        Assert.True(replay.Header.Complete);
        Assert.Equal(1001, replay.Header.GameId);
        Assert.Equal(1, replay.Header.GameNumber);
        Assert.Equal("Alice", replay.Header.Result?.Winner);
        Assert.Equal("concession", replay.Header.Result?.Reason);
        Assert.Null(manager.CurrentSession);
    }

    [Fact]
    public void NormalGame_AccumulatesEventsInTimeline()
    {
        using var manager = CreateManager();

        _client.SimulateGameStart(1001);
        _client.SimulateTurnChange(1, 0, "Alice");
        _client.SimulateGameAction("CastSpell", 42, "Lightning Bolt", 0);
        _client.SimulateLifeChange(1, 20, 17);
        _client.SimulateZoneChange(42, "Lightning Bolt", "Hand", "Stack", 0);
        _client.SimulateGameEnd(1001, "Alice");

        var timeline = _replays[0].Timeline;

        // Should have: snapshot (turn 1), CastSpell event, LifeChange event, ZoneTransition event
        Assert.Equal(4, timeline.Count);
        Assert.Equal("snapshot", timeline[0].Type);
        Assert.Equal("event", timeline[1].Type);
        Assert.Equal("CastSpell", timeline[1].Event!.Type);
        Assert.Equal("event", timeline[2].Type);
        Assert.Equal("LifeChange", timeline[2].Event!.Type);
        Assert.Equal("event", timeline[3].Type);
        Assert.Equal("ZoneTransition", timeline[3].Event!.Type);
    }

    [Fact]
    public void NormalGame_EventsCarryTurnAndPhaseContext()
    {
        using var manager = CreateManager();

        _client.SimulateGameStart(1001);
        _client.SimulateTurnChange(1, 0, "Alice");
        _client.SimulatePhaseChange("combat", 0);
        _client.SimulateGameAction("CastSpell", 42, "Lightning Bolt", 0);
        _client.SimulateGameEnd(1001);

        var evt = _replays[0].Timeline.Last(e => e.Type == "event").Event!;
        Assert.Equal(1, evt.Turn);
        Assert.Equal("combat", evt.Phase);
        Assert.Equal("Alice", evt.ActivePlayer);
    }

    // ── Players and format population ──

    [Fact]
    public void GameStart_PopulatesPlayersFromEventArgs()
    {
        using var manager = CreateManager();

        var players = TestPlayers();
        _client.SimulateGameStart(1001, players: players, format: "Modern");
        _client.SimulateGameEnd(1001, "Alice");

        var replay = _replays[0];
        Assert.Equal(2, replay.Header.Players.Count);
        Assert.Equal("Alice", replay.Header.Players[0].Name);
        Assert.Equal(0, replay.Header.Players[0].Seat);
        Assert.Equal("Bob", replay.Header.Players[1].Name);
        Assert.Equal(1, replay.Header.Players[1].Seat);
    }

    [Fact]
    public void GameStart_PopulatesFormatFromEventArgs()
    {
        using var manager = CreateManager();

        _client.SimulateGameStart(1001, players: TestPlayers(), format: "Modern");
        _client.SimulateGameEnd(1001);

        Assert.Equal("Modern", _replays[0].Header.Format);
    }

    [Fact]
    public void GameStart_NullPlayers_DefaultsToEmptyList()
    {
        using var manager = CreateManager();

        _client.SimulateGameStart(1001);
        _client.SimulateGameEnd(1001);

        Assert.Empty(_replays[0].Header.Players);
    }

    // ── Card catalog ──

    [Fact]
    public void AssembleReplay_IncludesCardCatalog()
    {
        _client.CardCatalog = new Dictionary<string, CardCatalogEntry>
        {
            ["42"] = new CardCatalogEntry { Name = "Lightning Bolt", ManaCost = "{R}", TypeLine = "Instant" },
            ["99"] = new CardCatalogEntry { Name = "Mountain", TypeLine = "Basic Land — Mountain" },
        };

        using var manager = CreateManager();

        _client.SimulateGameStart(1001);
        _client.SimulateGameEnd(1001, "Alice");

        var catalog = _replays[0].CardCatalog;
        Assert.Equal(2, catalog.Count);
        Assert.Equal("Lightning Bolt", catalog["42"].Name);
        Assert.Equal("{R}", catalog["42"].ManaCost);
        Assert.Equal("Mountain", catalog["99"].Name);
    }

    // ── Deck list capture ──

    [Fact]
    public void GameStart_CapturesDeckList()
    {
        var deckList = new DeckList
        {
            Mainboard = new List<string> { "Lightning Bolt", "Lightning Bolt", "Snapcaster Mage" },
            Sideboard = new List<string> { "Rest in Peace", "Wear // Tear" },
        };

        using var manager = CreateManager(
            deckListProvider: gameId => deckList);

        _client.SimulateGameStart(1001);
        _client.SimulateGameEnd(1001, "Alice");

        Assert.NotNull(_replays[0].Header.DeckList);
        Assert.Equal(3, _replays[0].Header.DeckList!.Mainboard.Count);
        Assert.Equal(2, _replays[0].Header.DeckList!.Sideboard.Count);
        Assert.Contains("Lightning Bolt", _replays[0].Header.DeckList!.Mainboard);
    }

    [Fact]
    public void GameStart_NoDeckListProvider_NoDeckList()
    {
        using var manager = CreateManager();

        _client.SimulateGameStart(1001);
        _client.SimulateGameEnd(1001);

        Assert.Null(_replays[0].Header.DeckList);
    }

    // ── Snapshot capture ──

    [Fact]
    public void TurnStart_CapturesSnapshot()
    {
        var snapshotState = new Dictionary<string, object>
        {
            ["battlefield"] = new List<string> { "Mountain", "Lightning Bolt" },
            ["life"] = 20,
        };

        using var manager = CreateManager(
            snapshotProvider: (gameId, turn) => snapshotState);

        _client.SimulateGameStart(1001);
        _client.SimulateTurnChange(1, 0, "Alice");
        _client.SimulateTurnChange(2, 1, "Bob");
        _client.SimulateGameEnd(1001);

        var snapshots = _replays[0].Timeline.Where(e => e.Type == "snapshot").ToList();
        Assert.Equal(2, snapshots.Count);

        Assert.Equal(1, snapshots[0].Snapshot!.Turn);
        Assert.Equal("Alice", snapshots[0].Snapshot!.ActivePlayer);
        Assert.Equal(2, snapshots[1].Snapshot!.Turn);
        Assert.Equal("Bob", snapshots[1].Snapshot!.ActivePlayer);

        // State should contain the mock data
        Assert.True(snapshots[0].Snapshot!.State.ContainsKey("battlefield"));
    }

    // ── MTGO crash mid-game ──

    [Fact]
    public void MtgoCrashMidGame_ProducesIncompleteReplay()
    {
        using var manager = CreateManager();

        _client.SimulateGameStart(1001);
        _client.SimulateTurnChange(1, 0, "Alice");
        _client.SimulateGameAction("CastSpell", 42, "Lightning Bolt", 0);

        // Simulate MTGO crash — force end the session
        manager.ForceEndSession("crash");

        Assert.Single(_replays);
        Assert.False(_replays[0].Header.Complete);
        Assert.Equal("crash", _replays[0].Header.Result?.Reason);
        Assert.Null(_replays[0].Header.Result?.Winner);
        Assert.Null(manager.CurrentSession);
    }

    [Fact]
    public void DisconnectMidGame_ProducesIncompleteReplay()
    {
        using var manager = CreateManager();

        _client.SimulateGameStart(1001);
        _client.SimulateTurnChange(1, 0, "Alice");

        manager.ForceEndSession("disconnect");

        Assert.Single(_replays);
        Assert.False(_replays[0].Header.Complete);
        Assert.Equal("disconnect", _replays[0].Header.Result?.Reason);
    }

    [Fact]
    public void DisposeDuringActiveSession_SavesIncomplete()
    {
        var manager = CreateManager();

        _client.SimulateGameStart(1001);
        _client.SimulateTurnChange(1, 0, "Alice");

        // Dispose while game is active
        manager.Dispose();

        Assert.Single(_replays);
        Assert.False(_replays[0].Header.Complete);
        Assert.Equal("shutdown", _replays[0].Header.Result?.Reason);
    }

    // ── Multi-game match with _g1/_g2 tracking ──

    [Fact]
    public void MultiGameMatch_TracksGameNumbers()
    {
        using var manager = CreateManager();

        // Game 1
        _client.SimulateGameStart(1001);
        _client.SimulateTurnChange(1, 0, "Alice");
        _client.SimulateGameEnd(1001, "Alice", "life");

        // Game 2
        _client.SimulateGameStart(1002);
        _client.SimulateTurnChange(1, 1, "Bob");
        _client.SimulateGameEnd(1002, "Bob", "concession");

        // Game 3
        _client.SimulateGameStart(1003);
        _client.SimulateTurnChange(1, 0, "Alice");
        _client.SimulateGameEnd(1003, "Alice", "life");

        Assert.Equal(3, _replays.Count);
        Assert.Equal(1, _replays[0].Header.GameNumber);
        Assert.Equal(2, _replays[1].Header.GameNumber);
        Assert.Equal(3, _replays[2].Header.GameNumber);

        Assert.Equal(1001, _replays[0].Header.GameId);
        Assert.Equal(1002, _replays[1].Header.GameId);
        Assert.Equal(1003, _replays[2].Header.GameId);

        Assert.All(_replays, r => Assert.True(r.Header.Complete));
        Assert.Equal(3, manager.GamesRecorded);
    }

    [Fact]
    public void MultiGameMatch_CrashDuringGame2_ContinuesWithGame3()
    {
        using var manager = CreateManager();

        // Game 1 — normal
        _client.SimulateGameStart(1001);
        _client.SimulateGameEnd(1001, "Alice");

        // Game 2 — crash
        _client.SimulateGameStart(1002);
        _client.SimulateTurnChange(1, 0, "Alice");
        manager.ForceEndSession("crash");

        // Game 3 — normal (MTGO restarted)
        _client.SimulateGameStart(1003);
        _client.SimulateGameEnd(1003, "Bob");

        Assert.Equal(3, _replays.Count);
        Assert.True(_replays[0].Header.Complete);
        Assert.False(_replays[1].Header.Complete);
        Assert.True(_replays[2].Header.Complete);

        Assert.Equal(1, _replays[0].Header.GameNumber);
        Assert.Equal(2, _replays[1].Header.GameNumber);
        Assert.Equal(3, _replays[2].Header.GameNumber);
    }

    // ── Edge cases ──

    [Fact]
    public void GameEndWithoutStart_DoesNothing()
    {
        using var manager = CreateManager();

        // Fire end without start — should not crash or produce output
        _client.SimulateGameEnd(1001, "Alice");

        Assert.Empty(_replays);
    }

    [Fact]
    public void EventsWithoutActiveSession_AreIgnored()
    {
        using var manager = CreateManager();

        // Fire events without an active game session
        _client.SimulateTurnChange(1, 0, "Alice");
        _client.SimulatePhaseChange("combat", 0);
        _client.SimulateGameAction("CastSpell", 42, "Lightning Bolt", 0);
        _client.SimulateLifeChange(0, 20, 17);
        _client.SimulateZoneChange(42, "Lightning Bolt", "Hand", "Stack", 0);

        Assert.Empty(_replays);
    }

    [Fact]
    public void NewGameStartWhileOldGameActive_FinalizesOldAsIncomplete()
    {
        using var manager = CreateManager();

        // Start game 1 but don't end it
        _client.SimulateGameStart(1001);
        _client.SimulateTurnChange(1, 0, "Alice");

        // Start game 2 — should force-end game 1
        _client.SimulateGameStart(1002);

        Assert.Single(_replays);
        Assert.False(_replays[0].Header.Complete);
        Assert.Equal(1001, _replays[0].Header.GameId);
        Assert.Equal("interrupted", _replays[0].Header.Result?.Reason);

        // Game 2 is now the active session
        Assert.NotNull(manager.CurrentSession);
        Assert.Equal(1002, manager.CurrentSession!.GameId);
    }

    [Fact]
    public void Concession_RecordedAsNormalGameEnd()
    {
        using var manager = CreateManager();

        _client.SimulateGameStart(1001);
        _client.SimulateTurnChange(1, 0, "Alice");
        _client.SimulateGameEnd(1001, "Alice", "concession");

        Assert.Single(_replays);
        Assert.True(_replays[0].Header.Complete);
        Assert.Equal("Alice", _replays[0].Header.Result?.Winner);
        Assert.Equal("concession", _replays[0].Header.Result?.Reason);
    }

    [Fact]
    public void LifeChangeEvent_CapturesSourceInfo()
    {
        using var manager = CreateManager();

        _client.SimulateGameStart(1001);
        _client.SimulateTurnChange(1, 0, "Alice");
        _client.SimulateLifeChange(1, 20, 17, "Lightning Bolt");
        _client.SimulateGameEnd(1001);

        var lifeEvent = _replays[0].Timeline
            .First(e => e.Type == "event" && e.Event!.Type == "LifeChange")
            .Event!;

        Assert.Equal(17, (int)lifeEvent.Data["newLife"]);
        Assert.Equal(20, (int)lifeEvent.Data["oldLife"]);
        Assert.Equal("Lightning Bolt", (string)lifeEvent.Data["source"]);
    }

    [Fact]
    public void GameActionEvent_CapturesCardData()
    {
        using var manager = CreateManager();

        _client.SimulateGameStart(1001);
        _client.SimulateTurnChange(1, 0, "Alice");
        _client.SimulateGameAction("CastSpell", 42, "Lightning Bolt", 0);
        _client.SimulateGameEnd(1001);

        var actionEvent = _replays[0].Timeline
            .First(e => e.Type == "event" && e.Event!.Type == "CastSpell")
            .Event!;

        Assert.Equal(42, (int)actionEvent.Data["cardId"]);
        Assert.Equal("Lightning Bolt", (string)actionEvent.Data["cardName"]);
        Assert.Equal(0, (int)actionEvent.Data["playerSeat"]);
    }
}
