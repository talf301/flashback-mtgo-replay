// tools/capture-hook/Patcher.cs
//
// Harmony-based capture hook for MTGO protocol traffic.
//
// Hooks SslMessageSocketEventArgs constructor to capture every complete
// framed message dispatched by SslSocketWrapper.CompleteRead. Messages
// arrive after ACK/FAF unwrapping, with the standard wire format:
//   [4-byte i32 total_len | 2-byte u16 opcode | 2-byte u16 typecheck | payload]
//
// The output file is a concatenation of these framed messages — exactly
// the format that `cargo run --bin decode` expects.
//
// Injection method: AppDomainManager
//   The CLR loads our CaptureHookManager class on startup when MTGO.exe.config
//   contains the appDomainManagerAssembly/Type entries. We wait for FlsClient
//   to load, then apply the Harmony patch. No debugger required.
//
// Setup: run install-hook.ps1 to patch MTGO.exe.config and copy DLLs.
// Teardown: run uninstall-hook.ps1 to restore the original config.

using System;
using System.IO;
using System.Reflection;
using System.Threading;
using HarmonyLib;

namespace CaptureHook
{
    /// <summary>
    /// AppDomainManager that bootstraps the capture hook inside MTGO's process.
    /// The CLR instantiates this automatically on startup via MTGO.exe.config.
    /// </summary>
    public class CaptureHookManager : AppDomainManager
    {
        public override void InitializeNewDomain(AppDomainSetup appDomainInfo)
        {
            base.InitializeNewDomain(appDomainInfo);

            // Register assembly resolve handler so Harmony can find its dependencies
            AppDomain.CurrentDomain.AssemblyResolve += OnAssemblyResolve;

            // FlsClient may not be loaded yet at startup. Wait for it on a background thread.
            var thread = new Thread(WaitAndPatch)
            {
                IsBackground = true,
                Name = "CaptureHook-Init"
            };
            thread.Start();
        }

        private static Assembly OnAssemblyResolve(object sender, ResolveEventArgs args)
        {
            var name = new AssemblyName(args.Name).Name;
            var dir = Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location);
            var path = Path.Combine(dir, name + ".dll");
            if (File.Exists(path))
                return Assembly.LoadFile(path);
            return null;
        }

        private static void WaitAndPatch()
        {
            var logPath = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.Desktop),
                "mtgo-capture", "hook_log.txt");
            try
            {
                Directory.CreateDirectory(Path.GetDirectoryName(logPath));
                Log(logPath, "CaptureHook loaded, waiting for FlsClient...");

                // Poll for FlsClient assembly (loaded when MTGO connects)
                Assembly flsClient = null;
                for (int i = 0; i < 120; i++) // wait up to 2 minutes
                {
                    Thread.Sleep(1000);
                    flsClient = FindLoadedAssembly("FlsClient");
                    if (flsClient != null) break;
                }

                if (flsClient == null)
                {
                    Log(logPath, "ERROR: FlsClient never loaded after 2 minutes.");
                    return;
                }

                Log(logPath, $"FlsClient found: {flsClient.Location}");
                Patcher.Init(flsClient, logPath);
            }
            catch (Exception ex)
            {
                try { Log(logPath, $"WaitAndPatch error: {ex}"); } catch { }
            }
        }

        private static Assembly FindLoadedAssembly(string name)
        {
            foreach (var asm in AppDomain.CurrentDomain.GetAssemblies())
            {
                if (asm.GetName().Name == name)
                    return asm;
            }
            return null;
        }

        internal static void Log(string path, string message)
        {
            File.AppendAllText(path, $"[{DateTime.UtcNow:O}] {message}\n");
        }
    }

    public static class Patcher
    {
        private static Harmony _harmony;

        public static void Init(Assembly flsClientAsm, string logPath)
        {
            if (_harmony != null) return;

            var captureDir = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.Desktop),
                "mtgo-capture");
            Directory.CreateDirectory(captureDir);
            CaptureLog.OutputPath = Path.Combine(captureDir, "single_game.bin");

            _harmony = new Harmony("com.flashback.capturehook");

            var targetType = flsClientAsm.GetType("FlsClient.Sockets.Events.SslMessageSocketEventArgs");
            if (targetType == null)
            {
                CaptureHookManager.Log(logPath, "ERROR: SslMessageSocketEventArgs type not found.");
                _harmony = null;
                return;
            }

            var ctor = targetType.GetConstructors()[0];
            var postfix = new HarmonyMethod(
                typeof(SslMessageSocketEventArgsPatch).GetMethod(
                    nameof(SslMessageSocketEventArgsPatch.Postfix),
                    BindingFlags.Public | BindingFlags.Static));
            _harmony.Patch(ctor, postfix: postfix);

            CaptureHookManager.Log(logPath, $"Patched {targetType.FullName}..ctor — capturing to {CaptureLog.OutputPath}");

            // Auto-finish on process exit
            AppDomain.CurrentDomain.ProcessExit += (s, e) =>
            {
                CaptureLog.Close();
                CaptureHookManager.Log(logPath,
                    $"Process exit. Messages: {CaptureLog.MessageCount}, Hits: {CaptureLog.HitCount}");
            };
        }

        public static string Status()
        {
            return $"Hits: {CaptureLog.HitCount}, Messages: {CaptureLog.MessageCount}, " +
                   $"File exists: {File.Exists(CaptureLog.OutputPath)}";
        }

        public static void Finish()
        {
            CaptureLog.Close();
            _harmony?.UnpatchAll("com.flashback.capturehook");
            _harmony = null;
        }
    }

    [HarmonyPatch]
    public static class SslMessageSocketEventArgsPatch
    {
        public static void Postfix(byte[] buffer)
        {
            try
            {
                CaptureLog.IncrementHitCount();
                if (buffer != null && buffer.Length > 0)
                {
                    CaptureLog.Write(buffer, 0, buffer.Length);
                }
            }
            catch { }
        }
    }

    internal static class CaptureLog
    {
        public static string OutputPath = "";
        public static long MessageCount => _messageCount;
        public static long HitCount => _hitCount;

        private static readonly object Lock = new object();
        private static FileStream _stream;
        private static long _totalBytes;
        private static long _messageCount;
        private static long _hitCount;

        public static void IncrementHitCount()
        {
            Interlocked.Increment(ref _hitCount);
        }

        public static void Write(byte[] buffer, int offset, int count)
        {
            lock (Lock)
            {
                if (_stream == null)
                {
                    _stream = new FileStream(OutputPath, FileMode.Create, FileAccess.Write, FileShare.Read);
                    _totalBytes = 0;
                    _messageCount = 0;
                }

                _stream.Write(buffer, offset, count);
                _totalBytes += count;
                _messageCount++;

                if (_totalBytes % (64 * 1024) < count)
                {
                    _stream.Flush();
                }
            }
        }

        public static void Close()
        {
            lock (Lock)
            {
                _stream?.Flush();
                _stream?.Dispose();
                _stream = null;
            }
        }
    }
}
