using System.Text.Json;

namespace FlashbackRecorder;

/// <summary>
/// Simple settings manager for the Flashback Recorder.
/// Reads/writes a JSON config file in the user's AppData folder.
/// </summary>
public sealed class Settings
{
    private static readonly string DefaultConfigDir =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "FlashbackRecorder");

    private static readonly string ConfigPath =
        Path.Combine(DefaultConfigDir, "settings.json");

    /// <summary>Directory where .flashback replay files are saved.</summary>
    public string OutputDirectory { get; set; } =
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments), "FlashbackReplays");

    /// <summary>Whether to show desktop notifications when a game is saved.</summary>
    public bool ShowNotifications { get; set; } = true;

    /// <summary>Load settings from disk, or return defaults if file doesn't exist.</summary>
    public static Settings Load()
    {
        try
        {
            if (File.Exists(ConfigPath))
            {
                var json = File.ReadAllText(ConfigPath);
                return JsonSerializer.Deserialize<Settings>(json) ?? new Settings();
            }
        }
        catch
        {
            // Corrupted config — fall back to defaults.
        }
        return new Settings();
    }

    /// <summary>Persist current settings to disk.</summary>
    public void Save()
    {
        Directory.CreateDirectory(DefaultConfigDir);
        var json = JsonSerializer.Serialize(this, new JsonSerializerOptions { WriteIndented = true });
        File.WriteAllText(ConfigPath, json);
    }
}
