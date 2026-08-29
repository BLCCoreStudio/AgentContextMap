# AgentContextMap

**See what instructions your coding agents actually see.**

AgentContextMap is a local CLI that maps repository instruction files across Codex, Claude Code, Gemini, GitHub Copilot, Cursor, Windsurf, and Cline. It can show the instruction chain for a specific file, flag obvious conflicts and duplicates, estimate context size, and generate a self-contained HTML report.

> **Status:** early alpha. The supported file conventions and deterministic conflict heuristics are intentionally conservative and will evolve as agent tooling changes.

## Why this exists

Modern repositories can contain several overlapping instruction systems at once:

- `AGENTS.md`
- `CLAUDE.md`
- `GEMINI.md`
- `.github/copilot-instructions.md`
- `.github/instructions/*.instructions.md`
- `.cursor/rules/*.mdc`
- `.windsurfrules`
- `.clinerules`

Once those files become nested or pattern-scoped, it gets hard to answer a simple question:

**Which instructions can affect this file right now?**

AgentContextMap makes that visible without calling an AI model or sending repository content anywhere.

## Quick start

Requires Rust 1.74+ while the project is in alpha.

```bash
cargo install --git https://github.com/BLCCoreStudio/AgentContextMap --bin agentcontext
```

Scan a repository:

```bash
agentcontext .
```

Inspect one target path:

```bash
agentcontext . --target src/api/auth.rs
```

Generate a shareable local report:

```bash
agentcontext . --target src/api/auth.rs --html report.html
```

Machine-readable output for CI or other tooling:

```bash
agentcontext . --json
```

Fail CI when a high-confidence conflict is detected:

```bash
agentcontext . --json --fail-on-conflict
```

## Example

```text
AgentContextMap
===============
Root: /work/acme
Target: src/api/auth.rs
Sources: 5 | Approx. tokens: 812 | Findings: 1

[Codex / AGENTS.md]
  1. AGENTS.md              (workspace tree)
  2. src/api/AGENTS.md      (src/api subtree)

[Claude Code]
  1. CLAUDE.md              (workspace tree)

[GitHub Copilot]
  1. .github/copilot-instructions.md  (workspace-wide)

[Cursor]
  1. .cursor/rules/rust.mdc (pattern: **/*.rs)

Findings
--------
CONFLICT [high] AGENTS.md <-> src/api/AGENTS.md
  Overlapping sources contain directives with opposite polarity.
```

## What it checks today

| Capability | Alpha support |
| --- | --- |
| Discover common coding-agent instruction files | Yes |
| Nested `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` scope | Yes |
| Copilot workspace + `applyTo` instructions | Yes |
| Cursor `globs` + `alwaysApply` rules | Yes |
| Workspace-wide Windsurf and Cline rules | Yes |
| Target-specific effective source list | Yes |
| Obvious positive/negative directive conflicts | Yes |
| Duplicate directives across overlapping sources | Yes |
| JavaScript package-manager choice conflicts | Yes |
| Approximate context/token budget | Yes |
| JSON output | Yes |
| Self-contained interactive HTML report | Yes |
| Executes agent tools, prompts, or scripts | **No** |
| Sends repository content to a remote service | **No** |

## Scope model

AgentContextMap does not pretend every agent has identical semantics.

The alpha release models common repository conventions deterministically:

- hierarchical files apply to their directory subtree;
- workspace instruction files apply across the repository;
- supported pattern-scoped files are matched against the target path;
- nested sources are shown in increasing scope depth so the most specific source is visible last.

Agent products change quickly. If a vendor changes how an instruction format is loaded, open an issue with a documentation link or reproducible example.

## Designed for inspection, not execution

AgentContextMap only reads text files. It does **not** execute commands found in instructions, run agent skills, start MCP servers, call LLM APIs, or require an account.

That makes it useful before giving a repository to an AI coding agent, and safe to run in CI as a read-only inspection step.

## CLI

```text
agentcontext [ROOT] [OPTIONS]

--target <PATH>        Show the effective instruction chain for a target path
--json                 Emit machine-readable JSON
--html <PATH>          Write a self-contained visual HTML report
--fail-on-conflict     Exit with code 2 on a high-severity conflict
-h, --help             Print help
-V, --version          Print version
```

Exit codes:

- `0`: analysis completed and no configured failure condition was hit
- `1`: invalid arguments or I/O failure
- `2`: high-severity conflict found with `--fail-on-conflict`

## Roadmap

Near-term work is focused on correctness rather than adding every agent format possible:

1. broader documented precedence models;
2. richer conflict classes with lower false-positive rates;
3. context-budget breakdown by agent and scope;
4. SARIF / GitHub code-scanning output;
5. signed release binaries and one-command installs;
6. benchmark fixtures from real multi-agent repositories.

## Contributing

Bug reports and small, well-scoped pull requests are welcome. Please include a minimal repository layout when reporting scope or precedence problems.

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

MIT
