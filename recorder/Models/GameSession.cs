namespace FlashbackRecorder.Models;

/// <summary>
/// In-progress recording session for a single game within a match.
/// </summary>
public class GameSession
{
    public required int GameId { get; init; }
    public required int GameNumber { get; init; }

    public DateTimeOffset StartTime { get; init; } = DateTimeOffset.UtcNow;
    public DateTimeOffset? EndTime { get; set; }

    /// <summary>Player info: name → seat mapping.</summary>
    public List<PlayerInfo> Players { get; init; } = new();

    /// <summary>Deck list captured at game start.</summary>
    public DeckList? DeckList { get; set; }

    /// <summary>
    /// Interleaved timeline of snapshots and events in chronological order.
    /// </summary>
    public List<TimelineEntry> Timeline { get; } = new();

    /// <summary>Current turn number, updated on TurnChange events.</summary>
    public int CurrentTurn { get; set; } = 0;

    /// <summary>Current phase, updated on PhaseChange events.</summary>
    public string CurrentPhase { get; set; } = "pregame";

    /// <summary>Current active player name.</summary>
    public string CurrentActivePlayer { get; set; } = "";

    /// <summary>Whether the game ended normally.</summary>
    public bool Complete { get; set; } = false;

    /// <summary>Winner name, if game ended normally.</summary>
    public string? WinnerName { get; set; }

    /// <summary>End reason (concession, life, decking, etc.).</summary>
    public string? EndReason { get; set; }
}

public class PlayerInfo
{
    public required string Name { get; init; }
    public required int Seat { get; init; }
}

public class DeckList
{
    public List<string> Mainboard { get; init; } = new();
    public List<string> Sideboard { get; init; } = new();
}

/// <summary>
/// A tagged union entry in the timeline — either a snapshot or an event.
/// </summary>
public class TimelineEntry
{
    public required string Type { get; init; } // "snapshot" or "event"
    public Snapshot? Snapshot { get; init; }
    public GameEvent? Event { get; init; }
}
