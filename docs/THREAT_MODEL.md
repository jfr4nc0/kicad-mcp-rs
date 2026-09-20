# Threat model

## Protected assets

- KiCad project files and unsaved editor state.
- The KiCad API token and local IPC endpoint.
- Files outside the explicitly approved project root.
- Manufacturing outputs and user trust in validation results.

## Primary threats

- A model issues destructive or stale mutations.
- Prompt content attempts path traversal or shell injection.
- Sensitive environment variables leak through logs or tool results.
- Concurrent requests reorder KiCad operations.
- Automatic retries duplicate a mutation.
- A malicious local process impersonates or races the KiCad endpoint.
- Huge boards or outputs exhaust memory/context limits.

## Required controls

- Read-only by default; explicit environment opt-in for writes.
- Canonicalize and enforce an allowed project root.
- Never invoke a shell for `kicad-cli`; pass an argument vector.
- One serialized IPC connection.
- Precondition/revision checks for mutations.
- No blind mutation retries.
- Token redaction and structured errors.
- Request timeouts, response size limits, and pagination.
- Read-back receipts and explicit DRC/ERC validation.
- Local stdio transport first; remote transport is out of initial scope.

## Trust boundary

The MCP client and model are not trusted to provide safe paths, fresh state, valid electrical intent, or manufacturing-ready designs. KiCad validates file/model operations, while project-specific electrical correctness still requires human engineering review.
