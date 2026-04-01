namespace FlashbackRecorder;

/// <summary>
/// Minimal entry point for the Flashback Recorder.
/// Game session management, file writing, and UI are added in later tasks.
/// </summary>
public static class Program
{
    public static async Task Main(string[] args)
    {
        using var cts = new CancellationTokenSource();
        Console.CancelKeyPress += (_, e) => { e.Cancel = true; cts.Cancel(); };

        using IMtgoClient client = new MtgoClient();

        Console.WriteLine("Flashback Recorder — waiting for MTGO...");
        await client.ConnectAsync(cts.Token);
        Console.WriteLine("Attached to MTGO.");

        // Keep alive until Ctrl+C. Session manager will hook into events.
        try
        {
            await Task.Delay(Timeout.Infinite, cts.Token);
        }
        catch (OperationCanceledException)
        {
            // Normal shutdown.
        }

        Console.WriteLine("Shutting down.");
    }
}
