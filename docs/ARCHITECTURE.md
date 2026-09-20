# Architecture

## Scope

`kicad-mcp-rs` is a local MCP server that translates a deliberately small set of agent-oriented tools into KiCad IPC API requests. KiCad remains the source of truth and performs all supported design mutations.

## Components

### MCP transport and tool router

The server uses the official Rust MCP SDK (`rmcp`) and stdio transport. Stdout is reserved for protocol frames; diagnostics go to stderr.

### Capability service

At startup and after reconnecting, the server identifies the KiCad/API version and enables only compatible tools. KiCad 10 is PCB-first. KiCad 11 can add schematic and headless/export capabilities.

### Safety policy

Three operation classes are planned:

1. `inspect`: read-only and enabled by default.
2. `validate`: runs checks or computes a proposed change without committing it.
3. `mutate`: disabled unless `KICAD_MCP_ALLOW_WRITE=true` and guarded by document/revision preconditions.

Sensitive values such as `KICAD_API_TOKEN` are consumed by the adapter but never returned or logged.

### Command queue

KiCad processes API requests synchronously in its GUI event loop. The adapter therefore owns exactly one connection and serializes requests. It applies bounded timeouts and retries only status codes documented as transient (`AS_BUSY`, `AS_NOT_READY`, selected timeouts). Mutation calls are not blindly retried.

### KiCad IPC adapter

The adapter will:

- use protobuf definitions pinned to a tested KiCad release tag;
- generate Rust message types at build time with `prost`;
- exchange request/response envelopes through NNG IPC;
- validate response tokens and map KiCad status codes to stable MCP errors;
- reconnect safely when KiCad restarts or its token changes.

### CLI adapter

For KiCad 10 operations absent from IPC (for example plotting/exporting), a narrowly scoped `kicad-cli` adapter may be used. It will execute argument arrays without a shell, restrict paths to an approved project root, and return bounded output.

## Data flow for a mutation

1. Agent calls a mutation tool with project/document identity and precondition.
2. Tool validates schema, scope, units, and write policy.
3. Adapter reads current document/revision state.
4. If the precondition fails, the call returns a conflict without mutation.
5. Request enters the single serialized KiCad queue.
6. KiCad applies the operation.
7. Server reads back the affected object and returns a structured receipt.
8. Agent explicitly requests DRC/ERC or other validation.

## Non-goals

- Editing `.kicad_pcb` or `.kicad_sch` as raw text while KiCad is open.
- Autonomous manufacturing orders.
- Pretending KiCad 10 supports schematic IPC operations that only exist in KiCad 11.
- Remote unauthenticated HTTP transport in the initial releases.
