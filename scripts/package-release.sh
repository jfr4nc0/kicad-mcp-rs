#!/usr/bin/env bash
set -euo pipefail

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n1)"
target="${CARGO_BUILD_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
name="kicad-mcp-${version}-${target}"
binary="kicad-mcp"
[[ "$target" == *windows* ]] && binary="kicad-mcp.exe"

build_args=(--release --locked)
if [[ -n "${CARGO_BUILD_TARGET:-}" ]]; then
  build_args+=(--target "$CARGO_BUILD_TARGET")
fi
cargo build "${build_args[@]}"
root="release"
if [[ -n "${CARGO_BUILD_TARGET:-}" ]]; then
  root="target/$CARGO_BUILD_TARGET/release"
else
  root="target/release"
fi
rm -rf "dist/$name"
mkdir -p "dist/$name/bin" "dist/$name/docs"
cp "$root/$binary" "dist/$name/bin/"
cp scripts/run-headless.sh "dist/$name/bin/run-headless"
cp README.md LICENSE SECURITY.md "dist/$name/"
cp plugin/plugin.json "dist/$name/"
cp docs/CLIENTS.md docs/COMPATIBILITY.md docs/PROTOBUF_PROVENANCE.md "dist/$name/docs/"

python3 - "$name" <<'PY'
import gzip
import pathlib
import tarfile
import sys

name = sys.argv[1]
root = pathlib.Path("dist")
source = root / name
archive = root / f"{name}.tar.gz"

with archive.open("wb") as raw:
    with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as output:
            for path in [source, *sorted(source.rglob("*"), key=lambda item: item.as_posix())]:
                relative = path.relative_to(root).as_posix()
                info = output.gettarinfo(str(path), arcname=relative)
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                info.mtime = 0
                if info.isdir():
                    info.mode = 0o755
                    output.addfile(info)
                elif info.isfile():
                    executable = path.name in {"kicad-mcp", "kicad-mcp.exe", "run-headless"}
                    info.mode = 0o755 if executable else 0o644
                    with path.open("rb") as payload:
                        output.addfile(info, payload)
                else:
                    raise SystemExit(f"unsupported release entry: {path}")
PY
if command -v cargo-cyclonedx >/dev/null 2>&1; then
  cargo cyclonedx --format json --override-filename "${name}.cdx"
  mv "${name}.cdx.json" "dist/${name}.cdx.json"
else
  echo "cargo-cyclonedx is required to produce the release SBOM" >&2
  exit 1
fi
python3 - "$name" <<'PY'
import hashlib, pathlib, sys
name = sys.argv[1]
root = pathlib.Path("dist")
paths = [root / f"{name}.tar.gz", root / f"{name}.cdx.json"]
with (root / f"{name}.sha256").open("w", encoding="utf-8") as output:
    for path in paths:
        output.write(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n")
PY
"dist/$name/bin/$binary" --probe >/dev/null
printf 'Packaged %s\n' "dist/$name.tar.gz"
