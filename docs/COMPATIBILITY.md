# Compatibility

| KiCad | PCB inspect | PCB mutate | DRC/export | Schematic IPC | Native IPC jobs | Status |
|---|---:|---:|---:|---:|---:|---|
| 9.x | Not tested | Not tested | CLI only | No | No | Unsupported |
| 10.0.6 | Yes | Yes, opt-in | `kicad-cli` | No | No | Primary target |
| 10.x other | Expected | Expected | `kicad-cli` | No | No | Best effort |
| 11 preview | Compiles; not live-tested | Compiles; not live-tested | CLI + native STEP job; not live-tested | Hierarchy, nets, symbol placement; not live-tested | STEP; not live-tested | Preview |

KiCad 10 and earlier require a running GUI. KiCad 11-only tools are recomputed for every `tools/list`/tool lookup and remain hidden unless the server currently detects a connected KiCad major version of at least 11. Tool execution also enforces the same runtime version gate.

Platform transport:

- Linux/macOS: NNG over the local Unix socket supplied by `KICAD_API_SOCKET`.
- Windows: NNG named-pipe endpoint supplied by `KICAD_API_SOCKET`; CI verifies compilation, but a live Windows KiCad smoke test remains release-gated.
