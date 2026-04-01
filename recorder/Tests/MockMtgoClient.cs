using FlashbackRecorder;

namespace FlashbackRecorder.Tests;

/// <summary>
/// Mock implementation of <see cref="IMtgoClient"/> for unit testing.
/// Exposes methods to simulate MTGO events.
/// </summary>
public class MockMtgoClient : IMtgoClient
{
    public ConnectionState State { get; set; } = ConnectionState.Attached;

    public event EventHandler<ZoneChangeEventArgs>? OnZoneChange;
    public event EventHandler<GameActionEventArgs>? OnGameAction;
    public event EventHandler<LifeChangeEventArgs>? OnLifeChange;
    public event EventHandler<GamePhaseChangeEventArgs>? OnGamePhaseChange;
    public event EventHandler<TurnChangeEventArgs>? OnTurnChange;
    public event EventHandler<GameStatusChangeEventArgs>? OnGameStatusChange;

    public Task ConnectAsync(CancellationToken cancellationToken = default) => Task.CompletedTask;
    public void Disconnect() { State = ConnectionState.Disconnected; }
    public void Dispose() { }

    // ── Simulation helpers ──

    public void SimulateGameStart(int gameId) =>
        OnGameStatusChange?.Invoke(this, new GameStatusChangeEventArgs
        {
            Status = GameStatus.Started,
            GameId = gameId,
        });

    public void SimulateGameEnd(int gameId, string? winner = null, string? reason = null) =>
        OnGameStatusChange?.Invoke(this, new GameStatusChangeEventArgs
        {
            Status = GameStatus.Ended,
            GameId = gameId,
            WinnerName = winner,
            Reason = reason,
        });

    public void SimulateTurnChange(int turnNumber, int activePlayerSeat, string activePlayerName) =>
        OnTurnChange?.Invoke(this, new TurnChangeEventArgs
        {
            TurnNumber = turnNumber,
            ActivePlayerSeat = activePlayerSeat,
            ActivePlayerName = activePlayerName,
        });

    public void SimulatePhaseChange(string phase, int activePlayerSeat) =>
        OnGamePhaseChange?.Invoke(this, new GamePhaseChangeEventArgs
        {
            Phase = phase,
            ActivePlayerSeat = activePlayerSeat,
        });

    public void SimulateZoneChange(int cardId, string cardName, string source, string dest, int ownerSeat) =>
        OnZoneChange?.Invoke(this, new ZoneChangeEventArgs
        {
            CardId = cardId,
            CardName = cardName,
            SourceZone = source,
            DestinationZone = dest,
            OwnerSeat = ownerSeat,
        });

    public void SimulateGameAction(string actionType, int cardId, string cardName, int playerSeat) =>
        OnGameAction?.Invoke(this, new GameActionEventArgs
        {
            ActionType = actionType,
            CardId = cardId,
            CardName = cardName,
            PlayerSeat = playerSeat,
        });

    public void SimulateLifeChange(int playerSeat, int oldLife, int newLife, string? source = null) =>
        OnLifeChange?.Invoke(this, new LifeChangeEventArgs
        {
            PlayerSeat = playerSeat,
            OldLife = oldLife,
            NewLife = newLife,
            Source = source,
        });
}
