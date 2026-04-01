using System.Text.Json;
using FlashbackRecorder;
using FlashbackRecorder.Models;
using Xunit;

namespace FlashbackRecorder.Tests;

public class FileWriterTests : IDisposable
{
    private readonly string _tempDir;
    private readonly FileWriter _writer;

    public FileWriterTests()
    {
        _tempDir = Path.Combine(Path.GetTempPath(), "flashback_test_" + Guid.NewGuid().ToString("N")[..8]);
        _writer = new FileWriter(_tempDir);
    }

    public void Dispose()
    {
        if (Directory.Exists(_tempDir))
        {
            Directory.Delete(_tempDir, recursive: true);
        }
    }

    // ── Helpers ──

    private static ReplayData CreateMinimalReplay(
        int gameId = 12345,
        int gameNumber = 1,
        string player1 = "Alice",
        string player2 = "Bob",
        bool complete = true)
    {
        return new ReplayData
        {
            Header = new ReplayHeader
            {
                GameId = gameId,
                GameNumber = gameNumber,
                Players = new List<PlayerInfo>
                {
                    new() { Name = player1, Seat = 0 },
                    new() { Name = player2, Seat = 1 },
                },
                StartTime = new DateTimeOffset(2026, 3, 31, 10, 0, 0, TimeSpan.Zero),
                EndTime = new DateTimeOffset(2026, 3, 31, 10, 15, 0, TimeSpan.Zero),
                Result = new GameResult { Winner = player1, Reason = "concession" },
                Complete = complete,
            },
            Timeline = new List<TimelineEntry>(),
        };
    }

    private static ReplayData CreateFullReplay()
    {
        var startTime = new DateTimeOffset(2026, 3, 31, 10, 0, 0, TimeSpan.Zero);

        return new ReplayData
        {
            Header = new ReplayHeader
            {
                GameId = 99999,
                GameNumber = 1,
                Players = new List<PlayerInfo>
                {
                    new() { Name = "Alice", Seat = 0 },
                    new() { Name = "Bob", Seat = 1 },
                },
                Format = "Modern",
                StartTime = startTime,
                EndTime = startTime.AddMinutes(15),
                Result = new GameResult { Winner = "Alice", Reason = "concession" },
                Complete = true,
                DeckList = new DeckList
                {
                    Mainboard = new List<string> { "Lightning Bolt", "Lightning Bolt", "Snapcaster Mage" },
                    Sideboard = new List<string> { "Rest in Peace", "Wear // Tear" },
                },
                SideboardChanges = null,
            },
            Timeline = new List<TimelineEntry>
            {
                new()
                {
                    Type = "snapshot",
                    Snapshot = new Snapshot
                    {
                        Turn = 1,
                        Phase = "precombat_main",
                        ActivePlayer = "Alice",
                        State = new Dictionary<string, object>
                        {
                            ["life_totals"] = new Dictionary<string, object> { ["Alice"] = 20, ["Bob"] = 20 },
                        },
                        Timestamp = startTime,
                    },
                },
                new()
                {
                    Type = "event",
                    Event = new GameEvent
                    {
                        Type = "CastSpell",
                        Turn = 1,
                        Phase = "precombat_main",
                        ActivePlayer = "Alice",
                        Data = new Dictionary<string, object>
                        {
                            ["cardId"] = 101,
                            ["cardName"] = "Lightning Bolt",
                            ["playerSeat"] = 0,
                        },
                        Timestamp = startTime.AddSeconds(30),
                    },
                },
                new()
                {
                    Type = "event",
                    Event = new GameEvent
                    {
                        Type = "LifeChange",
                        Turn = 1,
                        Phase = "precombat_main",
                        ActivePlayer = "Alice",
                        Data = new Dictionary<string, object>
                        {
                            ["playerSeat"] = 1,
                            ["oldLife"] = 20,
                            ["newLife"] = 17,
                            ["source"] = "Lightning Bolt",
                        },
                        Timestamp = startTime.AddSeconds(31),
                    },
                },
                new()
                {
                    Type = "snapshot",
                    Snapshot = new Snapshot
                    {
                        Turn = 2,
                        Phase = "untap",
                        ActivePlayer = "Bob",
                        State = new Dictionary<string, object>
                        {
                            ["life_totals"] = new Dictionary<string, object> { ["Alice"] = 20, ["Bob"] = 17 },
                        },
                        Timestamp = startTime.AddMinutes(1),
                    },
                },
            },
            CardCatalog = new Dictionary<string, CardCatalogEntry>
            {
                ["101"] = new() { Name = "Lightning Bolt", ManaCost = "{R}", TypeLine = "Instant" },
                ["102"] = new() { Name = "Snapcaster Mage", ManaCost = "{1}{U}", TypeLine = "Creature — Human Wizard" },
            },
        };
    }

