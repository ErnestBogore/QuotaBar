# Contributing

Run the complete local verification suite before submitting changes:

```sh
pnpm install
pnpm check
pnpm test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Changes to metering behavior must include a sanitized regression fixture or a unit test. Never commit real Codex session logs, authentication material, prompts, tool output, repository paths, signing keys, or updater private keys.

