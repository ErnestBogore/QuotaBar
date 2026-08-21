use chrono::{DateTime, Utc};
use quota_core::{should_block_client, ClientKind, GateState, MeterEngine, RateLimitSnapshot};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoricalFixture {
    snapshots: Vec<RateLimitSnapshot>,
    expected_window_start: DateTime<Utc>,
    expected_window_end: DateTime<Utc>,
    expected_used_percent: f64,
}

#[test]
fn historical_trace_reconstructs_the_classic_window() {
    let fixture: HistoricalFixture = serde_json::from_str(include_str!(
        "../../../fixtures/historical/classic-window.json"
    ))
    .expect("fixture must remain valid");
    let mut engine = MeterEngine::default();
    engine.set_connectivity(true, true);
    for snapshot in &fixture.snapshots {
        engine.observe_rate_limit(snapshot);
    }

    let snapshot = engine.snapshot(fixture.snapshots.last().unwrap().observed_at, true, true);
    assert_eq!(
        snapshot.window_started_at,
        Some(fixture.expected_window_start)
    );
    assert_eq!(snapshot.window_ends_at, Some(fixture.expected_window_end));
    assert_eq!(
        snapshot.five_hour_used_percent,
        fixture.expected_used_percent
    );

    // Account-wide movement advances the gate, but a non-desktop caller is never blocked.
    assert!(!should_block_client(
        &ClientKind::Cli,
        &GateState::Exhausted,
        true
    ));
}
