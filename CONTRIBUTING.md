# Contributing

AgentContextMap is in early alpha, so correctness matters more than feature count.

## Good contributions

- a reproducible scope or precedence bug;
- support for a documented instruction format;
- a false-positive reduction with tests;
- a small CLI or report usability improvement;
- documentation that links to the relevant agent/vendor behavior.

## Before opening a pull request

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Keep changes focused. For instruction-resolution bugs, include a minimal repository tree and the target path that produces the unexpected result.

Please do not add telemetry, remote AI calls, or execution of repository-provided commands without prior discussion. The default trust model is local, read-only inspection.
