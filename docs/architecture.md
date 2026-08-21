# Architecture

QuotaBar has three trust boundaries.

1. The SvelteKit frontend renders `GateSnapshotV1` and sends explicit user actions. It does not decide whether to block.
2. The Rust core owns rate-limit reconciliation, calibration, persistence, session classification, overrides, and the gate state machine.
3. The small `quotabar-hook` process accepts Codex hook JSON on stdin, asks the core about that exact session over a mode-`0600` Unix socket, and emits a block response only when the core positively classifies the session as desktop and the gate as exhausted.

## Account-wide data

The app starts the installed Codex helper in app-server mode and calls `account/rateLimits/read`. A 300-minute bucket is authoritative. Otherwise, a 10,080-minute Codex bucket drives the reconstructed five-hour window.

The session observer maintains byte offsets for JSONL files and uses typed structures that omit conversation fields. Existing files begin at EOF so installation cannot replay an account's entire history as new usage.

## Desktop-only gate

The Codex hook is user-level, so every local client may invoke it. The core maps `session_id` to the `originator` in session metadata. Only known desktop originators such as `Codex Desktop` and `codex_work_desktop` are enforceable. CLI, IDE, and unknown values always return `allow`.

The hook configuration is backed up before the first edit. QuotaBar marks its own entry with `QUOTABAR_HOOK=1`, allowing repair and uninstall to remove only that entry.

