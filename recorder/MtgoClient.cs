using MTGOSDK.API;
using MTGOSDK.API.Play;
using MTGOSDK.API.Play.Games;
using static MTGOSDK.API.Events;

namespace FlashbackRecorder;

/// <summary>
/// MTGOSDK integration layer. Attaches to the MTGO process and translates
/// SDK events into the DTOs defined in <see cref="IMtgoClient"/>.
/// </summary>
public sealed class MtgoClient : IMtgoClient
{
    private static readonly TimeSpan ProcessPollInterval = TimeSpan.FromSeconds(3);

    private Client? _sdkClient;
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
                SubscribeToEvents();

                State = ConnectionState.Attached;
                return;
            }
            catch (Exception) when (!cancellationToken.IsCancellationRequested)
            {
                // MTGO not running yet — wait and retry.
                _sdkClient?.Dispose();
                _sdkClient = null;

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
        State = ConnectionState.Disconnected;
    }

    // ── SDK event wiring ──

    private void SubscribeToEvents()
    {
        EventManager.GameJoined += OnSdkGameJoined;
    }

    private void UnsubscribeFromEvents()
    {
        EventManager.GameJoined -= OnSdkGameJoined;
    }

    /// <summary>
    /// Called when the local player joins a game. Sets up per-game event
    /// subscriptions on the <see cref="Game"/> instance.
    /// </summary>
    private void OnSdkGameJoined(Event playerEvent, Game game)
    {
        // Notify that a new game has started.
        OnGameStatusChange?.Invoke(this, new GameStatusChangeEventArgs
        {
            Status = GameStatus.Started,
            GameId = game.Id,
        });

        // Subscribe to per-game events.
        SubscribeToGameEvents(game);
    }

    private void SubscribeToGameEvents(Game game)
    {
        game.OnZoneChange += (GameCard card) =>
        {
            OnZoneChange?.Invoke(this, new ZoneChangeEventArgs
            {
                CardId = card.Id,
                CardName = card.Name,
                SourceZone = card.Zone.ToString(),
                DestinationZone = card.Zone.ToString(),
                OwnerSeat = card.Owner.Seat,
            });
        };

        game.OnGameAction += (GameAction action) =>
        {
            OnGameAction?.Invoke(this, new GameActionEventArgs
            {
                ActionType = action.Type.ToString(),
                CardId = action.Card.Id,
                CardName = action.Card.Name,
                PlayerSeat = action.Player.Seat,
                AbilityText = action.AbilityText,
                SourceZone = action.SourceZone?.ToString(),
            });
        };

        game.OnLifeChange += (GamePlayer player) =>
        {
            OnLifeChange?.Invoke(this, new LifeChangeEventArgs
            {
                PlayerSeat = player.Seat,
                OldLife = player.Life,
                NewLife = player.Life,
                Source = null,
            });
        };

        game.OnGamePhaseChange += (CurrentPlayerPhase phase) =>
        {
            OnGamePhaseChange?.Invoke(this, new GamePhaseChangeEventArgs
            {
                Phase = phase.ToString(),
                ActivePlayerSeat = 0,
            });
        };

        game.CurrentTurnChanged += (GameEventArgs args) =>
        {
            OnTurnChange?.Invoke(this, new TurnChangeEventArgs
            {
                TurnNumber = 0,
                ActivePlayerSeat = 0,
                ActivePlayerName = "",
            });
        };

        game.GameStatusChanged += (GameStatusEventArgs args) =>
        {
            if (args.IsComplete)
            {
                OnGameStatusChange?.Invoke(this, new GameStatusChangeEventArgs
                {
                    Status = GameStatus.Ended,
                    GameId = game.Id,
                    WinnerName = args.Winner?.Name,
                    Reason = args.Reason?.ToString(),
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
