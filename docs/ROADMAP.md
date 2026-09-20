# Implementation roadmap

Every milestone requires executable evidence, not only code. Current evidence is recorded in [VERIFICATION.md](VERIFICATION.md).

## M0 — Public scaffold

Status: implemented and verified.

- Rust binary using `rmcp` over stdio.
- Logs only on stderr.
- Status/info tools, architecture, threat model, contribution guide, GPL license, and pinned toolchain.

## M1 — KiCad 10 IPC transport

Status: implemented and live-verified against KiCad 10.0.6.

- Pinned official Protobuf schemas and documented provenance.
- Build-time Prost generation with vendored `protoc`.
- Serialized NNG request/reply transport with bounded send/receive timeouts and response size.
- Token rotation and redaction; no token in debug output or errors.
- Read-only transient retries; mutations execute once.
- Tests cover success, malformed response, timeout, busy retry, and redaction.

## M2 — Read-only PCB tools

Status: implemented and live-verified.

Tools:

- `get_active_project`, `get_board_summary`;
- `list_footprints`, `get_footprint`, `list_nets`, `get_selection`;
- `run_drc`.

Responses use stable IDs, explicit nanometre units, bounded cursor pagination, field projection, and output caps. DRC uses constrained `kicad-cli` and returns bounded KiCad JSON.

## M3 — Safe PCB mutations

Status: implemented and live-verified on disposable state/copies.

- Move/rotate footprint and edit allowlisted footprint fields.
- Create/update track, create through-via, delete by stable ID.
- Write mode off by default; canonical project-root allowlist blocks symlink escape.
- Revision conflicts, dry-run receipts, KiCad undo commits, and no mutation retries.
- Explicit save only; DRC can run immediately after edits.

## M4 — Agent resources and prompts

Status: implemented and MCP-Inspector verified.

Resources cover connection status, board summary, latest DRC, project metadata, capabilities, and safety policy. KiCad 11 schematic resources are version-gated.

Prompts implement inspect, guarded-change, and validation workflows. They preserve write opt-in, dry-run, revision, DRC/ERC, and explicit-save requirements.

## M5 — Packaging and compatibility

Status: implemented; cross-platform jobs await execution on GitHub Actions.

- Linux/macOS/Windows CI and release matrices.
- Native archives, CycloneDX SBOMs, SHA-256 checksums, and GitHub provenance attestations.
- KiCad executable plugin manifest validated against official schema v1.
- Compatibility matrix and Hermes, Claude Desktop, VS Code/Copilot, and Inspector examples.
- Packaged binary standalone probe.

## M6 — KiCad 11 expansion

Status: implementation complete against pinned development schemas; live acceptance blocked by absence of a KiCad 11 build.

- Separate v10/v11 generated namespaces.
- Version-gated schematic hierarchy and netlist.
- Version-gated symbol placement mutation.
- Version-gated native STEP export job.
- Headless-compatible socket configuration and standalone probe.
- ERC with a separate committed schematic fixture suite.
- KiCad 10 clients do not see KiCad 11 tools, resources, or prompts.

KiCad 11 remains preview until re-pinned and live-tested against a release. See [COMPATIBILITY.md](COMPATIBILITY.md).

## Definition of done for every tool

- JSON schema and concise agent-facing description.
- Version/capability gate.
- Explicit units for geometry.
- Permission class implied by tool contract and safety checks.
- Unit tests plus live/integration evidence when an available KiCad version is required.
- Bounded responses and stable error mapping.
- No sensitive values in logs or returned errors.
