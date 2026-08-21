# Security policy

Please report vulnerabilities privately to the repository maintainers rather than opening a public issue.

QuotaBar's enforcement is intentionally fail-open. A stopped app, unavailable Unix socket, malformed hook input, unknown client, changed Codex log format, or missing usage source must allow the prompt rather than blocking an unrelated workflow.

The gate socket is created with mode `0600`. Transcript paths supplied to the hook are accepted only when their canonical path is a `.jsonl` file below `~/.codex/sessions`.

Release signing and updater private keys must be stored only in the release operator's password manager and GitHub Actions secrets.

