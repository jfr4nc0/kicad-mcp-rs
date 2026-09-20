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

Result: all passed (20 tests). The suite contains deterministic discovery, pagination, revision, path/symlink confinement, output-symlink rejection, capability gates, per-item mutation-status rejection, bounded MCP responses, bounded `kicad-cli` execution, NNG request/reply, malformed Protobuf, timeout, transient retry, token rotation, and token-redaction tests.

`cargo audit` scanned 140 locked dependencies against 1,251 RustSec advisories and reported no vulnerabilities.

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
- post-commit track read-back returned a new revision based on KiCad's actual stored item;
- through-via creation succeeded inside an undoable commit;
- explicit save changed the SHA-256 of a disposable fixture copy and returned `saved: true`;
- KiCad 11-only tools, resources, and prompts were absent while connected to KiCad 10.

## Post-audit hardening (`0.2.1`)

The production-readiness audit was revalidated against the current tree. Previously reported `EndCommit`, item-status, timeout, and Clippy findings were already fixed before this pass. This pass additionally verified or added:

- canonical active-document confinement before every board/schematic mutation and explicit save;
- rejection of output symlinks plus use of the canonical output parent for CLI/job paths;
- export to a randomized sibling staging path followed by checked atomic publication, with existing destinations rejected;
- one process-wide mutation workflow lock spanning revision read, commit, mutation, end/drop, and read-back;
- read-back revisions for footprint and track updates, in addition to create read-back;
- KiCad 11 `BeginCommit` requests with the required document-bearing `ItemHeader`;
- hierarchy `path_ids` and optional explicit target-sheet selection for KiCad 11 symbol placement;
- mandatory version/envelope status/item status payloads instead of silent defaults;
- token-rotation preservation and read-only retry on `AS_TOKEN_MISMATCH`;
- reconnect-time socket rediscovery when the previous path disappeared;
- bounded CLI stdout/stderr capture, a configurable hard timeout, and bounded MCP JSON responses;
- bounded KiCad 11 schematic hierarchy traversal;
- reconnect-aware dynamic KiCad 11 tool discovery rather than startup-only route filtering;
- footprint-placement, routing-review, and manufacturing-preflight MCP prompts.

After these changes, a fresh live KiCad 10.0.6 smoke test created and updated a disposable track. Both operations committed, and the update receipt's revision changed after reading the actual item back from KiCad. The editor was then terminated without saving the disposable change.

A live SVG export also completed with exit code `0`, produced a 56,502-byte artifact through the randomized staging path, atomically published it to the requested project-root path, left no staging files, and was removed after verification. A preceding export outside the configured project root was correctly rejected with `PATH_OUTSIDE_PROJECT_ROOT`.

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

Running the package script twice without source changes produced the same archive SHA-256 (`56e3a31895a1b6b2aae0c4652422de109a64a7ae7a9bc7b95e8e06a0be3e35f6`). Archive timestamps, ownership, and modes are normalized. A clean `cargo install --path . --locked --root ...` followed by MCP Inspector `tools/list` also passed and returned 18 KiCad-10-compatible tools while disconnected from KiCad 11.

`plugin/plugin.json` validated against `https://go.kicad.org/api/schemas/v1`.

## KiCad 11 limitation

As of 2026-09-20 the official KiCad Linux download page lists 10.0.6 as the stable release. KiCad 11 code is compiled from the pinned development schema and is strictly runtime-gated. Schematic hierarchy/netlist, symbol placement, headless connection compatibility, and native STEP-job code compile, but live KiCad 11 acceptance cannot be executed until a compatible KiCad 11 build is available in the test environment.
