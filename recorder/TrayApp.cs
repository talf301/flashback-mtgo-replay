using System.Drawing;
using System.Windows.Forms;
using FlashbackRecorder.Models;

namespace FlashbackRecorder;

/// <summary>
/// Windows system tray application for the Flashback Recorder.
/// Shows a tray icon with color states, context menu, and desktop notifications.
/// </summary>
public sealed class TrayApp : IDisposable
{
    private readonly NotifyIcon _notifyIcon;
    private readonly ContextMenuStrip _contextMenu;
    private readonly Settings _settings;
    private readonly IMtgoClient _client;
    private readonly ToolStripMenuItem _statusMenuItem;
    private bool _disposed;

    /// <summary>
    /// Current tray icon state. Setting this property updates the icon color.
    /// </summary>
    public TrayIconState IconState { get; private set; } = TrayIconState.Waiting;

    public TrayApp(IMtgoClient client, Settings settings)
    {
        _client = client ?? throw new ArgumentNullException(nameof(client));
        _settings = settings ?? throw new ArgumentNullException(nameof(settings));

        _contextMenu = BuildContextMenu(out _statusMenuItem);
        _notifyIcon = new NotifyIcon
        {
            Text = "Flashback Recorder",
            Icon = CreateIcon(TrayIconState.Waiting),
            ContextMenuStrip = _contextMenu,
            Visible = true,
        };

        UpdateIconState(TrayIconState.Waiting);
    }

    // ── Icon state management ──

    /// <summary>
    /// Update the tray icon to reflect the current recorder state.
    /// </summary>
    public void UpdateIconState(TrayIconState state)
    {
        IconState = state;
        _notifyIcon.Icon = CreateIcon(state);
        _notifyIcon.Text = state switch
        {
            TrayIconState.Waiting => "Flashback Recorder — Waiting for MTGO",
            TrayIconState.Recording => "Flashback Recorder — Recording",
            TrayIconState.Error => "Flashback Recorder — Error",
            _ => "Flashback Recorder",
        };
        _statusMenuItem.Text = state switch
        {
            TrayIconState.Waiting => "Status: Waiting for game...",
            TrayIconState.Recording => "Status: Recording game",
            TrayIconState.Error => "Status: Error",
            _ => "Status: Unknown",
        };
    }

    /// <summary>
    /// Call when the MTGO client connection state changes.
    /// </summary>
    public void OnConnectionStateChanged(ConnectionState connectionState)
    {
        switch (connectionState)
        {
            case ConnectionState.Disconnected:
            case ConnectionState.WaitingForProcess:
                UpdateIconState(TrayIconState.Waiting);
                break;
            case ConnectionState.Attached:
                UpdateIconState(TrayIconState.Waiting);
                break;
            case ConnectionState.Error:
                UpdateIconState(TrayIconState.Error);
                break;
        }
    }

    /// <summary>
    /// Call when a game session starts recording.
    /// </summary>
    public void OnGameStarted()
    {
        UpdateIconState(TrayIconState.Recording);
    }

    /// <summary>
    /// Call when a game session ends and a replay is saved.
    /// Shows a desktop notification with player names.
    /// </summary>
    public void OnGameSaved(ReplayData replay)
    {
        UpdateIconState(TrayIconState.Waiting);

        if (_settings.ShowNotifications)
        {
            var players = replay.Header.Players;
            var names = players.Count >= 2
                ? $"{players[0].Name} vs {players[1].Name}"
                : players.Count == 1 ? players[0].Name : "Unknown";

            ShowNotification("Game saved", $"Game saved: {names}");
        }
    }

    /// <summary>
    /// Show a balloon tip notification from the tray icon.
    /// </summary>
    public void ShowNotification(string title, string message, ToolTipIcon icon = ToolTipIcon.Info)
    {
        _notifyIcon.BalloonTipTitle = title;
        _notifyIcon.BalloonTipText = message;
        _notifyIcon.BalloonTipIcon = icon;
        _notifyIcon.ShowBalloonTip(3000);
    }

    // ── Context menu ──

    private ContextMenuStrip BuildContextMenu(out ToolStripMenuItem statusItem)
    {
        var menu = new ContextMenuStrip();

        statusItem = new ToolStripMenuItem("Status: Waiting for game...")
        {
            Enabled = false, // Display-only
        };
        menu.Items.Add(statusItem);
        menu.Items.Add(new ToolStripSeparator());

        var openFolder = new ToolStripMenuItem("Open Replay Folder");
        openFolder.Click += OnOpenReplayFolder;
        menu.Items.Add(openFolder);

        var settingsItem = new ToolStripMenuItem("Settings...");
        settingsItem.Click += OnOpenSettings;
        menu.Items.Add(settingsItem);

        menu.Items.Add(new ToolStripSeparator());

        var quit = new ToolStripMenuItem("Quit");
        quit.Click += OnQuit;
        menu.Items.Add(quit);

        return menu;
    }

