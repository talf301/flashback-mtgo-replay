namespace FlashbackRecorder.Models;

/// <summary>
/// A single game event in the timeline (zone change, life change, action, phase change).
/// </summary>
public class GameEvent
{
    public required string Type { get; init; }
    public required int Turn { get; init; }
    public required string Phase { get; init; }
    public required string ActivePlayer { get; init; }

    /// <summary>Event-specific payload.</summary>
    public Dictionary<string, object> Data { get; init; } = new();

    public DateTimeOffset Timestamp { get; init; } = DateTimeOffset.UtcNow;
}