    // ── File naming tests ──

    [Fact]
    public void GenerateFileName_BasicFormat()
    {
        var replay = CreateMinimalReplay();
        var fileName = FileWriter.GenerateFileName(replay);
        Assert.Equal("2026-03-31_Alice-vs-Bob_12345_g1.flashback", fileName);
    }

    [Fact]
    public void GenerateFileName_Game2Suffix()
    {
        var replay = CreateMinimalReplay(gameNumber: 2);
        var fileName = FileWriter.GenerateFileName(replay);
        Assert.Contains("_g2.flashback", fileName);
    }

    [Fact]
    public void GenerateFileName_Game3Suffix()
    {
        var replay = CreateMinimalReplay(gameNumber: 3);
        var fileName = FileWriter.GenerateFileName(replay);
        Assert.Contains("_g3.flashback", fileName);
    }

    [Fact]
    public void GenerateFileName_SpecialCharactersInPlayerName()
    {
        var replay = CreateMinimalReplay(player1: "Player<1>", player2: "Bob/Smith");
        var fileName = FileWriter.GenerateFileName(replay);
        // Should not contain invalid file name characters
        var invalidChars = Path.GetInvalidFileNameChars();
        Assert.DoesNotContain(fileName, c => invalidChars.Contains(c));
        Assert.EndsWith(".flashback", fileName);
    }

    [Fact]
    public void GenerateFileName_SpacesReplacedWithUnderscores()
    {
        var replay = CreateMinimalReplay(player1: "John Doe", player2: "Jane Doe");
        var fileName = FileWriter.GenerateFileName(replay);
        Assert.Contains("John_Doe-vs-Jane_Doe", fileName);
    }

    // ── JSON structure tests ──

    [Fact]
    public void SerializeToJson_TopLevelStructure()
    {
        var replay = CreateFullReplay();
        var json = FileWriter.SerializeToJson(replay);
        var doc = JsonDocument.Parse(json);
        var root = doc.RootElement;

        Assert.Equal("3.0", root.GetProperty("version").GetString());
        Assert.True(root.TryGetProperty("header", out _));
        Assert.True(root.TryGetProperty("timeline", out _));
        Assert.True(root.TryGetProperty("card_catalog", out _));
    }

    [Fact]
    public void SerializeToJson_HeaderFields()
    {
        var replay = CreateFullReplay();
        var json = FileWriter.SerializeToJson(replay);
        var doc = JsonDocument.Parse(json);
        var header = doc.RootElement.GetProperty("header");

        Assert.Equal(99999, header.GetProperty("game_id").GetInt32());
        Assert.Equal("Modern", header.GetProperty("format").GetString());
        Assert.True(header.GetProperty("complete").GetBoolean());
        Assert.Equal("Alice", header.GetProperty("result").GetProperty("winner").GetString());
        Assert.Equal("concession", header.GetProperty("result").GetProperty("reason").GetString());

        // start_time and end_time are ISO 8601 strings
        var startTime = header.GetProperty("start_time").GetString()!;
        Assert.Contains("2026-03-31", startTime);

        var endTime = header.GetProperty("end_time").GetString()!;
        Assert.Contains("2026-03-31", endTime);
    }

    [Fact]
    public void SerializeToJson_HeaderPlayers()
    {
        var replay = CreateFullReplay();
        var json = FileWriter.SerializeToJson(replay);
        var doc = JsonDocument.Parse(json);
        var players = doc.RootElement.GetProperty("header").GetProperty("players");

        Assert.Equal(2, players.GetArrayLength());
        Assert.Equal("Alice", players[0].GetProperty("name").GetString());
        Assert.Equal(0, players[0].GetProperty("seat").GetInt32());
        Assert.Equal("Bob", players[1].GetProperty("name").GetString());
        Assert.Equal(1, players[1].GetProperty("seat").GetInt32());
    }

    [Fact]
    public void SerializeToJson_DeckList()
    {
        var replay = CreateFullReplay();
        var json = FileWriter.SerializeToJson(replay);
        var doc = JsonDocument.Parse(json);
        var decklist = doc.RootElement.GetProperty("header").GetProperty("decklist");

        Assert.Equal(3, decklist.GetProperty("mainboard").GetArrayLength());
        Assert.Equal(2, decklist.GetProperty("sideboard").GetArrayLength());
        Assert.Equal("Lightning Bolt", decklist.GetProperty("mainboard")[0].GetString());
    }

