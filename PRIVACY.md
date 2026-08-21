# Privacy

QuotaBar is local-first and has no analytics.

It communicates with the official Codex helper on the same Mac to retrieve account rate limits. It also checks GitHub Releases when the signed updater is configured. No other outbound service is used by QuotaBar.

QuotaBar does not read or copy `~/.codex/auth.json`. It does not retain prompts, model responses, tool calls, command output, repository paths, or file contents. The narrow session decoder stores only metadata required for quota accounting and desktop-client classification.

Detailed rate and weighted-token samples are retained for 30 days. Daily numeric aggregates are retained until the user selects **Delete history**. All data lives under `~/Library/Application Support/QuotaBar`.

