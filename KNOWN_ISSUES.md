# Known Issues and Limitations

## Architecture Transition

The project has transitioned from a Rust-based network protocol decode pipeline to a C#/MTGOSDK-based recorder. The previous known issues related to the Rust decode pipeline (protocol decoding, StateBuf diffs, chat-based enrichment, phantom players, etc.) are no longer applicable.

See [the redesign spec](docs/superpowers/specs/2026-03-31-flashback-mtgosdk-redesign.md) for the new architecture.

---

## Web Viewer Gaps

- No combat pairing display (attacker-blocker grouping) — planned for v3 viewer update
- No deck list panel — planned for v3 viewer update
- No mana pool display — planned for v3 viewer update
- No counter type display on cards — planned for v3 viewer update
- Zones are created dynamically from actions

---

## Recorder (In Development)

The C# recorder using MTGOSDK is not yet implemented. See the design spec for planned error handling and edge cases.
