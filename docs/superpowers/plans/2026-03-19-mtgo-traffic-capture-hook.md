# MTGO Traffic Capture via .NET Hooking

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Capture decrypted MTGO protocol bytes by hooking `FlsClient.dll`'s TLS receive path using Harmony, producing a `single_game.bin` file that the Phase A `decode` tool can parse.

**Architecture:** Build a C# class library that uses Harmony to postfix-patch the method in `FlsClient.Sockets.SslSocketWrapper` that delivers decrypted bytes. The patch appends all received bytes to a file. Injection into the running MTGO process is done via dnSpy's debugger Immediate window. The output file is raw server→client application-layer bytes — exactly the format the `decode` CLI expects.

**Tech Stack:** C# (.NET Framework, matching MTGO's target), Harmony 2.x (NuGet), dnSpy (debugger + decompiler), ILSpy (MCP server for reconnaissance).

**Prerequisite:** Phase A capture harness is complete (`cargo run --bin decode` works).

---

## Overview of Approach

MTGO is a .NET Framework ClickOnce app. All network traffic goes through TLS on port 7770. Rather than intercepting at the TLS layer (which requires key material or cert patching), we hook **inside** the process at the point where bytes have already been decrypted by `SslStream` and are being handed to the message pump.

The approach has three phases:
1. **Recon** — Use ILSpy to find the exact method to hook in `FlsClient.dll`
2. **Build** — Create a Harmony patcher DLL
3. **Capture** — Inject the patcher into MTGO via dnSpy, play a game, collect output

---

## File Structure

**Create (on Windows machine):**
- `tools/capture-hook/CaptureHook.csproj` — C# class library project
- `tools/capture-hook/Patcher.cs` — Harmony patch entry point + postfix patch
- `tools/capture-hook/.gitignore` — build output

**Output:**
- `tests/fixtures/single_game.bin` — captured game bytes (copied back to dev machine)

---

## Task 1: Reconnaissance — Find the Hook Target

**Goal:** Identify the exact method in `FlsClient.dll` to patch. We need the method where decrypted bytes arrive from the TLS socket.

**What we know from PROTOCOL_RESEARCH.md:**
- `FlsClient.dll` contains the network layer
- `FlsClient.Sockets.SslSocketWrapper` is the class that handles decrypted bytes
- The message pump reads from this wrapper and dispatches by opcode

- [ ] **Step 1: Locate MTGO's install directory**

MTGO is deployed via ClickOnce. Find the install directory:

```powershell
# PowerShell — find the MTGO ClickOnce directory
$appsDir = Join-Path $env:LOCALAPPDATA "Apps\2.0"
Get-ChildItem -Path $appsDir -Recurse -Filter "FlsClient.dll" | Select-Object -First 1 -ExpandProperty DirectoryName
```

Save this path — you'll need it for ILSpy and for building the patcher.

- [ ] **Step 2: Load FlsClient.dll in ILSpy and find SslSocketWrapper**

Open `FlsClient.dll` from the MTGO install directory. Navigate to:
- Namespace: `FlsClient.Sockets` (or search for `SslSocketWrapper`)
- Look for the class that wraps `System.Net.Security.SslStream`

**What to look for:**
- A method that calls `SslStream.Read()` or `SslStream.ReadAsync()` and returns the decrypted bytes
- It might be named `Read`, `Receive`, `ReadBytes`, `ReadMessage`, or similar
- It likely fills a `byte[]` buffer or returns one
- There may be a synchronous and async variant — we want the one that's actually called

**Record these details:**
```
Namespace:     ??? (likely FlsClient.Sockets)
Class:         SslSocketWrapper (confirm exact name)
Method:        ??? (the receive/read method)
Signature:     ??? (return type, parameter types)
Is virtual:    ??? (yes/no)
Is static:     ??? (yes/no — almost certainly no)
```

- [ ] **Step 3: Trace the call chain upward**

From the read method, trace callers to understand the message pump:
1. Who calls `SslSocketWrapper.Read()`?
2. Does the caller process one message at a time or read in bulk?
3. Are bytes delivered as a complete message (post-framing) or as raw stream chunks?

**Critical question:** Does the hook target deliver:
- **(a) Raw TCP stream bytes** (pre-framing) — ideal, this is what `decode` expects
- **(b) Already-framed single messages** (post-framing) — we'd need to re-frame them
- **(c) Something else** (e.g., a `Message` object) — we'd need to go lower

