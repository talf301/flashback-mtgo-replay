namespace FlashbackRecorder.Models;

/// <summary>
/// A full board-state snapshot captured at the start of each turn.
/// </summary>
public class Snapshot
{
    public required int Turn { get; init; }
    public required string Phase { get; init; }
    public required string ActivePlayer { get; init; }

    /// <summary>
    /// Full game state keyed by zone/player. Structure is opaque to the session
    /// manager — populated by the snapshot capture logic and serialized as-is.
    /// </summary>
    public Dictionary<string, object> State { get; init; } = new();

    public DateTimeOffset Timestamp { get; init; } = DateTimeOffset.UtcNow;
}
