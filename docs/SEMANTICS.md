# Agent instruction semantics

AgentContextMap intentionally models only behavior that can be tied to current vendor documentation or a narrowly defined compatibility rule.

Last verified: **2026-08-29**.

The project is a diagnostic approximation, not a replacement for each agent's own runtime introspection. Agent products change quickly, and some activation decisions are model- or UI-dependent.

## Support matrix

| Source | Modeled behavior | Notes |
| --- | --- | --- |
| `AGENTS.md` | Directory-scoped; nested files can become more specific | Modeled for Codex, GitHub Copilot, Cursor, Windsurf and Cline where documented |
| `AGENTS.override.md` | Directory-scoped Codex source | Codex-specific |
| `CLAUDE.md` | Hierarchical Claude Code source | Repository-contained `@` imports are expanded up to depth 5 |
| `GEMINI.md` | Hierarchical Gemini CLI source | Repository-contained `@` imports are expanded up to depth 5 |
| `.github/copilot-instructions.md` | Repository-wide | GitHub Copilot |
| `.github/instructions/**/*.instructions.md` | `applyTo` path-specific | GitHub Copilot |
| `.cursor/rules/**/*.mdc` | Always, glob, model-decided or manual depending on frontmatter | Plain `.md` files in this directory are deliberately ignored |
| `.windsurf/rules/**/*.md` | `always_on`, `glob`, `model_decision`, or `manual` | Current Windsurf workspace rule format |
| `.clinerules/**/*.md` / `*.txt` | Always-on without `paths`; path-specific with `paths` | Current Cline workspace rule format |
| `.cursorrules`, `.windsurfrules` | Cline compatibility sources | Modeled as Cline inputs, not as current Cursor/Windsurf native formats |

## Activation labels

AgentContextMap distinguishes whether a source is definitely present for the requested target or merely capable of becoming relevant:

- `active`: definitely in scope for the requested target under the modeled rule.
- `directory-scoped`: hierarchy is known, but no target path was supplied.
- `path-specific`: glob scope is known, but no target path was supplied.
- `conditional`: the agent/model decides whether to load the full rule.
- `manual`: requires explicit user activation.

High-severity conflicts are only produced when two sources share at least one agent and are definitely active for the requested target. Conditional/manual conflicts are downgraded rather than presented as certain runtime conflicts.

## Imports

Claude Code and Gemini CLI both document `@` imports in their project context files. AgentContextMap follows **repository-contained** relative imports only.

It deliberately does not read imports outside the repository, even when a vendor supports absolute paths or home-directory paths. This prevents a repository scan from unexpectedly reading unrelated or sensitive local files.

Missing repository imports are reported. Import recursion is capped at five levels.

## Known limits

- User-, organization-, enterprise-, and machine-global instruction files outside the scanned repository are not included.
- Vendor configuration can change context filenames or enable/disable sources. AgentContextMap does not currently parse every vendor settings file.
- Model-decided and manual rules cannot be proven active from repository files alone.
- Conflict detection is deterministic and conservative. It is not an LLM semantic judge and will not detect every natural-language contradiction.
- The HTML output is a viewer for the CLI analysis. Browsers cannot silently rescan arbitrary local repositories; rerun the CLI after repository files change.

## Verification sources

### OpenAI Codex

- https://openai.com/index/unrolling-the-codex-agent-loop/
- https://openai.com/index/introducing-codex/

Codex documents repository instruction discovery from the project root toward the working directory, including `AGENTS.md` and `AGENTS.override.md`, with more specific instructions later in the assembled context.

### Claude Code

- https://docs.anthropic.com/en/docs/claude-code/memory

Claude Code documents hierarchical `CLAUDE.md` discovery and recursive `@path` imports up to five hops.

### Gemini CLI

- https://google-gemini.github.io/gemini-cli/docs/cli/gemini-md.html
- https://google-gemini.github.io/gemini-cli/docs/core/memport.html

Gemini CLI documents hierarchical `GEMINI.md` context and `@file` imports.

### GitHub Copilot

- https://docs.github.com/en/copilot/reference/custom-instructions-support
- https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-custom-instructions
- https://docs.github.com/en/copilot/how-tos/copilot-on-github/customize-copilot/add-custom-instructions/add-repository-instructions

GitHub documents `.github/copilot-instructions.md`, path-specific `.github/instructions/**/*.instructions.md`, and `AGENTS.md` support across relevant Copilot surfaces. Exact surface support differs, which is why AgentContextMap does not claim every source applies identically to every Copilot product.

### Cursor

- https://cursor.com/docs/rules

Cursor documents `.cursor/rules` as `.mdc` files and supports nested `AGENTS.md`. Rule activation can be always-on, path-scoped, model-relevant, or manual depending on rule metadata/type.

### Windsurf

- https://docs.windsurf.com/windsurf/cascade/memories
- https://docs.windsurf.com/windsurf/cascade/agents-md

Windsurf documents `.windsurf/rules/*.md` with explicit activation triggers and directory-scoped `AGENTS.md`.

### Cline

- https://docs.cline.bot/customization/cline-rules

Cline documents `.clinerules/` Markdown/text rules, optional `paths` frontmatter, and compatibility with `AGENTS.md`, `.cursorrules`, and `.windsurfrules`.
