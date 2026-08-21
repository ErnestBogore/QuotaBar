# QuotaBar

Website: [quotabar.fyi](https://quotabar.fyi) · Source: [github.com/ErnestBogore/QuotaBar](https://github.com/ErnestBogore/QuotaBar)

QuotaBar is a local macOS menu-bar app that uses OpenAI's account-specific five-hour Codex meter when available and otherwise reconstructs one from the account-wide weekly quota. It measures every surface reported by the Codex account meter, but only gates new prompts submitted from the Codex Mac app.

QuotaBar is an independent community project. It is not affiliated with or endorsed by OpenAI, and its synthetic meter is an estimate rather than an official quota.

## Install the preview (no developer tools needed)

1. Download the latest `QuotaBar-…-macos-arm64.zip` file and double-click it.
2. Drag **QuotaBar** into the **Applications** folder. Choose **Replace** if an older preview is already installed.
3. Open Applications, Control-click **QuotaBar**, and choose **Open**. This extra step is required for the current community-signed preview.
4. Click the new **Q** icon in the menu bar. QuotaBar starts measuring automatically.
5. Open the gear button only if you want to turn on or fix **Pause new prompts at 0%**.

The large number is the five-hour budget remaining. The smaller bar is the official weekly budget. Every Codex client contributes to both meters, but the optional gate pauses only new prompts from the Codex Mac app. QuotaBar deliberately allows CLI, IDE, browser, cloud, and unknown clients.

QuotaBar behaves like a menu-bar popover: click another app, press Escape, or use the × button to hide it. It continues measuring in the menu bar. After installing a release that supports updates, use **Settings → Updates → Check** to download and install future signed releases without replacing the app manually.

## What it does

- Reads official account rate-limit snapshots through `codex app-server`.
- Interpolates rounded server percentages from local token counters without retaining conversation content.
- Uses a fixed, activity-anchored five-hour window and a classic 16-weekly-point allowance.
- Warns at 50%, 75%, 90%, and 100%.
- Optionally installs a fail-open `UserPromptSubmit` hook that blocks only positively identified Mac-app sessions.
- Leaves CLI, IDE, browser, cloud, mobile, and other machines unblocked.
- Stores all state locally in `~/Library/Application Support/QuotaBar`.

## Development

Requirements:

- macOS 14 or newer
- Rust stable
- Node.js 22 and pnpm 9+
- The ChatGPT/Codex Mac app or a signed-in Codex CLI

```sh
pnpm install
pnpm check
cargo test --workspace
pnpm tauri:dev
```

The integration suite consumes only the sanitized traces in `fixtures/`; no fixture may contain conversation or repository content.

The desktop gate is never installed automatically. Open Settings in QuotaBar and choose **Install Mac app gate** after reviewing the scope.

## Metering model

If OpenAI supplies a 300-minute metered bucket, QuotaBar uses its account-specific percentage directly even when the bucket has an opaque plan identifier. Otherwise it reconstructs the window from the 10,080-minute `codex` weekly bucket:

```text
five-hour usage = weekly points consumed in the window / allowance × 100
```

The default allowance is 16 weekly percentage points, equivalent to a 6.25× relationship between weekly movement and the synthetic meter. This coefficient was reconstructed from historical client telemetry and is intentionally labeled as a community estimate.

Local interpolation uses relative weighted usage:

```text
input + 0.1 × cached input + 6 × output
```

QuotaBar learns the weighted units represented by one official weekly point. Local interpolation does not move or enforce the meter until at least five official percentage points have been observed. Until then, the weekly fallback moves only when OpenAI's percentage moves.

## Privacy and failure behavior

QuotaBar never opens Codex authentication files and never uploads application data. The session-log decoder retains only event type, timestamps, session/origin identifiers, model identifiers, rate-limit values, and token counters. It ignores prompts, responses, tool output, and repository data.

If both official and local meters become unavailable, enforcement fails open. Unknown clients also fail open; QuotaBar requires positive desktop-origin evidence before rejecting a prompt.

See [PRIVACY.md](PRIVACY.md), [SECURITY.md](SECURITY.md), [docs/architecture.md](docs/architecture.md), and [docs/methodology.md](docs/methodology.md) for the full trust and measurement model.

## Release signing

Production releases require a Developer ID Application certificate, Apple notarization credentials, and the Tauri updater private key. The updater public key is committed; its matching private key is generated into ignored `work/quotabar-updater.key` for initial project setup and must be moved into the `TAURI_SIGNING_PRIVATE_KEY` GitHub secret before publishing. Never commit it.

The manual Release workflow requires an explicit acceptance-test confirmation. It builds a universal binary, signs the hook as `app.quotabar.hook`, re-signs the outer app as `app.quotabar.mac`, notarizes and staples the artifacts, signs the updater archive, and creates a draft GitHub Release under `ErnestBogore/QuotaBar`.

## License

MIT
