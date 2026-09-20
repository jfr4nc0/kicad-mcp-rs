# Implementation roadmap

Each milestone must finish with executable evidence, not only code or documentation.

## M0 — Public scaffold (current)

Deliverables:

- Rust binary using the official `rmcp` SDK over stdio.
- Safe `kicad_status` and `server_info` tools.
- Architecture, threat model, contribution guide, and pinned toolchain.

Acceptance:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
```

## M1 — KiCad 10 IPC transport spike

Files/modules:

- `build.rs`: generate Rust bindings with `prost-build`.
- `proto/`: pinned KiCad 10.0.6 definitions plus provenance/license notice.
- `src/ipc/{mod.rs,envelope.rs,transport.rs,error.rs}`.
- `tests/ipc_fixture.rs`: deterministic envelope and status-code tests.

Tasks:

1. Pin the KiCad protobuf schema to tag `10.0.6`; document the exact upstream commit and license.
2. Build NNG request/reply transport on Unix sockets and define the Windows named-pipe strategy.
3. Add client identity and token handling without logging secrets.
4. Implement handshake/API-version query against a running PCB Editor.
5. Serialize all calls through one connection and add bounded timeout handling.
6. Retry only safe transient reads; never blindly retry mutations.
7. Capture an opt-in KiCad API log fixture with secrets redacted.

Acceptance:

- A live KiCad 10.0.6 PCB Editor responds to `kicad_status` with `connected: true` and an API version.
- Token mismatch, busy, timeout, malformed response, and restart cases have tests.
- No token appears in normal or debug logs.

## M2 — Read-only PCB tools

Initial tool contracts:

- `get_active_project`
- `get_board_summary`
- `list_footprints`
- `get_footprint`
- `list_nets`
- `get_selection`
- `run_drc` (IPC where available; otherwise a constrained `kicad-cli` path)

Requirements:

- Pagination/cursors for large boards.
- Stable IDs and explicit units in every geometry response.
- Output size limits and optional field selection.
- Capability negotiation based on KiCad version.

Acceptance:

- Golden tests against a small committed fixture board.
- Live smoke test against KiCad 10.0.6.
- MCP Inspector can discover and call every tool.

## M3 — Safe PCB mutations

First mutation set:

- move/rotate a footprint;
- set a footprint property;
- add/update a track or via only when the IPC API supports an unambiguous operation;
- delete by stable object ID.

Guardrails:

- Write mode disabled by default (`KICAD_MCP_ALLOW_WRITE=false`).
- Project-root allowlist.
- Document/revision preconditions to prevent stale-agent writes.
- Dry-run/preview where the API can represent it.
- Structured mutation receipt with affected IDs and before/after summaries.
- No automatic save unless explicitly requested.

Acceptance:

- Mutation tests operate on disposable fixture copies.
- Stale preconditions produce conflicts and zero writes.
- DRC can run immediately after edits and report introduced violations.

## M4 — Agent workflow resources and prompts

Add MCP resources for:

- current board summary;
- DRC report;
- project metadata;
- server capabilities and safety policy.

Add conservative prompts/workflows for:

- inspect → propose → mutate → verify;
- footprint placement review;
- routing review;
- manufacturing preflight.

Acceptance:

- Prompt workflows never bypass write opt-in or validation.
- Resource updates are bounded and do not leak project files outside the configured root.

## M5 — Packaging and compatibility

- Reproducible Linux/macOS/Windows binaries.
- SBOM and checksums for releases.
- GitHub release workflow with provenance/attestations.
- KiCad executable-plugin manifest (`plugin.json`) for optional launch from PCB Editor.
- Compatibility matrix for supported KiCad versions.
- Hermes, Claude Desktop, VS Code/Copilot, and MCP Inspector examples.

Acceptance:

- Clean-machine install test for every release target.
- Binary starts and completes MCP initialization without a Rust toolchain installed.

## M6 — KiCad 11 expansion

Only after KiCad 10 PCB support is stable:

- Schematic Editor tools through the KiCad 11 IPC API.
- Headless IPC execution.
- Native plot/export job support.
- ERC and schematic inspection/mutation contracts.

Acceptance:

- Version-gated tools: KiCad 10 clients never see KiCad 11-only capabilities.
- Separate fixture suites for PCB and schematic workflows.

## Definition of done for every tool

- JSON schema and concise agent-facing description.
- Version/capability gate.
- Unit and coordinate conventions documented.
- Permission class (`inspect`, `validate`, or `mutate`).
- Unit tests plus a live/integration test when KiCad is required.
- Bounded response size and stable error mapping.
- No sensitive data in logs or returned errors.
