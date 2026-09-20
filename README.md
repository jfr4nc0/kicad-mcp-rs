# kicad-mcp-rs

A local-first Model Context Protocol server for KiCad, written in Rust. It talks to KiCad through the official Protobuf-over-NNG IPC API; it does not rewrite an open design as text.

## Status

Version `0.2.1` implements the M0–M6 surface and adds post-audit security hardening. KiCad 10.0.6 is the primary target. KiCad 11 support is built from a pinned development schema and remains preview until tested against a released KiCad 11 build. See [COMPATIBILITY.md](docs/COMPATIBILITY.md).

## Safety model

- MCP uses stdio; diagnostics use stderr only.
- Reads are bounded and paginated.
- `KICAD_API_TOKEN` is never returned or logged.
- Mutations require both `KICAD_MCP_ALLOW_WRITE=true` and `KICAD_MCP_PROJECT_ROOT`.
- Update/delete operations require a content revision.
- Mutation tools default to dry-run and return structured receipts.
- Mutations never save automatically; `save_active_board` is explicit.
- DRC/ERC and exports use argument arrays, not a shell, and paths are confined to the project root.

## Tool surface

Inspect and validate:

- `kicad_status`, `server_info`, `get_active_project`, `get_board_summary`
- `list_footprints`, `get_footprint`, `list_nets`, `get_selection`
- `run_drc`, `run_erc`, `export_board`

PCB mutation:

- `move_footprint`, `set_footprint_property`
- `create_track`, `update_track`, `create_via`, `delete_item`
- `save_active_board`

KiCad 11 preview (hidden unless KiCad 11+ is detected at startup):

- `schematic_hierarchy`, `schematic_netlist`, `place_symbol`
- `native_export_step`

The server also exposes bounded MCP resources for project/board/DRC/capabilities/safety state and conservative workflow prompts.

## Requirements

- Rust 1.88+ to build from source.
- KiCad 10+ with the IPC API enabled.
- A running PCB Editor for KiCad 10.
- `kicad-cli` for DRC, ERC, and KiCad 10 exports.

## Build

```bash
cargo build --release --locked
cargo test --all-features --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
```

Probe a KiCad-launched or explicitly configured IPC connection:

```bash
target/release/kicad-mcp --probe
```

Run as an MCP stdio server:

```bash
RUST_LOG=info target/release/kicad-mcp
```

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `KICAD_API_SOCKET` | auto-discovered | KiCad NNG socket or named-pipe endpoint |
| `KICAD_API_TOKEN` | empty | KiCad instance authentication token |
| `KICAD_MCP_ALLOW_WRITE` | `false` | Explicit mutation opt-in |
| `KICAD_MCP_PROJECT_ROOT` | unset | Canonical path allowlist for design and output files |
| `KICAD_MCP_MAX_PAGE_SIZE` | bounded internal default | Maximum items per page |
| `KICAD_MCP_MAX_OUTPUT_BYTES` | `65536` | Maximum CLI and MCP response bytes |
| `KICAD_MCP_CLI_TIMEOUT_SECS` | `120` | Hard timeout for each `kicad-cli` subprocess |
| `KICAD_CLI` | discovered | Explicit `kicad-cli` path |

Client examples: [CLIENTS.md](docs/CLIENTS.md).

## KiCad plugin action

`plugin/plugin.json` conforms to KiCad's official IPC plugin schema. Release archives place it beside `bin/kicad-mcp`; installing that directory under KiCad's versioned `plugins` directory adds a probe action. The action checks connectivity and exits; MCP clients normally launch the same binary directly over stdio.

## KiCad 11 preview and headless mode

KiCad 11 development schemas add schematic IPC and native export jobs. These tools are compiled in but omitted from tool/resource/prompt discovery unless the connected KiCad reports major version 11 or newer.

On KiCad 11, `scripts/run-headless.sh` supervises the official headless API server and this MCP process together:

```bash
KICAD_CLI=/path/to/kicad-cli \
KICAD_MCP_BIN=/path/to/kicad-mcp \
  scripts/run-headless.sh /absolute/path/project.kicad_pro
```

The wrapper starts `kicad-cli api-server ... --socket ...`, waits for the NNG socket, exports its path, runs the stdio MCP server, and terminates the API server when MCP exits. Write mode remains disabled unless `KICAD_MCP_ALLOW_WRITE=true` is explicitly supplied. This path is implementation-complete but remains unverified until a compatible KiCad 11 binary is available.

## Packaging

Release automation produces native archives, CycloneDX SBOMs, SHA-256 checksums, and GitHub build-provenance attestations. To package locally:

```bash
cargo install cargo-cyclonedx --locked
bash scripts/package-release.sh
```

## Architecture and provenance

- [Architecture](docs/ARCHITECTURE.md)
- [Threat model](docs/THREAT_MODEL.md)
- [Roadmap and acceptance criteria](docs/ROADMAP.md)
- [Executed verification record](docs/VERIFICATION.md)
- [Protobuf provenance](docs/PROTOBUF_PROVENANCE.md)

Upstream references:

- https://dev-docs.kicad.org/en/apis-and-binding/ipc-api/index.html
- https://dev-docs.kicad.org/en/apis-and-binding/ipc-api/for-addon-developers/
- https://github.com/KiCad/kicad-source-mirror/tree/10.0.6/api/proto
- https://github.com/modelcontextprotocol/rust-sdk

## License

GPL-3.0-or-later.
