# Contributing

AgentContextMap is in early alpha, so correctness matters more than feature count.

## Start with the right report

- Use the structured **Bug report** issue form for reproducible incorrect results, crashes, or CLI/report regressions.
- Use the **Feature request** form for focused workflow improvements.
- For instruction discovery, scope, precedence, activation, imports, or conflict behavior, include the relevant vendor documentation or a narrow reproducible observation.
- Do not publish vulnerability exploit details in a public issue. Follow [`SECURITY.md`](SECURITY.md).

## Good contributions

- a reproducible scope or precedence bug;
- support for a documented instruction format;
- a false-positive reduction with tests;
- a small CLI or report usability improvement;
- GitHub Actions or packaging hardening;
- documentation that links to the relevant agent/vendor behavior.

## Before opening a pull request

Run the core checks:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

If you change `action.yml` or `action-entrypoint.sh`, also run:

```bash
bash -n action-entrypoint.sh
```

The repository CI additionally exercises the release build, CLI behavior, interactive HTML report behavior, and the composite GitHub Action.

## Pull request scope

Keep changes focused. For instruction-resolution bugs, include a minimal repository tree and the target path that produces the unexpected result. Semantics changes should update tests and `docs/SEMANTICS.md` when the supported behavior matrix changes.

Please do not add telemetry, remote AI calls, or execution of repository-provided commands without prior discussion. The default trust model is local, read-only inspection.