If **(b)**, also record the header format so we know if we need to prepend the 8-byte frame header when logging.

If **(c)**, look for a lower-level read method on `SslSocketWrapper` or on the `SslStream` directly.

- [ ] **Step 4: Check for assembly signing**

In ILSpy, check if `FlsClient.dll` is strong-named:
- Look at the assembly metadata for a `PublicKeyToken`
- If it has one, note it — this means other assemblies may reference it by strong name, but won't prevent Harmony patching (Harmony patches at runtime, it doesn't modify the DLL on disk)

- [ ] **Step 5: Determine the .NET Framework version**

Check what framework MTGO targets:
The easiest way is to check in ILSpy: open `MTGO.exe` → Assembly metadata → look for `TargetFramework` or `ImageRuntimeVersion`.

Alternatively in PowerShell (lighter than loading the full assembly):
```powershell
[System.Reflection.AssemblyName]::GetAssemblyName("path\to\MTGO.exe").Version
```

Expected: `.NET Framework 4.x` (likely 4.6.2 or 4.8).

**Record this** — the patcher DLL must target the same framework.

- [ ] **Step 6: Document findings**

Before proceeding, write down:
1. Exact method to hook: `Namespace.Class.Method(params) → return`
2. Whether it delivers raw stream bytes or framed messages
3. .NET Framework version
4. Any surprises (async-only, multiple receive paths, etc.)

---

## Task 2: Build the Harmony Patcher DLL

**Files:**
- Create: `tools/capture-hook/CaptureHook.csproj`
- Create: `tools/capture-hook/Patcher.cs`
- Create: `tools/capture-hook/.gitignore`

**Note:** The code below uses placeholder `FIXME` comments where values depend on Task 1 findings. Replace them with the actual method names/signatures discovered during recon.

- [ ] **Step 1: Create the project**

```powershell
mkdir tools\capture-hook
cd tools\capture-hook
```

```xml
<!-- tools/capture-hook/CaptureHook.csproj -->
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net48</TargetFramework>  <!-- FIXME: match MTGO's framework version -->
    <LangVersion>latest</LangVersion>
    <AssemblyName>CaptureHook</AssemblyName>
    <RootNamespace>CaptureHook</RootNamespace>
  </PropertyGroup>

  <ItemGroup>
    <PackageReference Include="Lib.Harmony" Version="2.3.3" />
  </ItemGroup>
</Project>
```

- [ ] **Step 2: Create .gitignore**

```
bin/
obj/
```

- [ ] **Step 3: Write Patcher.cs**

This is the core file. It has three parts:
1. `Patcher.Init()` — entry point called from dnSpy, applies all Harmony patches
2. `SslSocketWrapperPatch` — the Harmony postfix that logs bytes
3. `CaptureLog` — thread-safe file writer

