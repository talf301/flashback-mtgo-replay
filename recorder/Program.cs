using System.Windows.Forms;
using FlashbackRecorder.Models;

namespace FlashbackRecorder;

/// <summary>
/// Entry point for the Flashback Recorder.
/// Launches the system tray application and connects to MTGO.
/// </summary>
public static class Program
{
    [STAThread]
    public static void Main(string[] args)
    {
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        Application.SetHighDpiMode(HighDpiMode.SystemAware);

        var settings = Settings.Load();
        using IMtgoClient client = new MtgoClient();
        using var trayApp = new TrayApp(client, settings);
        var fileWriter = new FileWriter(settings.OutputDirectory);

        // Wire the session manager to update tray state and show notifications.
        using var sessionManager = new GameSessionManager(
            client,
            onReplayComplete: replay =>
            {
                fileWriter.WriteReplay(replay);
                trayApp.OnGameSaved(replay);
            },
            deckListProvider: _ => client.CaptureDeckList(),
            snapshotProvider: (gameId, turn) => client.CaptureSnapshot(turn));

        // Track game start for icon state.
        client.OnGameStatusChange += (_, e) =>
        {
            if (e.Status == GameStatus.Started)
                trayApp.OnGameStarted();
        };

        // Connect to MTGO in a background thread.
        var cts = new CancellationTokenSource();
        _ = Task.Run(async () =>
        {
            try
            {
                trayApp.OnConnectionStateChanged(ConnectionState.WaitingForProcess);
                await client.ConnectAsync(cts.Token);
                trayApp.OnConnectionStateChanged(ConnectionState.Attached);
            }
            catch (OperationCanceledException)
            {
                // Normal shutdown.
            }
            catch (Exception)
            {
                trayApp.OnConnectionStateChanged(ConnectionState.Error);
            }
        });

        // Run the Windows message loop (keeps tray icon alive).
        Application.Run();

        // Cleanup on exit.
        cts.Cancel();
    }
}
