# kicad-mcp-rs

A fast, local-first Model Context Protocol (MCP) server for KiCad, written in Rust.

> Early scaffold: the server currently exposes safe discovery/status tools. It does **not** modify KiCad projects yet. See [ROADMAP.md](docs/ROADMAP.md).

## Why

KiCad provides an official language-agnostic IPC API, but no official KiCad-maintained MCP server. This project aims to expose a small, auditable tool surface to AI agents without parsing or rewriting KiCad files behind the application's back.

## Design principles

- Use KiCad's official IPC API (Protobuf over NNG), not ad-hoc file mutation.
- Keep one serialized connection to the running KiCad instance.
- Read-only by default; mutation requires explicit opt-in.
- Return structured, bounded results suitable for agent context windows.
- Validate edits with KiCad and provide clear revision/precondition failures.
- Never expose `KICAD_API_TOKEN` in logs or MCP results.
- Keep the default deployment local over MCP stdio.

## Current tools

- `kicad_status`: discovers the IPC socket and reports safety state without revealing the API token.
- `server_info`: reports server version, transport, and implementation scope.

## Build and run

Requirements:

- Rust 1.88+
- KiCad 10+ (KiCad 10 support is the first target)

```bash
cargo build --release
RUST_LOG=info cargo run
```

The process speaks MCP over stdin/stdout. Logs are emitted only to stderr.

Example client configuration:

```json
{
  "mcpServers": {
    "kicad": {
      "command": "/absolute/path/to/kicad-mcp-rs/target/release/kicad-mcp"
    }
  }
}
```

For Hermes Agent:

```yaml
mcp_servers:
  kicad:
    command: "/absolute/path/to/kicad-mcp-rs/target/release/kicad-mcp"
    timeout: 30
```

## KiCad 10 constraints

According to KiCad's official IPC documentation:

- KiCad 9 and 10 require a running GUI instance.
- IPC plugins in KiCad 9 and 10 are limited to PCB Editor.
- Headless IPC and export/plot support arrive in KiCad 11; for KiCad 10, export automation must use `kicad-cli`.
- Schematic Editor IPC plugin support arrives in KiCad 11.

The MCP capability list will reflect the connected KiCad version instead of advertising unsupported tools.

## Architecture

```text
Agent / MCP client
        |
        | MCP stdio (JSON-RPC)
        v
kicad-mcp-rs
  - tool router
  - capability and safety policy
  - serialized command queue
  - KiCad IPC adapter
        |
        | Protobuf envelopes over NNG IPC
        v
Running KiCad instance
```

See [ARCHITECTURE.md](docs/ARCHITECTURE.md) and [ROADMAP.md](docs/ROADMAP.md).

## Upstream references

- KiCad IPC API overview: https://dev-docs.kicad.org/en/apis-and-binding/ipc-api/index.html
- Add-on developer documentation: https://dev-docs.kicad.org/en/apis-and-binding/ipc-api/for-addon-developers/
- KiCad source mirror and protobuf definitions: https://github.com/KiCad/kicad-source-mirror/tree/10.0.6/api/proto
- Official MCP Rust SDK: https://github.com/modelcontextprotocol/rust-sdk

## Safety

Generated PCB changes can be electrically or physically wrong. This project is not a substitute for ERC/DRC, datasheet review, design review, or engineering judgment. Never manufacture safety-critical hardware from unreviewed agent output.

## License

GPL-3.0-or-later. This choice is compatible with the official KiCad protobuf definitions that the IPC adapter will consume.
