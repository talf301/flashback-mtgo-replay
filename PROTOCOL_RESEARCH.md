# MTGO Protocol Research Notes

## Research Approach

### Phase 1: Capture Raw Traffic
1. Run packet capture during actual MTGO gameplay
2. Capture multiple game types (Constructed, Limited)
3. Save raw dumps for offline analysis

### Phase 2: Identify Protocol Characteristics
- [x] Is traffic encrypted? **YES — TLS**
- [x] Transport protocol: **TCP**
- [x] Port numbers used: **7770** (destination/server port; not the previously assumed 4724/4765)
- [ ] Message boundaries
- [ ] Encoding format (binary, JSON, protobuf, XML?)

### Phase 3: Pattern Recognition
Look for common patterns:
- Game start/end markers
- Card ID formats
- Player identification
- Turn structure
- Action sequences

### Phase 4: Build Decoder
Start with high-frequency events:
- Card draw
- Land play
- Spell cast
- Attack declaration
- Life total changes

## Known MTGO Servers

- **69.174.204.165** — confirmed Daybreak Games (MTGO operator), found via Wireshark capture

## Next Steps for Protocol Decoding

Traffic is TLS-encrypted, so raw packet capture is not directly readable. Two approaches:

### Option A: TLS Session Key Logging (try first)
Set `SSLKEYLOGFILE` before launching MTGO and check if .NET's TLS stack honours it:
```powershell
$env:SSLKEYLOGFILE = "C:\mtgo-keys.log"
# launch MTGO, play a game, then close
```
Then in Wireshark: Edit → Preferences → Protocols → TLS → point to the key log file.
If it works, the decrypted stream will be visible directly.

### Option B: Decompile the Client
MTGO is a .NET application — decompile with **dnSpy** or **ILSpy** to read the protocol
serialization code directly. Look for:
- Classes named `GameMessage`, `Packet`, `Serialize`, `Protocol`
- References to the server IP or port
- Network stream read/write methods

## Packet Samples

Document discovered packet formats here as they're identified.

## Decoder Updates

When protocol updates are detected:
1. Version the decoder
2. Keep raw dump mode for re-analysis
3. Document changes in CHANGELOG.md
