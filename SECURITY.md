# Security Policy

AgentContextMap is designed to inspect repository instruction files without executing their contents or sending them to an external service.

## Report a vulnerability privately

If you find a vulnerability that could cause command execution, unintended file writes, path traversal outside the requested repository, exposure of repository contents, or a release-verification bypass, please avoid publishing exploit details in a public issue. Use GitHub's private vulnerability reporting feature when available.

## Current trust boundaries

- Repository instruction text is treated as data, not as commands to execute.
- Repository-contained Claude/Gemini imports are constrained to the scanned repository.
- The composite GitHub Action restricts the requested scan root to `GITHUB_WORKSPACE`.
- The Action downloads a fixed published Linux binary and verifies its expected SHA-256 digest before execution.
- The documented Action workflow requires only read access to checked-out repository contents.
- AgentContextMap has no hosted backend, account system, telemetry service, or remote LLM call.

The project is currently alpha software. Security-sensitive behavior and parser assumptions may change before a stable release, so reports should include the exact AgentContextMap version or commit SHA.
