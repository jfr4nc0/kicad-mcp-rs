# Contributing

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Keep MCP protocol output on stdout and all logs on stderr. Never add tests that print or commit a real `KICAD_API_TOKEN`.

## Pull requests

- Keep tool contracts small and version-gated.
- State whether a tool is `inspect`, `validate`, or `mutate`.
- Add tests and document units, coordinate systems, pagination, and output bounds.
- For live KiCad tests, describe the KiCad version and fixture used.
- Do not commit proprietary board files or unredacted API logs.

## License

By contributing, you agree that your contribution is licensed under GPL-3.0-or-later.
