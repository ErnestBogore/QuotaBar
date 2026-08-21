# Historical meter methodology

QuotaBar separates official observations from community reconstruction.

## Official inputs

The Codex app-server provides account-wide rate-limit buckets through `account/rateLimits/read` and `account/rateLimits/updated`. Those percentages and reset timestamps are authoritative. QuotaBar does not claim to identify which remote surface caused a server-side change.

If any metered bucket includes a 300-minute window, QuotaBar uses that account-specific window directly even when its identifier is opaque. A literal `codex` bucket wins an otherwise ambiguous tie; otherwise the most-consumed 300-minute bucket is selected conservatively. If no 300-minute bucket exists, QuotaBar reconstructs a local five-hour budget from the exact `codex` 10,080-minute window.

Protocol references:

- [Codex app-server protocol](https://learn.chatgpt.com/docs/app-server)
- [Codex hooks](https://learn.chatgpt.com/docs/hooks)

## Community reconstruction

The classic allowance is modeled as 16 weekly percentage points per fixed five-hour window:

```text
synthetic five-hour used % = weekly points consumed / 16 × 100
```

Therefore each weekly point moves the synthetic meter by 6.25 points. This is not an OpenAI-published billing formula. It is labeled as a community reconstruction everywhere it matters.

The window starts at the first positive usage observation after the prior window expires and ends exactly five hours later in UTC. Server corrections cannot move its displayed usage backward. Weekly resets create a new segment inside the same five-hour window rather than erasing earlier consumption.

## Local interpolation

Rounded official percentages can remain still during active use. QuotaBar estimates relative local movement with:

```text
input + 0.1 × cached input + 6 × output
```

The result is multiplied by a versioned model/speed coefficient. Coefficient table version 1 is deliberately neutral (`1.0`) until public, fixture-backed relative costs are available.

QuotaBar compares local weighted movement with official weekly movement, uses a rolling robust median, and requires five cumulative official points before local interpolation can move or enforce the five-hour meter. It also limits estimator movement to 20% inside a 24-hour anchor period. Unexplained server movement is retained; it is never subtracted as if it did not happen.

## Reproducibility

The sanitized trace in `fixtures/historical/classic-window.json` contains only synthetic timestamps and quota percentages. The integration test verifies its activity anchor, fixed five-hour reset, and 6.25× relationship. No historical fixture may contain conversation text, credentials, usernames, or repository paths.

Public 1.0 should remain a draft until real-device acceptance tests confirm both account-wide meter movement and positive desktop-session classification across supported Codex releases.