    [Fact]
    public void SerializeToJson_SideboardChangesNull_ExcludedFromJson()
    {
        var replay = CreateFullReplay();
        var json = FileWriter.SerializeToJson(replay);
        var doc = JsonDocument.Parse(json);
        var header = doc.RootElement.GetProperty("header");

        // sideboard_changes should be null in JSON
        Assert.True(header.TryGetProperty("sideboard_changes", out var sbProp));
        Assert.Equal(JsonValueKind.Null, sbProp.ValueKind);
    }

    [Fact]
    public void SerializeToJson_SideboardChangesPresent()
    {
        var replay = CreateFullReplay();
        // Modify to add sideboard changes (simulate game 2)
        var replayG2 = new ReplayData
        {
            Header = new ReplayHeader
            {
                GameId = replay.Header.GameId,
                GameNumber = 2,
                Players = replay.Header.Players,
                StartTime = replay.Header.StartTime,
                Complete = true,
                SideboardChanges = new SideboardChanges
                {
                    In = new List<string> { "Rest in Peace" },
                    Out = new List<string> { "Lightning Bolt" },
                },
            },
            Timeline = new List<TimelineEntry>(),
        };

        var json = FileWriter.SerializeToJson(replayG2);
        var doc = JsonDocument.Parse(json);
        var sb = doc.RootElement.GetProperty("header").GetProperty("sideboard_changes");

        Assert.Equal(1, sb.GetProperty("in").GetArrayLength());
        Assert.Equal("Rest in Peace", sb.GetProperty("in")[0].GetString());
        Assert.Equal(1, sb.GetProperty("out").GetArrayLength());
        Assert.Equal("Lightning Bolt", sb.GetProperty("out")[0].GetString());
    }

    // ── Timeline tests ──

    [Fact]
    public void SerializeToJson_TimelineOrdering()
    {
        var replay = CreateFullReplay();
        var json = FileWriter.SerializeToJson(replay);
        var doc = JsonDocument.Parse(json);
        var timeline = doc.RootElement.GetProperty("timeline");

        Assert.Equal(4, timeline.GetArrayLength());

        // First: snapshot turn 1
        Assert.Equal("snapshot", timeline[0].GetProperty("type").GetString());
        Assert.Equal(1, timeline[0].GetProperty("turn").GetInt32());

        // Second: CastSpell event turn 1
        Assert.Equal("event", timeline[1].GetProperty("type").GetString());
        Assert.Equal(1, timeline[1].GetProperty("turn").GetInt32());
        Assert.Equal("CastSpell", timeline[1].GetProperty("event").GetProperty("type").GetString());

        // Third: LifeChange event turn 1
        Assert.Equal("event", timeline[2].GetProperty("type").GetString());
        Assert.Equal("LifeChange", timeline[2].GetProperty("event").GetProperty("type").GetString());

        // Fourth: snapshot turn 2
        Assert.Equal("snapshot", timeline[3].GetProperty("type").GetString());
        Assert.Equal(2, timeline[3].GetProperty("turn").GetInt32());
    }

    [Fact]
    public void SerializeToJson_SnapshotEntryFormat()
    {
        var replay = CreateFullReplay();
        var json = FileWriter.SerializeToJson(replay);
        var doc = JsonDocument.Parse(json);
        var snapshot = doc.RootElement.GetProperty("timeline")[0];

        Assert.Equal("snapshot", snapshot.GetProperty("type").GetString());
        Assert.Equal(1, snapshot.GetProperty("turn").GetInt32());
        Assert.Equal("precombat_main", snapshot.GetProperty("phase").GetString());
        Assert.Equal("Alice", snapshot.GetProperty("active_player").GetString());
        Assert.True(snapshot.TryGetProperty("state", out _));
        // Event should not be present on snapshot entries
        Assert.False(snapshot.TryGetProperty("event", out _));
    }

    [Fact]
    public void SerializeToJson_EventEntryFormat()
    {
        var replay = CreateFullReplay();
        var json = FileWriter.SerializeToJson(replay);
        var doc = JsonDocument.Parse(json);
        var eventEntry = doc.RootElement.GetProperty("timeline")[1];

        Assert.Equal("event", eventEntry.GetProperty("type").GetString());
        Assert.Equal(1, eventEntry.GetProperty("turn").GetInt32());
        Assert.Equal("precombat_main", eventEntry.GetProperty("phase").GetString());
        Assert.Equal("Alice", eventEntry.GetProperty("active_player").GetString());

        var evt = eventEntry.GetProperty("event");
        Assert.Equal("CastSpell", evt.GetProperty("type").GetString());
        Assert.True(evt.TryGetProperty("data", out _));
        Assert.True(evt.TryGetProperty("timestamp", out _));

        // State should not be present on event entries
        Assert.False(eventEntry.TryGetProperty("state", out _));
    }