```csharp
// tools/capture-hook/Patcher.cs

using System;
using System.IO;
using System.Reflection;
using HarmonyLib;

namespace CaptureHook
{
    /// <summary>
    /// Entry point for the capture hook. Call Init() from dnSpy's Immediate window
    /// after attaching to the MTGO process.
    /// </summary>
    public static class Patcher
    {
        private static Harmony _harmony;

        /// <summary>
        /// Apply all Harmony patches. Safe to call multiple times (idempotent).
        /// </summary>
        public static void Init()
        {
            if (_harmony != null)
            {
                Console.WriteLine("[CaptureHook] Already initialized.");
                return;
            }

            var captureDir = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.Desktop),
                "mtgo-capture");
            Directory.CreateDirectory(captureDir);
            CaptureLog.OutputPath = Path.Combine(captureDir, "single_game.bin");
            CaptureLog.MetaPath = Path.Combine(captureDir, "capture_meta.txt");

            _harmony = new Harmony("com.flashback.capturehook");

            // --- Find the target method via reflection ---
            // FIXME: Replace these strings with the actual namespace/class/method
            // discovered during Task 1 reconnaissance.
            var flsClientAsm = FindLoadedAssembly("FlsClient");
            if (flsClientAsm == null)
            {
                Console.WriteLine("[CaptureHook] ERROR: FlsClient assembly not loaded yet.");
                Console.WriteLine("[CaptureHook] Make sure MTGO has fully started before injecting.");
                return;
            }

            // FIXME: Replace with actual class name from Task 1, Step 2
            var targetType = flsClientAsm.GetType("FlsClient.Sockets.SslSocketWrapper");
            if (targetType == null)
            {
                Console.WriteLine("[CaptureHook] ERROR: SslSocketWrapper type not found.");
                Console.WriteLine("[CaptureHook] Available types in FlsClient.Sockets:");
                foreach (var t in flsClientAsm.GetTypes())
                {
                    if (t.Namespace?.Contains("Sockets") == true)
                        Console.WriteLine($"  {t.FullName}");
                }
                return;
            }

            // FIXME: Replace "Read" with the actual method name from Task 1, Step 2.
            // FIXME: If the method has overloads, specify parameter types to disambiguate:
            //   var targetMethod = AccessTools.Method(targetType, "Read",
            //       new[] { typeof(byte[]), typeof(int), typeof(int) });
            var targetMethod = AccessTools.Method(targetType, "Read");
            if (targetMethod == null)
            {
                Console.WriteLine("[CaptureHook] ERROR: Target method not found.");
                Console.WriteLine("[CaptureHook] Methods on SslSocketWrapper:");
                foreach (var m in targetType.GetMethods(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic))
                {
                    Console.WriteLine($"  {m.ReturnType.Name} {m.Name}({string.Join(", ", Array.ConvertAll(m.GetParameters(), p => $"{p.ParameterType.Name} {p.Name}"))})");
                }
                return;
            }

            // Apply the postfix patch
            var postfix = new HarmonyMethod(typeof(SslSocketWrapperPatch).GetMethod(nameof(SslSocketWrapperPatch.Postfix)));
            _harmony.Patch(targetMethod, postfix: postfix);

            File.WriteAllText(CaptureLog.MetaPath,
                $"Capture started: {DateTime.UtcNow:O}\n" +
                $"Hooked method: {targetMethod.DeclaringType?.FullName}.{targetMethod.Name}\n" +
                $"Output: {CaptureLog.OutputPath}\n");

            Console.WriteLine($"[CaptureHook] Patched {targetMethod.DeclaringType?.FullName}.{targetMethod.Name}");
            Console.WriteLine($"[CaptureHook] Capturing to: {CaptureLog.OutputPath}");
            Console.WriteLine("[CaptureHook] Play a game. When done, call CaptureHook.Patcher.Finish()");
        }

        /// <summary>
        /// Call when done capturing. Flushes and closes the output file.
        /// </summary>
        public static void Finish()
        {
            CaptureLog.Close();
            _harmony?.UnpatchAll("com.flashback.capturehook");
            _harmony = null;
            Console.WriteLine($"[CaptureHook] Capture complete. File: {CaptureLog.OutputPath}");
            Console.WriteLine($"[CaptureHook] Bytes captured: {new FileInfo(CaptureLog.OutputPath).Length}");
        }

        private static Assembly? FindLoadedAssembly(string name)
        {
            foreach (var asm in AppDomain.CurrentDomain.GetAssemblies())
            {
                if (asm.GetName().Name == name)
                    return asm;
            }
            return null;
        }
    }

    /// <summary>
    /// Harmony postfix patch for SslSocketWrapper's receive method.
    ///
    /// IMPORTANT: The parameter names and types below MUST match the target method's
    /// signature exactly. Harmony uses parameter name matching to inject values.
    ///
    /// FIXME: Adapt this postfix based on Task 1 findings. The examples below cover
    /// the two most common .NET socket read patterns:
    ///
    /// Pattern A — Stream.Read(byte[] buffer, int offset, int count) → int bytesRead:
    ///   The method fills a caller-provided buffer and returns bytes read.
    ///   We capture buffer[offset..offset+bytesRead].
    ///
    /// Pattern B — ReadBytes() → byte[]:
    ///   The method returns a new byte array with the received data.
    ///   We capture the entire return value.
    ///
    /// Uncomment/adapt the correct pattern below.
    /// </summary>
    [HarmonyPatch]
    public static class SslSocketWrapperPatch
    {
        // === Pattern A: Read(byte[] buffer, int offset, int count) → int ===
        // Use this if the target method follows the Stream.Read pattern.
        public static void Postfix(byte[] buffer, int offset, int __result)
        {
            if (__result > 0)
            {
                CaptureLog.Write(buffer, offset, __result);
            }
        }

        // === Pattern B: ReadBytes() → byte[] ===
        // Use this if the target method returns a byte array.
        // Comment out Pattern A and uncomment this:
        //
        // public static void Postfix(byte[]? __result)
        // {
        //     if (__result != null && __result.Length > 0)
        //     {
        //         CaptureLog.Write(__result, 0, __result.Length);
        //     }
        // }
    }

    /// <summary>
    /// Thread-safe binary file writer. Appends all received bytes sequentially
    /// to produce the raw server→client stream that the Rust decode tool expects.
    /// </summary>
    internal static class CaptureLog
    {
        public static string OutputPath = "";
        public static string MetaPath = "";

        private static readonly object Lock = new object();
        private static FileStream _stream;
        private static long _totalBytes;

        public static void Write(byte[] buffer, int offset, int count)
        {
            lock (Lock)
            {
                if (_stream == null)
                {
                    _stream = new FileStream(OutputPath, FileMode.Create, FileAccess.Write, FileShare.Read);
                    _totalBytes = 0;
                }

                _stream.Write(buffer, offset, count);
                _totalBytes += count;

                // Flush periodically so we don't lose data if MTGO crashes
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
```

