using System.Text.Json;
using FlashbackRecorder.Models;

namespace FlashbackRecorder;

/// <summary>
/// Serializes assembled replay data to v3 JSON .flashback format and writes to disk.
/// Handles file naming (YYYY-MM-DD_Player1-vs-Player2_GameID_gN.flashback),
/// output directory configuration, and the complete v3 JSON structure.
/// </summary>
public class FileWriter
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        PropertyNamingPolicy = null, // We use explicit JsonPropertyName attributes
    };

    private readonly string _outputDirectory;

    /// <summary>
    /// Creates a new FileWriter.
    /// </summary>
    /// <param name="outputDirectory">
    /// Directory to write .flashback files to. If null, uses the default
    /// %APPDATA%/Flashback/replays/ directory.
    /// </param>
    public FileWriter(string? outputDirectory = null)
    {
        _outputDirectory = outputDirectory ?? GetDefaultOutputDirectory();
    }

    /// <summary>The configured output directory.</summary>
    public string OutputDirectory => _outputDirectory;

    /// <summary>
    /// Writes a replay to disk as a .flashback JSON file.
    /// Returns the full path to the written file.
    /// </summary>
    public string WriteReplay(ReplayData replay)
    {
        if (replay == null) throw new ArgumentNullException(nameof(replay));

        Directory.CreateDirectory(_outputDirectory);

        var fileName = GenerateFileName(replay);
        var filePath = Path.Combine(_outputDirectory, fileName);
        var format = ConvertToFileFormat(replay);
        var json = JsonSerializer.Serialize(format, JsonOptions);
        File.WriteAllText(filePath, json);

        return filePath;
    }

    /// <summary>
    /// Serializes a replay to a JSON string without writing to disk.
    /// Useful for testing and inspection.
    /// </summary>
    public static string SerializeToJson(ReplayData replay)
    {
        if (replay == null) throw new ArgumentNullException(nameof(replay));

        var format = ConvertToFileFormat(replay);
        return JsonSerializer.Serialize(format, JsonOptions);
    }

    /// <summary>
    /// Generates the file name: YYYY-MM-DD_Player1-vs-Player2_GameID_gN.flashback
    /// </summary>
    public static string GenerateFileName(ReplayData replay)
    {
        var date = replay.Header.StartTime.ToString("yyyy-MM-dd");

        var player1 = replay.Header.Players.Count > 0
            ? SanitizeFileName(replay.Header.Players[0].Name)
            : "Unknown";
        var player2 = replay.Header.Players.Count > 1
            ? SanitizeFileName(replay.Header.Players[1].Name)
            : "Unknown";

        var gameId = replay.Header.GameId;
        var gameNum = replay.Header.GameNumber;

        return $"{date}_{player1}-vs-{player2}_{gameId}_g{gameNum}.flashback";
    }

    /// <summary>
    /// Converts internal ReplayData to the v3 JSON-serializable format.
    /// </summary>
    internal static ReplayFileFormat ConvertToFileFormat(ReplayData replay)
    {
        var header = ConvertHeader(replay.Header);
        var timeline = ConvertTimeline(replay.Timeline);
        var catalog = ConvertCardCatalog(replay.CardCatalog);

        return new ReplayFileFormat
        {
            Version = replay.Version,
            Header = header,
            Timeline = timeline,
            CardCatalog = catalog,
        };
    }

    private static ReplayFileHeader ConvertHeader(ReplayHeader h)
    {
        return new ReplayFileHeader
        {
            GameId = h.GameId,
            Players = h.Players.Select(p => new PlayerInfoFormat
            {
                Name = p.Name,
                Seat = p.Seat,
            }).ToList(),
            Format = h.Format,
            StartTime = h.StartTime.ToString("o"),
            EndTime = h.EndTime?.ToString("o"),
            Result = h.Result != null ? new GameResultFormat
            {
                Winner = h.Result.Winner,
                Reason = h.Result.Reason,
            } : null,
            Complete = h.Complete,
            DeckList = h.DeckList != null ? new DeckListFormat
            {
                Mainboard = h.DeckList.Mainboard,
                Sideboard = h.DeckList.Sideboard,
            } : null,
            SideboardChanges = h.SideboardChanges != null ? new SideboardChangesFormat
            {
                In = h.SideboardChanges.In,
                Out = h.SideboardChanges.Out,
            } : null,
        };
    }

    private static List<TimelineEntryFormat> ConvertTimeline(List<TimelineEntry> timeline)
    {
        return timeline.Select(entry =>
        {
            if (entry.Type == "snapshot" && entry.Snapshot != null)
            {
                return new TimelineEntryFormat
                {
                    Type = "snapshot",
                    Turn = entry.Snapshot.Turn,
                    Phase = entry.Snapshot.Phase,
                    ActivePlayer = entry.Snapshot.ActivePlayer,
                    State = entry.Snapshot.State,
                };
            }
            else if (entry.Type == "event" && entry.Event != null)
            {
                return new TimelineEntryFormat
                {
                    Type = "event",
                    Turn = entry.Event.Turn,
                    Phase = entry.Event.Phase,
                    ActivePlayer = entry.Event.ActivePlayer,
                    Event = new EventPayloadFormat
                    {
                        Type = entry.Event.Type,
                        Data = entry.Event.Data,
                        Timestamp = entry.Event.Timestamp.ToString("o"),
                    },
                };
            }
            else
            {
                // Fallback — shouldn't happen in practice.
                return new TimelineEntryFormat
                {
                    Type = entry.Type,
                };
            }
        }).ToList();
    }

    private static Dictionary<string, CardCatalogEntryFormat> ConvertCardCatalog(
        Dictionary<string, CardCatalogEntry> catalog)
    {
        return catalog.ToDictionary(
            kvp => kvp.Key,
            kvp => new CardCatalogEntryFormat
            {
                Name = kvp.Value.Name,
                ManaCost = kvp.Value.ManaCost,
                TypeLine = kvp.Value.TypeLine,
            });
    }

    /// <summary>
    /// Removes characters that are invalid in file names.
    /// </summary>
    internal static string SanitizeFileName(string name)
    {
        // Hardcoded Windows-invalid filename chars so behavior is consistent cross-platform
        var invalidChars = new HashSet<char>(new[] {
            '"', '<', '>', '|', '\0',
            (char)1, (char)2, (char)3, (char)4, (char)5, (char)6, (char)7, (char)8, (char)9, (char)10,
            (char)11, (char)12, (char)13, (char)14, (char)15, (char)16, (char)17, (char)18, (char)19, (char)20,
            (char)21, (char)22, (char)23, (char)24, (char)25, (char)26, (char)27, (char)28, (char)29, (char)30,
            (char)31, ':', '*', '?', '\\', '/'
        });
        var sanitized = new string(name.Where(c => !invalidChars.Contains(c)).ToArray());
        // Replace spaces with underscores for cleaner file names
        sanitized = sanitized.Replace(' ', '_');
        return string.IsNullOrWhiteSpace(sanitized) ? "Unknown" : sanitized;
    }

    private static string GetDefaultOutputDirectory()
    {
        var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        return Path.Combine(appData, "Flashback", "replays");
    }
}