    [Fact]
    public void SerializeToJson_EventDataPayload()
    {
        var replay = CreateFullReplay();
        var json = FileWriter.SerializeToJson(replay);
        var doc = JsonDocument.Parse(json);
        var eventData = doc.RootElement.GetProperty("timeline")[1]
            .GetProperty("event").GetProperty("data");

        Assert.Equal("Lightning Bolt", eventData.GetProperty("cardName").GetString());
        Assert.Equal(0, eventData.GetProperty("playerSeat").GetInt32());
    }

    // ── Card catalog tests ──

    [Fact]
    public void SerializeToJson_CardCatalog()
    {
        var replay = CreateFullReplay();
        var json = FileWriter.SerializeToJson(replay);
        var doc = JsonDocument.Parse(json);
        var catalog = doc.RootElement.GetProperty("card_catalog");

        Assert.True(catalog.TryGetProperty("101", out var bolt));
        Assert.Equal("Lightning Bolt", bolt.GetProperty("name").GetString());
        Assert.Equal("{R}", bolt.GetProperty("mana_cost").GetString());
        Assert.Equal("Instant", bolt.GetProperty("type_line").GetString());

        Assert.True(catalog.TryGetProperty("102", out var snap));
        Assert.Equal("Snapcaster Mage", snap.GetProperty("name").GetString());
    }

    [Fact]
    public void SerializeToJson_EmptyCardCatalog()
    {
        var replay = CreateMinimalReplay();
        var json = FileWriter.SerializeToJson(replay);
        var doc = JsonDocument.Parse(json);
        var catalog = doc.RootElement.GetProperty("card_catalog");
        Assert.Equal(0, catalog.EnumerateObject().Count());
    }

    // ── File I/O tests ──

    [Fact]
    public void WriteReplay_CreatesOutputDirectory()
    {
        Assert.False(Directory.Exists(_tempDir));
        var replay = CreateMinimalReplay();
        _writer.WriteReplay(replay);
        Assert.True(Directory.Exists(_tempDir));
    }

    [Fact]
    public void WriteReplay_CreatesFileWithCorrectName()
    {
        var replay = CreateMinimalReplay();
        var filePath = _writer.WriteReplay(replay);

        Assert.True(File.Exists(filePath));
        Assert.Equal("2026-03-31_Alice-vs-Bob_12345_g1.flashback", Path.GetFileName(filePath));
    }

    [Fact]
    public void WriteReplay_FileContainsValidJson()
    {
        var replay = CreateFullReplay();
        var filePath = _writer.WriteReplay(replay);

        var content = File.ReadAllText(filePath);
        var doc = JsonDocument.Parse(content);

        // Should be able to parse and has expected top-level keys
        Assert.Equal("3.0", doc.RootElement.GetProperty("version").GetString());
        Assert.True(doc.RootElement.TryGetProperty("header", out _));
        Assert.True(doc.RootElement.TryGetProperty("timeline", out _));
        Assert.True(doc.RootElement.TryGetProperty("card_catalog", out _));
    }

    [Fact]
    public void WriteReplay_OutputDirectoryConfigurable()
    {
        var customDir = Path.Combine(_tempDir, "custom", "path");
        var customWriter = new FileWriter(customDir);

        Assert.Equal(customDir, customWriter.OutputDirectory);

        var replay = CreateMinimalReplay();
        var filePath = customWriter.WriteReplay(replay);

        Assert.StartsWith(customDir, filePath);
        Assert.True(File.Exists(filePath));
    }

    [Fact]
    public void WriteReplay_DefaultOutputDirectory_IsAppDataFlashbackReplays()
    {
        var defaultWriter = new FileWriter();
        var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        var expectedDir = Path.Combine(appData, "Flashback", "replays");
        Assert.Equal(expectedDir, defaultWriter.OutputDirectory);
    }

    // ── Integration: full round-trip test ──