    private void OnOpenReplayFolder(object? sender, EventArgs e)
    {
        var dir = _settings.OutputDirectory;
        if (!Directory.Exists(dir))
        {
            Directory.CreateDirectory(dir);
        }

        System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
        {
            FileName = dir,
            UseShellExecute = true,
        });
    }

    private void OnOpenSettings(object? sender, EventArgs e)
    {
        using var dialog = new SettingsDialog(_settings);
        dialog.ShowDialog();
    }

    private void OnQuit(object? sender, EventArgs e)
    {
        _notifyIcon.Visible = false;
        Application.Exit();
    }

    // ── Icon rendering ──

    /// <summary>
    /// Create a simple colored circle icon for the tray.
    /// Grey = waiting, Green = recording, Red = error.
    /// </summary>
    internal static Icon CreateIcon(TrayIconState state)
    {
        var color = state switch
        {
            TrayIconState.Waiting => Color.Gray,
            TrayIconState.Recording => Color.LimeGreen,
            TrayIconState.Error => Color.Red,
            _ => Color.Gray,
        };

        using var bitmap = new Bitmap(16, 16);
        using var g = Graphics.FromImage(bitmap);
        g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
        g.Clear(Color.Transparent);
        using var brush = new SolidBrush(color);
        g.FillEllipse(brush, 1, 1, 14, 14);
        // Dark border for visibility
        using var pen = new Pen(Color.FromArgb(60, 60, 60), 1f);
        g.DrawEllipse(pen, 1, 1, 14, 14);

        return Icon.FromHandle(bitmap.GetHicon());
    }

    // ── IDisposable ──

    public void Dispose()
    {
        if (_disposed) return;
        _disposed = true;

        _notifyIcon.Visible = false;
        _notifyIcon.Dispose();
        _contextMenu.Dispose();
    }
}

/// <summary>
/// Icon color states for the system tray.
/// </summary>
public enum TrayIconState
{
    /// <summary>Grey — waiting for MTGO or between games.</summary>
    Waiting,
    /// <summary>Green — actively recording a game.</summary>
    Recording,
    /// <summary>Red — error state (connection lost, etc.).</summary>
    Error,
}

/// <summary>
/// Simple settings dialog for configuring the output directory path.
/// </summary>
internal sealed class SettingsDialog : Form
{
    private readonly Settings _settings;
    private readonly TextBox _outputDirTextBox;

    public SettingsDialog(Settings settings)
    {
        _settings = settings;

        Text = "Flashback Recorder Settings";
        Size = new Size(480, 180);
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MaximizeBox = false;
        MinimizeBox = false;
        StartPosition = FormStartPosition.CenterScreen;

        var label = new Label
        {
            Text = "Replay output directory:",
            Location = new Point(12, 16),
            AutoSize = true,
        };
        Controls.Add(label);

        _outputDirTextBox = new TextBox
        {
            Text = _settings.OutputDirectory,
            Location = new Point(12, 40),
            Width = 350,
        };
        Controls.Add(_outputDirTextBox);

        var browseButton = new Button
        {
            Text = "Browse...",
            Location = new Point(370, 38),
            Width = 80,
        };
        browseButton.Click += OnBrowse;
        Controls.Add(browseButton);

        var saveButton = new Button
        {
            Text = "Save",
            Location = new Point(290, 90),
            Width = 75,
            DialogResult = DialogResult.OK,
        };
        saveButton.Click += OnSave;
        Controls.Add(saveButton);

        var cancelButton = new Button
        {
            Text = "Cancel",
            Location = new Point(375, 90),
            Width = 75,
            DialogResult = DialogResult.Cancel,
        };
        Controls.Add(cancelButton);

        AcceptButton = saveButton;
        CancelButton = cancelButton;
    }

    private void OnBrowse(object? sender, EventArgs e)
    {
        using var dialog = new FolderBrowserDialog
        {
            SelectedPath = _outputDirTextBox.Text,
            Description = "Select replay output directory",
        };
        if (dialog.ShowDialog() == DialogResult.OK)
        {
            _outputDirTextBox.Text = dialog.SelectedPath;
        }
    }

    private void OnSave(object? sender, EventArgs e)
    {
        _settings.OutputDirectory = _outputDirTextBox.Text;
        _settings.Save();
        Close();
    }
}
