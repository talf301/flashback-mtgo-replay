using System.Text.Json.Serialization;

namespace FlashbackRecorder.Models;

/// <summary>
/// v3 JSON-serializable classes for the .flashback replay format.
/// These mirror the on-disk JSON structure and use snake_case naming via JsonPropertyName.
/// </summary>

/// <summary>
/// Top-level .flashback file structure.
/// </summary>
public class ReplayFileFormat
{
    [JsonPropertyName("version")]
    public string Version { get; init; } = "3.0";

    [JsonPropertyName("header")]
    public required ReplayFileHeader Header { get; init; }

    [JsonPropertyName("timeline")]
    public required List<TimelineEntryFormat> Timeline { get; init; }

    [JsonPropertyName("card_catalog")]
    public Dictionary<string, CardCatalogEntryFormat> CardCatalog { get; init; } = new();
}

public class ReplayFileHeader
{
    [JsonPropertyName("game_id")]
    public int GameId { get; init; }

    [JsonPropertyName("players")]
    public required List<PlayerInfoFormat> Players { get; init; }

    [JsonPropertyName("format")]
    public string? Format { get; init; }

    [JsonPropertyName("start_time")]
    public string StartTime { get; init; } = "";

    [JsonPropertyName("end_time")]
    public string? EndTime { get; init; }

    [JsonPropertyName("result")]
    public GameResultFormat? Result { get; init; }

    [JsonPropertyName("complete")]
    public bool Complete { get; init; }

    [JsonPropertyName("decklist")]
    public DeckListFormat? DeckList { get; init; }

    [JsonPropertyName("sideboard_changes")]
    public SideboardChangesFormat? SideboardChanges { get; init; }
}

public class PlayerInfoFormat
{
    [JsonPropertyName("name")]
    public required string Name { get; init; }

    [JsonPropertyName("seat")]
    public int Seat { get; init; }
}

public class GameResultFormat
{
    [JsonPropertyName("winner")]
    public string? Winner { get; init; }

    [JsonPropertyName("reason")]
    public string? Reason { get; init; }
}

public class DeckListFormat
{
    [JsonPropertyName("mainboard")]
    public List<string> Mainboard { get; init; } = new();

    [JsonPropertyName("sideboard")]
    public List<string> Sideboard { get; init; } = new();
}

public class SideboardChangesFormat
{
    [JsonPropertyName("in")]
    public List<string> In { get; init; } = new();

    [JsonPropertyName("out")]
    public List<string> Out { get; init; } = new();
}

public class CardCatalogEntryFormat
{
    [JsonPropertyName("name")]
    public required string Name { get; init; }

    [JsonPropertyName("mana_cost")]
    public string? ManaCost { get; init; }

    [JsonPropertyName("type_line")]
    public string? TypeLine { get; init; }
}

/// <summary>
/// A single timeline entry — either a snapshot or an event.
/// Uses a discriminator "type" field.
/// </summary>
public class TimelineEntryFormat
{
    [JsonPropertyName("type")]
    public required string Type { get; init; }

    [JsonPropertyName("turn")]
    public int Turn { get; init; }

    [JsonPropertyName("phase")]
    public string Phase { get; init; } = "";

    [JsonPropertyName("active_player")]
    public string ActivePlayer { get; init; } = "";

    /// <summary>Present when type == "snapshot".</summary>
    [JsonPropertyName("state")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public Dictionary<string, object>? State { get; init; }

    /// <summary>Present when type == "event".</summary>
    [JsonPropertyName("event")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public EventPayloadFormat? Event { get; init; }
}

public class EventPayloadFormat
{
    [JsonPropertyName("type")]
    public required string Type { get; init; }

    [JsonPropertyName("data")]
    public Dictionary<string, object> Data { get; init; } = new();

    [JsonPropertyName("timestamp")]
    public string Timestamp { get; init; } = "";
}