    [Fact]
    public void Integration_FullReplay_WriteThenReadAndValidate()
    {
        var replay = CreateFullReplay();
        var filePath = _writer.WriteReplay(replay);

        // Read back and validate complete v3 structure
        var content = File.ReadAllText(filePath);
        var doc = JsonDocument.Parse(content);
        var root = doc.RootElement;

        // Version
        Assert.Equal("3.0", root.GetProperty("version").GetString());

        // Header
        var header = root.GetProperty("header");
        Assert.Equal(99999, header.GetProperty("game_id").GetInt32());
        Assert.Equal("Modern", header.GetProperty("format").GetString());
        Assert.True(header.GetProperty("complete").GetBoolean());
        Assert.Equal(2, header.GetProperty("players").GetArrayLength());
        Assert.Equal("Alice", header.GetProperty("result").GetProperty("winner").GetString());
        Assert.Equal(3, header.GetProperty("decklist").GetProperty("mainboard").GetArrayLength());

        // Timeline — verify interleaved ordering
        var timeline = root.GetProperty("timeline");
        Assert.Equal(4, timeline.GetArrayLength());

        // snapshot → event → event → snapshot
        Assert.Equal("snapshot", timeline[0].GetProperty("type").GetString());
        Assert.Equal("event", timeline[1].GetProperty("type").GetString());
        Assert.Equal("event", timeline[2].GetProperty("type").GetString());
        Assert.Equal("snapshot", timeline[3].GetProperty("type").GetString());

        // Verify timeline entries reference correct turns
        Assert.Equal(1, timeline[0].GetProperty("turn").GetInt32());
        Assert.Equal(1, timeline[1].GetProperty("turn").GetInt32());
        Assert.Equal(1, timeline[2].GetProperty("turn").GetInt32());
        Assert.Equal(2, timeline[3].GetProperty("turn").GetInt32());

        // Card catalog
        var catalog = root.GetProperty("card_catalog");
        Assert.Equal(2, catalog.EnumerateObject().Count());
        Assert.Equal("Lightning Bolt", catalog.GetProperty("101").GetProperty("name").GetString());
        Assert.Equal("{R}", catalog.GetProperty("101").GetProperty("mana_cost").GetString());
    }

    [Fact]
    public void Integration_IncompleteGame_WritesPartialReplay()
    {
        var replay = new ReplayData
        {
            Header = new ReplayHeader
            {
                GameId = 55555,
                GameNumber = 1,
                Players = new List<PlayerInfo>
                {
                    new() { Name = "Alice", Seat = 0 },
                    new() { Name = "Bob", Seat = 1 },
                },
                StartTime = new DateTimeOffset(2026, 3, 31, 10, 0, 0, TimeSpan.Zero),
                EndTime = null,
                Result = null,
                Complete = false,
            },
            Timeline = new List<TimelineEntry>
            {
                new()
                {
                    Type = "snapshot",
                    Snapshot = new Snapshot
                    {
                        Turn = 1,
                        Phase = "untap",
                        ActivePlayer = "Alice",
                        State = new Dictionary<string, object>(),
                    },
                },
            },
        };

        var filePath = _writer.WriteReplay(replay);
        var content = File.ReadAllText(filePath);
        var doc = JsonDocument.Parse(content);
        var header = doc.RootElement.GetProperty("header");

        Assert.False(header.GetProperty("complete").GetBoolean());
        Assert.True(header.TryGetProperty("end_time", out var endTimeEl));
        Assert.Equal(JsonValueKind.Null, endTimeEl.ValueKind);
        Assert.True(header.TryGetProperty("result", out var resultEl));
        Assert.Equal(JsonValueKind.Null, resultEl.ValueKind);
    }

    // ── Edge cases ──

    [Fact]
    public void SerializeToJson_EmptyTimeline()
    {
        var replay = CreateMinimalReplay();
        var json = FileWriter.SerializeToJson(replay);
        var doc = JsonDocument.Parse(json);
        Assert.Equal(0, doc.RootElement.GetProperty("timeline").GetArrayLength());
    }

    [Fact]
    public void SanitizeFileName_RemovesInvalidCharacters()
    {
        Assert.Equal("PlayerName", FileWriter.SanitizeFileName("Player<Name>"));
        Assert.Equal("Test_Name", FileWriter.SanitizeFileName("Test Name"));
        Assert.Equal("Unknown", FileWriter.SanitizeFileName(""));
    }

    [Fact]
    public void WriteReplay_NullReplay_ThrowsArgumentNull()
    {
        Assert.Throws<ArgumentNullException>(() => _writer.WriteReplay(null!));
    }

    [Fact]
    public void SerializeToJson_NullReplay_ThrowsArgumentNull()
    {
        Assert.Throws<ArgumentNullException>(() => FileWriter.SerializeToJson(null!));
    }
}
