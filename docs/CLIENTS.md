# MCP client configuration

Build or install `kicad-mcp`, then use its absolute path. KiCad must be running with the IPC API enabled. Start the MCP client from the same desktop session so socket discovery works, or set `KICAD_API_SOCKET` and `KICAD_API_TOKEN` in the client environment.

## Hermes Agent

```yaml
mcp_servers:
  kicad:
    command: "/absolute/path/kicad-mcp"
    timeout: 30
    env:
      KICAD_MCP_PROJECT_ROOT: "/absolute/path/to/project"
      KICAD_MCP_ALLOW_WRITE: "false"
```

## Claude Desktop

```json
{
  "mcpServers": {
    "kicad": {
      "command": "/absolute/path/kicad-mcp",
      "env": {
        "KICAD_MCP_PROJECT_ROOT": "/absolute/path/to/project",
        "KICAD_MCP_ALLOW_WRITE": "false"
      }
    }
  }
}
```

## VS Code / Copilot

`.vscode/mcp.json`:

```json
{
  "servers": {
    "kicad": {
      "type": "stdio",
      "command": "/absolute/path/kicad-mcp",
      "env": {
        "KICAD_MCP_PROJECT_ROOT": "${workspaceFolder}",
        "KICAD_MCP_ALLOW_WRITE": "false"
      }
    }
  }
}
```

## MCP Inspector

```bash
npx --yes @modelcontextprotocol/inspector /absolute/path/kicad-mcp
```

## Enabling mutations

Set both:

```text
KICAD_MCP_ALLOW_WRITE=true
KICAD_MCP_PROJECT_ROOT=/canonical/project/root
```

Mutation tools still default to `dry_run: true`, require revisions for updates/deletes, and never save automatically. Inspect, dry-run, execute, run DRC/ERC, review, then call the explicit save tool.
