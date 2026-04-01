using MTGOSDK.API;
using MTGOSDK.API.Play;
using MTGOSDK.API.Play.Games;

namespace FlashbackRecorder;

/// <summary>
/// MTGOSDK integration layer. Attaches to the MTGO process and translates
/// SDK events into the DTOs defined in <see cref="IMtgoClient"/>.
/// </summary>
public sealed class MtgoClient : IMtgoClient
{
    private static readonly TimeSpan ProcessPollInterval = TimeSpan.FromSeconds(3);

    private Client? _sdkClient;
    private EventManager? _eventManager;
    private bool _disposed;

    public ConnectionState State { get; private set; } = ConnectionState.Disconnected;

    // ── Events ──

    public event EventHandler<ZoneChangeEventArgs>? OnZoneChange;
    public event EventHandler<GameActionEventArgs>? OnGameAction;
    public event EventHandler<LifeChangeEventArgs>? OnLifeChange;
    public event EventHandler<GamePhaseChangeEventArgs>? OnGamePhaseChange;
    public event EventHandler<TurnChangeEventArgs>? OnTurnChange;
    public event EventHandler<GameStatusChangeEventArgs>? OnGameStatusChange;

    // ── Connection lifecycle ──

    /// <summary>
    /// Attaches to a running MTGO process. If no process is found, polls at
    /// a fixed interval until one appears or cancellation is requested.
    /// </summary>
    public async Task ConnectAsync(CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);

        if (State == ConnectionState.Attached)
            return;

        State = ConnectionState.WaitingForProcess;

        while (!cancellationToken.IsCancellationRequested)
        {
            try
            {
                _sdkClient = new Client();
                _eventManager = new EventManager();
                SubscribeToEvents();

                State = ConnectionState.Attached;
                return;
            }
            catch (Exception) when (!cancellationToken.IsCancellationRequested)
            {
                // MTGO not running yet — wait and retry.
                _sdkClient?.Dispose();
                _sdkClient = null;
                _eventManager = null;

                await Task.Delay(ProcessPollInterval, cancellationToken)
                    .ConfigureAwait(false);
            }
        }

        cancellationToken.ThrowIfCancellationRequested();
    }

    public void Disconnect()
    {
        UnsubscribeFromEvents();
        _sdkClient?.Dispose();
        _sdkClient = null;
        _eventManager = null;
        State = ConnectionState.Disconnected;
    }

    // ── SDK event wiring ──

    private void SubscribeToEvents()
    {
        if (_eventManager is null) return;

        _eventManager.GameJoined += OnSdkGameJoined;
    }

    private void UnsubscribeFromEvents()
    {
        if (_eventManager is null) return;

        _eventManager.GameJoined -= OnSdkGameJoined;
    }

    /// <summary>
    /// Called when the local player joins a game. Sets up per-game event
    /// subscriptions on the <see cref="GameState"/> instance.
    /// </summary>
    private void OnSdkGameJoined(object? sender, GameJoinedEventArgs e)
    {
        var game = e.Game;

        // Notify that a new game has started.
        OnGameStatusChange?.Invoke(this, new GameStatusChangeEventArgs
        {
            Status = GameStatus.Started,
            GameId = game.GameId,
        });

        // Subscribe to per-game events.
        SubscribeToGameEvents(game);
    }

    private void SubscribeToGameEvents(GameState game)
    {
        game.OnZoneChange += (_, zoneArgs) =>
        {
            OnZoneChange?.Invoke(this, new ZoneChangeEventArgs
            {
                CardId = zoneArgs.Card.Id,
                CardName = zoneArgs.Card.Name,
                SourceZone = zoneArgs.SourceZone.ToString(),
                DestinationZone = zoneArgs.DestinationZone.ToString(),
                OwnerSeat = zoneArgs.Card.Owner.Seat,
            });
        };

        game.OnGameAction += (_, actionArgs) =>
        {
            OnGameAction?.Invoke(this, new GameActionEventArgs
            {
                ActionType = actionArgs.Action.Type.ToString(),
                CardId = actionArgs.Action.Card.Id,
                CardName = actionArgs.Action.Card.Name,
                PlayerSeat = actionArgs.Action.Player.Seat,
                AbilityText = actionArgs.Action.AbilityText,
                SourceZone = actionArgs.Action.SourceZone?.ToString(),
            });
        };

        game.OnLifeChange += (_, lifeArgs) =>
        {
            OnLifeChange?.Invoke(this, new LifeChangeEventArgs
            {
                PlayerSeat = lifeArgs.Player.Seat,
                OldLife = lifeArgs.OldValue,
                NewLife = lifeArgs.NewValue,
                Source = lifeArgs.Source?.Name,
            });
        };

        game.OnGamePhaseChange += (_, phaseArgs) =>
        {
            OnGamePhaseChange?.Invoke(this, new GamePhaseChangeEventArgs
            {
                Phase = phaseArgs.Phase.ToString(),
                ActivePlayerSeat = phaseArgs.ActivePlayer.Seat,
            });
        };

        game.CurrentTurnChanged += (_, turnArgs) =>
        {
            OnTurnChange?.Invoke(this, new TurnChangeEventArgs
            {
                TurnNumber = turnArgs.TurnNumber,
                ActivePlayerSeat = turnArgs.ActivePlayer.Seat,
                ActivePlayerName = turnArgs.ActivePlayer.Name,
            });
        };

        game.GameStatusChanged += (_, statusArgs) =>
        {
            // Only fire for game end here; game start is handled by GameJoined.
            if (statusArgs.IsComplete)
            {
                OnGameStatusChange?.Invoke(this, new GameStatusChangeEventArgs
                {
                    Status = GameStatus.Ended,
                    GameId = game.GameId,
                    WinnerName = statusArgs.Winner?.Name,
                    Reason = statusArgs.Reason?.ToString(),
                });
            }
        };
    }

    // ── IDisposable ──

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;
        Disconnect();
    }
}
