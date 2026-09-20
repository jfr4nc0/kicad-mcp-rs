# Verification record

Date: 2026-09-20

## Static quality gate

Executed locally with Rust 1.88-compatible project settings:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-features --locked
cargo build --release --locked
```

Result: all passed (18 tests). The suite contains deterministic discovery, pagination, revision, path/symlink confinement, capability gate, per-item mutation-status rejection, NNG request/reply, malformed Protobuf, timeout, transient retry, and token-redaction tests.

## KiCad 10.0.6 live IPC

Application:

```text
/home/jcanossa/.local/opt/kicad-10.0.6/kicad-10.0.6-x86_64.AppImage
```

Live fixture: `tests/fixtures/minimal/minimal.kicad_pcb`.

Observed through the real Unix NNG socket `/tmp/kicad/api.sock`:

- handshake returned KiCad `10.0.6`;
- `get_board_summary` returned one footprint;
- footprint list returned stable KIID, nanometre geometry, and SHA-256 revision;
- selected-field projection retained `id` and `revision`;
- DRC returned parsed KiCad JSON (not only CLI diagnostics);
- dry-run footprint move returned `committed: false`;
- actual move returned `committed: true` and changed live editor state;
- reuse of the stale revision returned `REVISION_CONFLICT` with no second write;
- track create and track update succeeded inside undoable commits;
- through-via creation succeeded inside an undoable commit;
- explicit save changed the SHA-256 of a disposable fixture copy and returned `saved: true`;
- KiCad 11-only tools, resources, and prompts were absent while connected to KiCad 10.

## MCP Inspector

Inspector CLI `2.7.0` negotiated MCP `2025-11-25`. It successfully exercised:

- strict `tools/list`;
- `kicad_status`;
- `get_board_summary`;
- `list_footprints`;
- `run_drc`;
- `resources/list` and `resources/read`;
- `prompts/list` and `prompts/get`.

The KiCad 10 tool list exposed 18 compatible tools and no KiCad 11-only tools. Inspector reported no schema errors (one portability warning for an optional nullable angle).

## Packaging

`bash scripts/package-release.sh` produced and verified:

- native `.tar.gz` archive;
- CycloneDX JSON SBOM;
- SHA-256 checksum file;
- standalone `--probe` execution from the packaged tree.

`plugin/plugin.json` validated against `https://go.kicad.org/api/schemas/v1`.

## KiCad 11 limitation

As of 2026-09-20 the official KiCad Linux download page lists 10.0.6 as the stable release. KiCad 11 code is compiled from the pinned development schema and is strictly runtime-gated. Schematic hierarchy/netlist, symbol placement, headless connection compatibility, and native STEP-job code compile, but live KiCad 11 acceptance cannot be executed until a compatible KiCad 11 build is available in the test environment.
