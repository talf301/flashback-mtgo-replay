namespace FlashbackRecorder.Models;

/// <summary>
/// Assembled replay data ready for serialization by the file writer.
/// This is the output of the Game Session Manager when a game ends.
/// </summary>
public class ReplayData
{
    public string Version { get; init; } = "3";

    public required ReplayHeader Header { get; init; }

    /// <summary>Interleaved snapshots and events in chronological order.</summary>
    public required List<TimelineEntry> Timeline { get; init; }

    /// <summary>Card metadata keyed by card ID.</summary>
    public Dictionary<string, CardCatalogEntry> CardCatalog { get; init; } = new();
}

public class ReplayHeader
{
    public required int GameId { get; init; }
    public required int GameNumber { get; init; }
    public required List<PlayerInfo> Players { get; init; }
    public string? Format { get; init; }
    public required DateTimeOffset StartTime { get; init; }
    public DateTimeOffset? EndTime { get; init; }
    public GameResult? Result { get; init; }
    public required bool Complete { get; init; }
    public DeckList? DeckList { get; init; }
    public SideboardChanges? SideboardChanges { get; init; }
}

public class GameResult
{
    public string? Winner { get; init; }
    public string? Reason { get; init; }
}

public class SideboardChanges
{
    public List<string> In { get; init; } = new();
    public List<string> Out { get; init; } = new();
}

public class CardCatalogEntry
{
    public required string Name { get; init; }
    public string? ManaCost { get; init; }
    public string? TypeLine { get; init; }
}
