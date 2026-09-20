#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "usage: $0 PROJECT_OR_KICAD_FILE [SOCKET_PATH]" >&2
  exit 64
fi

project="$(realpath "$1")"
cli="${KICAD_CLI:-kicad-cli}"
mcp="${KICAD_MCP_BIN:-kicad-mcp}"
socket="${2:-${TMPDIR:-/tmp}/kicad/kicad-mcp-headless-$$.sock}"
mkdir -p "$(dirname "$socket")"
rm -f "$socket"
log="${TMPDIR:-/tmp}/kicad-mcp-headless-$$.log"

"$cli" api-server "$project" --socket "$socket" >"$log" 2>&1 &
api_pid=$!
cleanup() {
  kill "$api_pid" 2>/dev/null || true
  wait "$api_pid" 2>/dev/null || true
  rm -f "$socket" "$log"
}
trap cleanup EXIT INT TERM

for _ in $(seq 1 80); do
  if [[ -S "$socket" ]]; then
    break
  fi
  if ! kill -0 "$api_pid" 2>/dev/null; then
    echo "KiCad headless API server exited before creating its socket:" >&2
    cat "$log" >&2
    exit 1
  fi
  sleep 0.25
done

if [[ ! -S "$socket" ]]; then
  echo "timed out waiting for KiCad headless IPC socket: $socket" >&2
  cat "$log" >&2
  exit 1
fi

export KICAD_API_SOCKET="$socket"
export KICAD_MCP_PROJECT_ROOT="${KICAD_MCP_PROJECT_ROOT:-$(dirname "$project")}"
"$mcp"
