## What changed

Describe the smallest useful change in this pull request.

## Why

Explain the bug, workflow, or documented agent behavior this addresses.

## Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test --all-targets`
- [ ] Relevant CLI/report behavior was exercised when applicable

## Semantics evidence

If this changes instruction discovery, precedence, activation, imports, or conflict behavior, link the relevant vendor documentation or provide a narrow reproducible observation. Otherwise write `Not applicable`.

## Safety / scope

- [ ] The change does not add telemetry or remote repository-content uploads.
- [ ] The change does not execute repository-provided instructions or commands unless the behavior was explicitly discussed and reviewed.
- [ ] Known limitations or compatibility changes are documented.
