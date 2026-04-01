namespace FlashbackRecorder;

/// <summary>
/// Abstraction over the MTGOSDK process attachment and event system.
/// Enables testability by allowing mock implementations.
/// </summary>
public interface IMtgoClient : IDisposable
{
    /// <summary>Current connection state to the MTGO process.</summary>
    ConnectionState State { get; }

    /// <summary>Attach to a running MTGO process, or poll until one appears.</summary>
    /// <param name="cancellationToken">Cancellation token to stop waiting.</param>
    Task ConnectAsync(CancellationToken cancellationToken = default);

    /// <summary>Disconnect from the MTGO process and unsubscribe all events.</summary>
    void Disconnect();

    // ── Event callbacks matching spec: Recorder Architecture > 1. MTGOSDK Integration Layer ──

    /// <summary>Fired when a card moves between zones.</summary>
    event EventHandler<ZoneChangeEventArgs> OnZoneChange;

    /// <summary>Fired when a spell is cast or ability activated.</summary>
    event EventHandler<GameActionEventArgs> OnGameAction;

    /// <summary>Fired when a player's life total changes.</summary>
    event EventHandler<LifeChangeEventArgs> OnLifeChange;

    /// <summary>Fired on phase transitions (e.g. main → combat).</summary>
    event EventHandler<GamePhaseChangeEventArgs> OnGamePhaseChange;

    /// <summary>Fired when the active turn changes.</summary>
    event EventHandler<TurnChangeEventArgs> OnTurnChange;

    /// <summary>Fired when game status changes (start/end).</summary>
    event EventHandler<GameStatusChangeEventArgs> OnGameStatusChange;
}

// ── Connection state ──

public enum ConnectionState
{
    Disconnected,
    WaitingForProcess,
    Attached,
    Error
}

// ── Event argument DTOs ──

public class ZoneChangeEventArgs : EventArgs
{
    public required int CardId { get; init; }
    public required string CardName { get; init; }
    public required string SourceZone { get; init; }
    public required string DestinationZone { get; init; }
    public required int OwnerSeat { get; init; }
}

public class GameActionEventArgs : EventArgs
{
    public required string ActionType { get; init; }  // "CastSpell", "ActivateAbility"
    public required int CardId { get; init; }
    public required string CardName { get; init; }
    public required int PlayerSeat { get; init; }
    public string? AbilityText { get; init; }
    public string? SourceZone { get; init; }
}

public class LifeChangeEventArgs : EventArgs
{
    public required int PlayerSeat { get; init; }
    public required int OldLife { get; init; }
    public required int NewLife { get; init; }
    public string? Source { get; init; }
}

public class GamePhaseChangeEventArgs : EventArgs
{
    public required string Phase { get; init; }
    public required int ActivePlayerSeat { get; init; }
}

public class TurnChangeEventArgs : EventArgs
{
    public required int TurnNumber { get; init; }
    public required int ActivePlayerSeat { get; init; }
    public required string ActivePlayerName { get; init; }
}

public class GameStatusChangeEventArgs : EventArgs
{
    public required GameStatus Status { get; init; }
    public required int GameId { get; init; }
    public string? WinnerName { get; init; }
    public string? Reason { get; init; }
}

public enum GameStatus
{
    Started,
    Ended
}
