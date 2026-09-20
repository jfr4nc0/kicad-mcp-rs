# Compatibility

| KiCad | PCB inspect | PCB mutate | DRC/export | Schematic IPC | Native IPC jobs | Status |
|---|---:|---:|---:|---:|---:|---|
| 9.x | Not tested | Not tested | CLI only | No | No | Unsupported |
| 10.0.6 | Yes | Yes, opt-in | `kicad-cli` | No | No | Primary target |
| 10.x other | Expected | Expected | `kicad-cli` | No | No | Best effort |
| 11 preview | Yes | Yes, opt-in | CLI + native STEP job | Hierarchy, nets, symbol placement | STEP | Preview |

KiCad 10 and earlier require a running GUI. KiCad 11-only tools are removed from `tools/list` unless the server detects a connected KiCad major version of at least 11 at startup. Restart the MCP server after changing the connected KiCad instance.

Platform transport:

- Linux/macOS: NNG over the local Unix socket supplied by `KICAD_API_SOCKET`.
- Windows: NNG named-pipe endpoint supplied by `KICAD_API_SOCKET`; CI verifies compilation, but a live Windows KiCad smoke test remains release-gated.