- [ ] **Step 4: Build the DLL**

```powershell
cd tools\capture-hook
dotnet build -c Release
```

Expected: Produces `bin\Release\net48\CaptureHook.dll` (and `0Harmony.dll` alongside it).

**Build prerequisites:**
- .NET SDK 6.0+ (for building SDK-style projects)
- .NET Framework 4.8 Developer Pack (for the `net48` target — download from Microsoft if `dotnet build` complains about missing targeting pack)

If you get framework targeting errors, adjust the `TargetFramework` in the `.csproj` to match MTGO's actual framework version from Task 1.

- [ ] **Step 5: Verify the build output**

```powershell
dir bin\Release\net48\
```

You should see at least:
- `CaptureHook.dll` — our patcher
- `0Harmony.dll` — Harmony library
- `Mono.Cecil.dll`, `MonoMod.Common.dll` — transitive Harmony dependencies (may vary by version)

**All DLLs in this directory are needed for injection.** When loading via dnSpy's Immediate window, .NET won't automatically resolve dependencies from this directory. You'll need to load them in order (dependencies first). The injection steps in Task 3 cover this.

---

## Task 3: Inject into MTGO and Capture

**Tools needed:** dnSpyEx (download from https://github.com/dnSpyEx/dnSpy/releases — the original dnSpy is archived/unmaintained; use the .NET Framework version, not .NET Core)

- [ ] **Step 1: Launch MTGO and log in**

Start MTGO normally. Log in and navigate to a point where you can start a game (e.g., join a match, start a practice game, or open a replay).

Wait until MTGO is fully loaded before proceeding.

- [ ] **Step 2: Attach dnSpy to MTGO**

1. Open dnSpy
2. Debug → Attach to Process (Ctrl+Alt+P)
3. Find `MTGO.exe` in the process list
4. Click Attach

MTGO will pause. That's normal — we'll resume it after injecting.

- [ ] **Step 3: Load the CaptureHook DLL via Immediate window**

In dnSpy's Immediate window (Debug → Windows → Immediate, or Ctrl+Alt+I), load the DLLs in dependency order. **Replace `C:\path\to\` with the actual path to your `bin\Release\net48\` build output.**

```csharp
// 1. Load Harmony's transitive dependencies first (check your build output — names may vary)
System.Reflection.Assembly.LoadFile(@"C:\path\to\Mono.Cecil.dll")
```

```csharp
// 2. Load Harmony
System.Reflection.Assembly.LoadFile(@"C:\path\to\0Harmony.dll")
```

```csharp
// 3. Load our capture hook
System.Reflection.Assembly.LoadFile(@"C:\path\to\CaptureHook.dll")
```

```csharp
// 4. Initialize — applies the Harmony patches
System.Reflection.Assembly.LoadFile(@"C:\path\to\CaptureHook.dll").GetType("CaptureHook.Patcher").GetMethod("Init").Invoke(null, null)
```

If step 1 fails with "file not found", check your build output for the actual dependency DLL names. If Harmony has no transitive dependencies in your version, skip step 1.

Watch the Output window for `[CaptureHook]` messages confirming the patch was applied.

**If you see errors:**
- "FlsClient assembly not loaded" → MTGO hasn't initialized networking yet. Resume, wait, break again.
- "SslSocketWrapper type not found" → The class name differs. Check the listed types and update `Patcher.cs`.
- "Target method not found" → The method name differs. Check the listed methods and update `Patcher.cs`.

- [ ] **Step 4: Resume MTGO**

In dnSpy: Debug → Continue (F5)

MTGO should resume normally.

- [ ] **Step 5: Play one complete game**

1. Start a game (practice match, bot match, or real match — any will work)
2. Play the game through to completion (game over screen)
3. The capture file is being written to `Desktop\mtgo-capture\single_game.bin`

You can monitor the file size to confirm bytes are flowing:
```powershell
# In a separate PowerShell window — watch the file grow
while ($true) {
    $f = Get-Item "$env:USERPROFILE\Desktop\mtgo-capture\single_game.bin" -ErrorAction SilentlyContinue
    if ($f) { Write-Host "$($f.Length) bytes" } else { Write-Host "waiting..." }
    Start-Sleep 2
}
```

- [ ] **Step 6: Stop the capture**

After the game ends, break MTGO again in dnSpy (Debug → Break All) and run in Immediate:

```csharp
// Find the already-loaded CaptureHook assembly and call Finish()
// (Do NOT use Assembly.LoadFile again — it may load a second copy with fresh static fields)
foreach (var a in System.AppDomain.CurrentDomain.GetAssemblies()) { if (a.GetName().Name == "CaptureHook") { a.GetType("CaptureHook.Patcher").GetMethod("Finish").Invoke(null, null); break; } }
```

Check the Output window for the final byte count.

- [ ] **Step 7: Detach dnSpy**

Debug → Detach All. MTGO continues running normally (or close it).

---

## Task 4: Validate the Capture

- [ ] **Step 1: Quick-check the file on Windows**

```powershell
$f = Get-Item "$env:USERPROFILE\Desktop\mtgo-capture\single_game.bin"
Write-Host "File size: $($f.Length) bytes"

# Check first 16 bytes — should start with a valid frame header
$bytes = [System.IO.File]::ReadAllBytes($f.FullName)
$len = [BitConverter]::ToInt32($bytes, 0)
$opcode = [BitConverter]::ToUInt16($bytes, 4)
Write-Host "First message: length=$len, opcode=$opcode"
```

**Good signs:**
- File size is 50KB–5MB (typical for one game)
- First message length is 8–4096 (reasonable)
- First opcode is a known FLS value (e.g., 1145 for GshGameStatusChangeMessage)

**Bad signs:**
- File is 0 bytes → hook didn't fire. Check Task 1 — wrong method?
- File is very small (< 1KB) → partial capture, game may not have started
- First length is negative or > 16MB → not MTGO framing (possibly captured something else, or byte stream is offset)

- [ ] **Step 2: Transfer the file to the dev machine**

Copy `single_game.bin` to `tests/fixtures/single_game.bin` in the repository on your dev machine.

- [ ] **Step 3: Run the decode tool**

```bash
cd /path/to/mtgo-replay-omp
cargo run --bin decode -- tests/fixtures/single_game.bin
```

**Expected output for a successful capture:**
```
Read NNNNN bytes from tests/fixtures/single_game.bin
Parsed NNN messages

Opcode distribution:
   1153  GsMessageMessage                XXX    ← should be most common
   1145  GshGameStatusChangeMessage      X      ← at least 2 (start + end)
   4652  GamePlayStatusMessage           XX     ← may appear if not wrapped
   ...

Payload size stats:
  Total: NNNNN bytes across NNN messages
  Min: N bytes, Max: NNNNN bytes, Avg: NNN bytes
```

**Indicators of a good capture (from Phase A plan):**
- Parsed message count is in the hundreds (200–1000+)
- `GsMessageMessage (1153)` is the most common opcode
- `GshGameStatusChangeMessage (1145)` appears at least twice
- No parse errors
- Payload sizes vary (small for status, large for state updates)

- [ ] **Step 4: Troubleshooting if decode shows problems**

**Problem: 0 messages parsed**
→ The captured bytes are not MTGO framing. Likely causes:
  - Hooked the wrong method (receiving non-protocol data)
  - Captured client→server instead of server→client (or both directions mixed)
  - Data is still encrypted (hook was above TLS, not below it)

Fix: Go back to Task 1, trace the call chain more carefully. Look for where `SslStream.Read` is called and hook just above that return.

**Problem: Messages parse but all opcodes are "(unknown)"**
→ Byte alignment is off. The stream might include extra framing bytes that aren't part of the MTGO protocol (e.g., a length prefix added by the socket wrapper).

Fix: Hex-dump the first 32 bytes of the capture file and compare against the expected 8-byte header format. Look for an offset where valid opcodes start appearing.

**Problem: Only a few messages, then errors**
→ The hook might be capturing both directions (client→server mixed in). Or the stream includes connection setup bytes that aren't message-framed.

Fix: Check if there's a separate method for sending vs receiving, and ensure you're only hooking the receive path.

---

## Task 5: Commit the Capture

- [ ] **Step 1: Add the fixture file**

```bash
git add tests/fixtures/single_game.bin
git commit -m "test: add captured MTGO game session for golden-file testing"
```

- [ ] **Step 2: Add the capture-hook tool**

```bash
git add tools/capture-hook/
git commit -m "tools: add Harmony-based MTGO traffic capture hook"
```

---

## Appendix A: Alternative Injection Methods

If dnSpy's debugger doesn't work (e.g., anti-debug, crashes on attach), try these alternatives:

### Alt 1: SharpDllLoader / LoadLibrary injection

Use any .NET DLL injector that can call a static method in a managed assembly. Search for "managed DLL injector .NET Framework" — several open-source options exist.

### Alt 2: COR_PROFILER environment variable

Set environment variables before launching MTGO to load a profiler DLL that bootstraps our Harmony patches. This requires building a native C++ profiler shim — significantly more complex but doesn't require attaching a debugger.

```powershell
$env:COR_ENABLE_PROFILING = "1"
$env:COR_PROFILER = "{YOUR-GUID-HERE}"
$env:COR_PROFILER_PATH = "C:\path\to\profiler_shim.dll"
# Launch MTGO
```

### Alt 3: Direct IL patching with dnSpy

If runtime injection proves impossible, use dnSpy to directly edit `FlsClient.dll`:
1. Open the DLL in dnSpy
2. Find the receive method
3. Right-click → Edit Method Body
4. Add IL instructions to write bytes to a file
5. File → Save Module → overwrite the original in MTGO's ClickOnce cache

**Caveat:** If the assembly is strong-named, you'll need to strip the strong name first. Also, MTGO updates will overwrite your changes.

## Appendix B: Adapting the Postfix for Different Method Signatures

The `Postfix` method in `SslSocketWrapperPatch` must match the target method's signature. Here are templates for common patterns:

### Stream.Read pattern: `int Read(byte[] buffer, int offset, int count)`
```csharp
public static void Postfix(byte[] buffer, int offset, int __result)
{
    if (__result > 0)
        CaptureLog.Write(buffer, offset, __result);
}
```

### ReadAsync pattern: `Task<int> ReadAsync(byte[] buffer, int offset, int count)`
```csharp
// For async methods, patch the MoveNext on the async state machine instead,
// or patch the synchronous caller that awaits the result.
// Harmony async patching is complex — prefer finding a sync wrapper.
```

### Custom buffer pattern: `void Receive(out byte[] data, out int length)`
```csharp
public static void Postfix(byte[] data, int length)
{
    if (data != null && length > 0)
        CaptureLog.Write(data, 0, length);
}
```

### Message-level pattern: `Message ReadMessage()` (already framed)
```csharp
// If the method returns a Message object, we need to re-serialize it.
// This is less ideal — look for a lower-level byte[] method instead.
// If this is the only option, capture the raw buffer from the Message:
public static void Postfix(object __result)
{
    // Use reflection to get the raw bytes from the message
    var bufferField = __result?.GetType().GetField("_buffer",
        BindingFlags.Instance | BindingFlags.NonPublic);
    if (bufferField?.GetValue(__result) is byte[] buf)
        CaptureLog.Write(buf, 0, buf.Length);
}
```

### Direction filtering (if one method handles both send and receive)
```csharp
// If the same method is used for both directions, check the caller
// or a direction flag. Example using stack trace (slow but works):
public static void Postfix(byte[] buffer, int offset, int __result)
{
    if (__result <= 0) return;
    // Only capture if we're in the receive path
    var stack = new System.Diagnostics.StackTrace();
    if (stack.ToString().Contains("Receive") || stack.ToString().Contains("Read"))
        CaptureLog.Write(buffer, offset, __result);
}
```
