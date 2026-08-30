# GitHub App integration boundary

AgentContextMap is currently designed as a local CLI and GitHub Action. The default product promise is that repository content is analyzed inside the user's environment or GitHub Actions runner and is not sent to an AgentContextMap-hosted backend.

A future GitHub App can improve installation and pull-request ergonomics, but it must not silently weaken that boundary.

## What exists now

- GitHub Marketplace Action
- pull-request and push workflow support
- GitHub Actions job summaries
- SARIF 2.1.0 output
- explicit GitHub Code Scanning upload support
- optional CI failure on high-confidence active conflicts
- no AgentContextMap-hosted service

## Preferred GitHub-native path

The Marketplace Action remains the recommended integration because the scan runs in the repository's GitHub Actions runner. The Action itself needs only `contents: read` for its normal scan. Repositories that choose Code Scanning explicitly add `security-events: write` for the separate SARIF upload step.

## Future GitHub App

A GitHub App is useful only if it provides a clear benefit over the Action, such as install-once checks across many repositories without requiring each repository to maintain workflow YAML.

That App would require a hosted webhook/backend component. A hosted scanner would necessarily receive some repository content, so it must be a separately documented, explicit opt-in product mode rather than a silent replacement for the current local-first behavior.

### Minimum permissions

The first prototype should request no more than:

- Metadata: read
- Contents: read
- Pull requests: read
- Checks: write

`Pull requests: write` should not be requested merely to post comments. Check Runs are the preferred first presentation surface because they avoid an unnecessary write permission to pull-request conversations.

### Webhook events

Start with only:

- `pull_request` — `opened`, `synchronize`, `reopened`

Additional events should be added only when a concrete feature requires them.

### Processing rules

A hosted prototype must:

1. validate every GitHub webhook signature before processing;
2. use short-lived installation access tokens;
3. fetch only repository data needed for the scan;
4. never execute repository instructions, scripts, tools, prompts, or MCP servers;
5. never send repository content to an LLM by default;
6. avoid persistent storage of repository file contents unless a future feature explicitly requires it and documents retention;
7. publish results through GitHub Checks first;
8. clearly distinguish deterministic findings from conditional/model-decided instruction states.

## Checks API result model

A future App Check Run should map AgentContextMap results approximately as follows:

- no findings: successful conclusion;
- review findings without an enforced gate: neutral conclusion;
- enforced high-confidence active conflict: failure conclusion;
- scanner/configuration error: action-required or failure conclusion with the error clearly separated from repository findings.

SARIF remains the preferred route for repositories that want findings in GitHub Code Scanning.

## Decision gate before implementation

Do not ship a hosted GitHub App until all of these are true:

- the Marketplace Action has real users who ask for install-once organization/repository coverage;
- the remote-processing privacy change is documented before installation;
- webhook signature verification and installation-token handling have automated tests;
- a deployment environment and support/incident path exist;
- the App can demonstrate a meaningful usability advantage over the Action.

Until then, improving the Marketplace Action is the safer and more useful GitHub-native integration path.
