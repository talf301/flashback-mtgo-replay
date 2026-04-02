using FlashbackRecorder.Models;

namespace FlashbackRecorder;

/// <summary>
/// Manages game recording sessions. Listens for events from <see cref="IMtgoClient"/>
/// to detect game start/end, creates recording sessions, captures deck lists,
/// accumulates events, and triggers keyframe snapshots at each turn start.
/// </summary>
public sealed class GameSessionManager : IDisposable
{
    private readonly IMtgoClient _client;
    private readonly Action<ReplayData> _onReplayComplete;
    private readonly Func<int, DeckList?>? _deckListProvider;
    private readonly Func<int, int, Dictionary<string, object>>? _snapshotProvider;

    private GameSession? _currentSession;
    private int _gameCounter = 0;
    private bool _disposed;

    /// <summary>The currently active recording session, if any.</summary>
    public GameSession? CurrentSession => _currentSession;

    /// <summary>Number of games recorded in this match so far.</summary>
    public int GamesRecorded => _gameCounter;

    /// <summary>
    /// Creates a new GameSessionManager.
    /// </summary>
    /// <param name="client">MTGO client to subscribe to events from.</param>
    /// <param name="onReplayComplete">
    /// Callback invoked with assembled replay data when a game ends.
    /// The file writer (or test harness) receives this.
    /// </param>
    /// <param name="deckListProvider">
    /// Optional function that returns the deck list for a given game ID.
    /// In production, this polls MTGOSDK. In tests, returns mock data.
    /// </param>
    /// <param name="snapshotProvider">
    /// Optional function that returns a full board state snapshot for a given
    /// game ID and turn number. In production, polls MTGOSDK. In tests, returns mock data.
    /// </param>
    public GameSessionManager(
        IMtgoClient client,
        Action<ReplayData> onReplayComplete,
        Func<int, DeckList?>? deckListProvider = null,
        Func<int, int, Dictionary<string, object>>? snapshotProvider = null)
    {
        _client = client ?? throw new ArgumentNullException(nameof(client));
        _onReplayComplete = onReplayComplete ?? throw new ArgumentNullException(nameof(onReplayComplete));
        _deckListProvider = deckListProvider;
        _snapshotProvider = snapshotProvider;

        SubscribeToClientEvents();
    }

    // ── Event subscription ──

    private void SubscribeToClientEvents()
    {
        _client.OnGameStatusChange += HandleGameStatusChange;
        _client.OnTurnChange += HandleTurnChange;
        _client.OnGamePhaseChange += HandleGamePhaseChange;
        _client.OnZoneChange += HandleZoneChange;
        _client.OnGameAction += HandleGameAction;
        _client.OnLifeChange += HandleLifeChange;
    }

    private void UnsubscribeFromClientEvents()
    {
        _client.OnGameStatusChange -= HandleGameStatusChange;
        _client.OnTurnChange -= HandleTurnChange;
        _client.OnGamePhaseChange -= HandleGamePhaseChange;
        _client.OnZoneChange -= HandleZoneChange;
        _client.OnGameAction -= HandleGameAction;
        _client.OnLifeChange -= HandleLifeChange;
    }

    // ── Game lifecycle ──

    private void HandleGameStatusChange(object? sender, GameStatusChangeEventArgs e)
    {
        if (e.Status == GameStatus.Started)
        {
            StartNewGame(e);
        }
        else if (e.Status == GameStatus.Ended)
        {
            EndCurrentGame(e.WinnerName, e.Reason, complete: true);
        }
    }

    private void StartNewGame(GameStatusChangeEventArgs e)
    {
        // If there's an in-progress session (e.g. MTGO crashed and restarted),
        // finalize it as incomplete before starting a new one.
        if (_currentSession != null)
        {
            EndCurrentGame(winnerName: null, reason: "interrupted", complete: false);
        }

        _gameCounter++;

        _currentSession = new GameSession
        {
            GameId = e.GameId,
            GameNumber = _gameCounter,
            StartTime = DateTimeOffset.UtcNow,
        };

        // Populate players and format from event args.
        if (e.Players != null)
        {
            _currentSession.Players = e.Players;
        }
        _currentSession.Format = e.Format;

        // Capture deck list at game start.
        if (_deckListProvider != null)
        {
            _currentSession.DeckList = _deckListProvider(e.GameId);
        }
    }

    private void EndCurrentGame(string? winnerName, string? reason, bool complete)
    {
        if (_currentSession == null)
            return;

        var session = _currentSession;
        _currentSession = null;

        session.EndTime = DateTimeOffset.UtcNow;
        session.Complete = complete;
        session.WinnerName = winnerName;
        session.EndReason = reason;

        // Assemble replay data and hand it to the callback.
        var replay = AssembleReplay(session);
        _onReplayComplete(replay);
    }

