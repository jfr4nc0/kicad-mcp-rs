# Protobuf provenance

The checked-in schemas are generated upstream artifacts and remain covered by KiCad's GPL-3.0-or-later licensing.

## KiCad 10

- Upstream: https://github.com/KiCad/kicad-source-mirror
- Tag: `10.0.6`
- Commit: `caf7377e9cb6fa1535ec3596dcb8c99bf44a996e`
- Source archive SHA-256: `8b236345688d4f2a3d37a9ba8ab4efb4ce6651da40f40b835966df40fd17014c`
- Imported subtree: `api/proto`
- Local destination: `proto/` except `proto/kicad11-preview/`

## KiCad 11 preview

- Upstream: https://github.com/KiCad/kicad-source-mirror
- Development branch snapshot: `master`
- Commit observed when provenance was recorded: `3c96faea994761c78c7a41a36317d92faf45602e`
- Imported subtree: `api/proto`
- Local destination: `proto/kicad11-preview/`
- Stability: preview only; generated support is runtime-gated and must be validated again against the final KiCad 11 release.

Bindings are generated at build time by `build.rs` using `prost-build` and vendored `protoc`. Do not edit generated Rust in `target/`.
