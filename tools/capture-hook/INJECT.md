# Capture Hook Injection Guide

## Prerequisites
- dnSpyEx (.NET Framework version)
- MTGO running and logged in
- Build output in `bin/Release/net472/`

## Steps

### 1. Attach dnSpy to MTGO
Debug → Attach to Process (Ctrl+Alt+P) → select MTGO.exe → Attach

### 2. Open Immediate Window
Debug → Windows → Immediate (Ctrl+Alt+I)

### 3. Load and initialize
Paste these one at a time:

```csharp
System.Reflection.Assembly.LoadFile(@"C:\Users\Tal\flashback-mtgo-replay\tools\capture-hook\bin\Release\net472\0Harmony.dll")
```

```csharp
System.Reflection.Assembly.LoadFile(@"C:\Users\Tal\flashback-mtgo-replay\tools\capture-hook\bin\Release\net472\CaptureHook.dll")
```

```csharp
CaptureHook.Patcher.Init()
```

### 4. Resume MTGO
Debug → Continue (F5)

### 5. Play a game
Monitor file growth in PowerShell:
```powershell
while ($true) { $f = gi "$env:USERPROFILE\Desktop\mtgo-capture\single_game.bin" -ea 0; if ($f) { "{0:N0} bytes, {1}" -f $f.Length, $f.LastWriteTime } else { "waiting..." }; sleep 2 }
```

### 6. Stop capture
Break MTGO in dnSpy (Debug → Break All), then in Immediate:

```csharp
foreach (var a in System.AppDomain.CurrentDomain.GetAssemblies()) { if (a.GetName().Name == "CaptureHook") { a.GetType("CaptureHook.Patcher").GetMethod("Finish").Invoke(null, null); break; } }
```

### 7. Detach
Debug → Detach All

### 8. Validate
```bash
cargo run --bin decode -- ~/Desktop/mtgo-capture/single_game.bin
```

Expected: hundreds of messages, GsMessageMessage (1153) most common.

### 9. Copy to repo
```bash
cp ~/Desktop/mtgo-capture/single_game.bin tests/fixtures/single_game.bin
```