    /// <summary>
    /// Force-close the current session as incomplete (e.g., on MTGO crash or disconnect).
    /// </summary>
    public void ForceEndSession(string reason = "crash")
    {
        EndCurrentGame(winnerName: null, reason: reason, complete: false);
    }

    // ── Turn and phase tracking ──

    private void HandleTurnChange(object? sender, TurnChangeEventArgs e)
    {
        if (_currentSession == null) return;

        _currentSession.CurrentTurn = e.TurnNumber;
        _currentSession.CurrentActivePlayer = e.ActivePlayerName;

        // Capture keyframe snapshot at each turn start.
        CaptureSnapshot(e.TurnNumber, _currentSession.CurrentPhase, e.ActivePlayerName);
    }

    private void HandleGamePhaseChange(object? sender, GamePhaseChangeEventArgs e)
    {
        if (_currentSession == null) return;

        _currentSession.CurrentPhase = e.Phase;
    }

    // ── Event accumulation ──

    private void HandleZoneChange(object? sender, ZoneChangeEventArgs e)
    {
        if (_currentSession == null) return;

        AddEvent("ZoneTransition", new Dictionary<string, object>
        {
            ["cardId"] = e.CardId,
            ["cardName"] = e.CardName,
            ["sourceZone"] = e.SourceZone,
            ["destinationZone"] = e.DestinationZone,
            ["ownerSeat"] = e.OwnerSeat,
        });
    }

    private void HandleGameAction(object? sender, GameActionEventArgs e)
    {
        if (_currentSession == null) return;

        var data = new Dictionary<string, object>
        {
            ["cardId"] = e.CardId,
            ["cardName"] = e.CardName,
            ["playerSeat"] = e.PlayerSeat,
        };
        if (e.AbilityText != null) data["abilityText"] = e.AbilityText;
        if (e.SourceZone != null) data["sourceZone"] = e.SourceZone;

        AddEvent(e.ActionType, data);
    }

    private void HandleLifeChange(object? sender, LifeChangeEventArgs e)
    {
        if (_currentSession == null) return;

        var data = new Dictionary<string, object>
        {
            ["playerSeat"] = e.PlayerSeat,
            ["oldLife"] = e.OldLife,
            ["newLife"] = e.NewLife,
        };
        if (e.Source != null) data["source"] = e.Source;

        AddEvent("LifeChange", data);
    }

    private void AddEvent(string type, Dictionary<string, object> data)
    {
        if (_currentSession == null) return;

        var gameEvent = new GameEvent
        {
            Type = type,
            Turn = _currentSession.CurrentTurn,
            Phase = _currentSession.CurrentPhase,
            ActivePlayer = _currentSession.CurrentActivePlayer,
            Data = data,
        };

        _currentSession.Timeline.Add(new TimelineEntry
        {
            Type = "event",
            Event = gameEvent,
        });
    }

    // ── Snapshot capture ──

    private void CaptureSnapshot(int turn, string phase, string activePlayer)
    {
        if (_currentSession == null) return;

        var state = _snapshotProvider?.Invoke(_currentSession.GameId, turn)
                    ?? new Dictionary<string, object>();

        var snapshot = new Snapshot
        {
            Turn = turn,
            Phase = phase,
            ActivePlayer = activePlayer,
            State = state,
        };

        _currentSession.Timeline.Add(new TimelineEntry
        {
            Type = "snapshot",
            Snapshot = snapshot,
        });
    }

    // ── Replay assembly ──

    private ReplayData AssembleReplay(GameSession session)
    {
        GameResult? result = null;
        if (session.WinnerName != null || session.EndReason != null)
        {
            result = new GameResult
            {
                Winner = session.WinnerName,
                Reason = session.EndReason,
            };
        }

        return new ReplayData
        {
            Header = new ReplayHeader
            {
                GameId = session.GameId,
                GameNumber = session.GameNumber,
                Players = session.Players,
                Format = session.Format,
                StartTime = session.StartTime,
                EndTime = session.EndTime,
                Result = result,
                Complete = session.Complete,
                DeckList = session.DeckList,
            },
            Timeline = session.Timeline.ToList(),
            CardCatalog = _client.GetCardCatalog(),
        };
    }

    // ── IDisposable ──

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;

        // If there's an active session, save it as incomplete.
        if (_currentSession != null)
        {
            EndCurrentGame(winnerName: null, reason: "shutdown", complete: false);
        }

        UnsubscribeFromClientEvents();
    }
}
