# MTGO Protocol Research Notes

## Research Approach

### Phase 1: Capture Raw Traffic
1. Run packet capture during actual MTGO gameplay
2. Capture multiple game types (Constructed, Limited)
3. Save raw dumps for offline analysis

### Phase 2: Identify Protocol Characteristics
- [ ] Is traffic encrypted? (TLS/SSL)
- [ ] Transport protocol (TCP/UDP/WebSocket?)
- [ ] Port numbers used
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

Update with actual IPs discovered during research:
- Server 1: [IP] [Port]
- Server 2: [IP] [Port]

## Packet Samples

Document discovered packet formats here as they're identified.

## Decoder Updates

When protocol updates are detected:
1. Version the decoder
2. Keep raw dump mode for re-analysis
3. Document changes in CHANGELOG.md
