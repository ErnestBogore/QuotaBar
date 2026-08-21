use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

pub const CLASSIC_ALLOWANCE_WEEKLY_POINTS: f64 = 16.0;
pub const FIVE_HOURS_MINUTES: i64 = 300;
pub const WEEK_MINUTES: i64 = 10_080;
pub const OVERRIDE_PHRASE: &str = "Use my one-time 15-minute pass";
pub const MODEL_COEFFICIENT_VERSION: u8 = 1;
pub const METER_STATE_VERSION: u8 = 2;

fn legacy_meter_state_version() -> u8 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub allowance_weekly_points: f64,
    pub launch_at_login: bool,
    pub notifications_enabled: bool,
    pub notification_sound_enabled: bool,
    pub onboarding_completed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            allowance_weekly_points: CLASSIC_ALLOWANCE_WEEKLY_POINTS,
            launch_at_login: true,
            notifications_enabled: true,
            notification_sound_enabled: true,
            onboarding_completed: false,
        }
    }
}

impl Settings {
    pub fn normalized(mut self) -> Self {
        self.allowance_weekly_points = self.allowance_weekly_points.clamp(1.0, 16.0);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitWindow {
    pub used_percent: f64,
    pub window_minutes: i64,
    pub resets_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitSnapshot {
    pub quota_id: String,
    pub observed_at: DateTime<Utc>,
    pub primary: Option<RateLimitWindow>,
    pub secondary: Option<RateLimitWindow>,
}

impl RateLimitSnapshot {
    pub fn official_five_hour(&self) -> Option<&RateLimitWindow> {
        [&self.primary, &self.secondary]
            .into_iter()
            .flatten()
            .find(|window| window.window_minutes == FIVE_HOURS_MINUTES)
    }

    pub fn weekly(&self) -> Option<&RateLimitWindow> {
        [&self.primary, &self.secondary]
            .into_iter()
            .flatten()
            .find(|window| window.window_minutes == WEEK_MINUTES)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UsageSource {
    Desktop,
    Cli,
    Ide,
    LocalUnknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageEvent {
    pub source: UsageSource,
    pub session_id: String,
    pub model: Option<String>,
    pub weighted_tokens: f64,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    Desktop,
    Cli,
    Ide,
    WebCloud,
    Remote,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GateState {
    Open,
    Warning,
    Exhausted,
    Override,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MeterConfidence {
    Official,
    Calibrated,
    Coarse,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GateWindow {
    pub started_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub allowance_weekly_points: f64,
    pub server_weekly_points: f64,
    pub local_weighted_tokens: f64,
    pub displayed_used_percent: f64,
    pub direct_official_used_percent: Option<f64>,
    pub override_requested_at: Option<DateTime<Utc>>,
    pub override_ends_at: Option<DateTime<Utc>>,
    pub override_used: bool,
}

impl GateWindow {
    fn synthetic(started_at: DateTime<Utc>, allowance_weekly_points: f64) -> Self {
        Self {
            started_at,
            ends_at: started_at + Duration::minutes(FIVE_HOURS_MINUTES),
            allowance_weekly_points,
            server_weekly_points: 0.0,
            local_weighted_tokens: 0.0,
            displayed_used_percent: 0.0,
            direct_official_used_percent: None,
            override_requested_at: None,
            override_ends_at: None,
            override_used: false,
        }
    }

    fn official(window: &RateLimitWindow) -> Self {
        Self {
            started_at: window.resets_at - Duration::minutes(window.window_minutes),
            ends_at: window.resets_at,
            allowance_weekly_points: CLASSIC_ALLOWANCE_WEEKLY_POINTS,
            server_weekly_points: 0.0,
            local_weighted_tokens: 0.0,
            displayed_used_percent: window.used_percent.clamp(0.0, 100.0),
            direct_official_used_percent: Some(window.used_percent.clamp(0.0, 100.0)),
            override_requested_at: None,
            override_ends_at: None,
            override_used: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Calibrator {
    samples: Vec<f64>,
    credits_per_weekly_point: Option<f64>,
    observed_weekly_points: f64,
    daily_anchor: Option<f64>,
    daily_anchor_at: Option<DateTime<Utc>>,
}

impl Calibrator {
    pub fn observe(
        &mut self,
        local_weighted_tokens: f64,
        weekly_points: f64,
        observed_at: DateTime<Utc>,
    ) {
        if local_weighted_tokens <= 0.0 || weekly_points <= 0.0 || weekly_points > 10.0 {
            return;
        }
        let candidate = local_weighted_tokens / weekly_points;
        if !candidate.is_finite() || candidate <= 0.0 {
            return;
        }
        self.samples.push(candidate);
        if self.samples.len() > 20 {
            self.samples.remove(0);
        }
        self.observed_weekly_points += weekly_points;
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        let median = sorted[sorted.len() / 2];
        let anchor_expired = self
            .daily_anchor_at
            .map(|anchor_at| observed_at >= anchor_at + Duration::days(1))
            .unwrap_or(true);
        if anchor_expired {
            self.daily_anchor = self.credits_per_weekly_point.or(Some(median));
            self.daily_anchor_at = Some(observed_at);
        }
        let anchor = self.daily_anchor.unwrap_or(median);
        self.credits_per_weekly_point = Some(median.clamp(anchor * 0.8, anchor * 1.2));
    }

    pub fn credits_per_weekly_point(&self) -> Option<f64> {
        self.credits_per_weekly_point
    }

    pub fn is_calibrated(&self) -> bool {
        self.observed_weekly_points >= 5.0 && self.credits_per_weekly_point.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct MeterEngine {
    #[serde(default = "legacy_meter_state_version")]
    pub state_version: u8,
    pub settings: Settings,
    pub window: Option<GateWindow>,
    pub calibrator: Calibrator,
    pub weekly_used_percent: Option<f64>,
    pub weekly_resets_at: Option<DateTime<Utc>>,
    pub last_weekly_used_percent: Option<f64>,
    pub last_weekly_resets_at: Option<DateTime<Utc>>,
    pub app_server_connected: bool,
    pub session_logs_connected: bool,
    pub official_five_hour_available: bool,
    pub official_bucket_format_supported: bool,
    pub available_buckets: Vec<RateLimitSnapshot>,
    pending_local_since_weekly_sample: f64,
    history: Vec<(DateTime<Utc>, f64)>,
}

impl Default for MeterEngine {
    fn default() -> Self {
        Self {
            state_version: METER_STATE_VERSION,
            settings: Settings::default(),
            window: None,
            calibrator: Calibrator::default(),
            weekly_used_percent: None,
            weekly_resets_at: None,
            last_weekly_used_percent: None,
            last_weekly_resets_at: None,
            app_server_connected: false,
            session_logs_connected: false,
            official_five_hour_available: false,
            official_bucket_format_supported: true,
            available_buckets: Vec::new(),
            pending_local_since_weekly_sample: 0.0,
            history: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateSnapshotV1 {
    pub version: u8,
    pub state: GateState,
    pub five_hour_used_percent: f64,
    pub five_hour_remaining_percent: f64,
    pub weekly_used_percent: Option<f64>,
    pub weekly_remaining_percent: Option<f64>,
    pub window_started_at: Option<DateTime<Utc>>,
    pub window_ends_at: Option<DateTime<Utc>>,
    pub weekly_resets_at: Option<DateTime<Utc>>,
    pub allowance_weekly_points: f64,
    pub launch_at_login: bool,
    pub notifications_enabled: bool,
    pub notification_sound_enabled: bool,
    pub onboarding_completed: bool,
    pub available_buckets: Vec<RateLimitSnapshot>,
    pub confidence: MeterConfidence,
    pub source_label: String,
    pub burn_rate_per_hour: Option<f64>,
    pub projected_exhaustion_at: Option<DateTime<Utc>>,
    pub override_requested_at: Option<DateTime<Utc>>,
    pub override_available_at: Option<DateTime<Utc>>,
    pub override_ends_at: Option<DateTime<Utc>>,
    pub override_used: bool,
    pub desktop_hook_installed: bool,
    pub desktop_classification_healthy: bool,
    pub app_server_connected: bool,
    pub session_logs_connected: bool,
    pub status_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OverrideStatus {
    CountdownStarted,
    Waiting,
    Activated,
}

#[derive(Debug, thiserror::Error)]
pub enum OverrideError {
    #[error("The emergency pass is available only after the five-hour allowance is exhausted.")]
    GateOpen,
    #[error("Type the confirmation phrase exactly: {OVERRIDE_PHRASE}")]
    PhraseMismatch,
    #[error("The emergency pass has already been used in this window.")]
    AlreadyUsed,
}

impl MeterEngine {
    pub fn migrate_persisted_state(&mut self) -> bool {
        if self.state_version >= METER_STATE_VERSION {
            return false;
        }
        // Version 1 could double-count one-second reset timestamp jitter and cumulative
        // token counters. Keep the official weekly baseline and user settings, but discard
        // the synthetic window and calibration derived from those samples.
        self.state_version = METER_STATE_VERSION;
        self.window = None;
        self.calibrator = Calibrator::default();
        self.pending_local_since_weekly_sample = 0.0;
        self.history.clear();
        self.last_weekly_used_percent = self.weekly_used_percent;
        self.last_weekly_resets_at = self.weekly_resets_at;
        true
    }

    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings.normalized();
        if let Some(window) = self.window.as_mut() {
            if window.direct_official_used_percent.is_none() {
                window.allowance_weekly_points = self.settings.allowance_weekly_points;
            }
        }
        self.recalculate(Utc::now());
    }

    pub fn set_connectivity(&mut self, app_server: bool, session_logs: bool) {
        self.app_server_connected = app_server;
        self.session_logs_connected = session_logs;
    }

    pub fn set_official_buckets(&mut self, snapshots: &[RateLimitSnapshot]) {
        self.available_buckets = snapshots.to_vec();
        if let Some(observed_at) = snapshots.iter().map(|snapshot| snapshot.observed_at).max() {
            self.tick(observed_at);
        }

        // Bucket identifiers other than `codex` are intentionally opaque. The window
        // duration is the authoritative signal: any server-provided 300-minute bucket
        // takes precedence over reconstruction, regardless of account plan or bucket ID.
        let official = snapshots
            .iter()
            .filter(|snapshot| snapshot.official_five_hour().is_some())
            .max_by(|left, right| {
                let left_is_codex = left.quota_id == "codex";
                let right_is_codex = right.quota_id == "codex";
                left_is_codex.cmp(&right_is_codex).then_with(|| {
                    left.official_five_hour()
                        .map(|window| window.used_percent)
                        .unwrap_or_default()
                        .total_cmp(
                            &right
                                .official_five_hour()
                                .map(|window| window.used_percent)
                                .unwrap_or_default(),
                        )
                })
            });

        if let Some(snapshot) = official {
            self.official_bucket_format_supported = true;
            self.observe_rate_limit(snapshot);
            return;
        }

        // Updated notifications can contain only one changed bucket. Keep an active
        // official window until it expires; the next full read will refresh it.
        if self.official_five_hour_available {
            self.official_bucket_format_supported = true;
            return;
        }

        self.official_bucket_format_supported = snapshots
            .iter()
            .any(|snapshot| snapshot.quota_id == "codex" && snapshot.weekly().is_some());
        for snapshot in snapshots
            .iter()
            .filter(|snapshot| snapshot.quota_id == "codex")
        {
            self.observe_rate_limit(snapshot);
        }
    }

    pub fn observe_rate_limit(&mut self, snapshot: &RateLimitSnapshot) {
        self.tick(snapshot.observed_at);

        if let Some(official) = snapshot.official_five_hour() {
            self.official_five_hour_available = true;
            let replace = self
                .window
                .as_ref()
                .map(|current| {
                    current.ends_at != official.resets_at
                        || current.direct_official_used_percent.is_none()
                })
                .unwrap_or(true);
            if replace {
                self.window = Some(GateWindow::official(official));
            } else if let Some(window) = self.window.as_mut() {
                window.direct_official_used_percent = Some(official.used_percent.clamp(0.0, 100.0));
                window.displayed_used_percent =
                    window.displayed_used_percent.max(official.used_percent);
            }
        }

        if let Some(weekly) = snapshot.weekly() {
            let reset_changed = self
                .last_weekly_resets_at
                .map(|previous| (previous - weekly.resets_at).num_seconds().abs() >= 3_600)
                .unwrap_or(false);
            let delta = match self.last_weekly_used_percent {
                Some(_) if reset_changed => weekly.used_percent.max(0.0),
                Some(previous) => (weekly.used_percent - previous).max(0.0),
                None => 0.0,
            };

            if !self.official_five_hour_available && delta > 0.0 {
                self.ensure_window(snapshot.observed_at);
                if let Some(window) = self.window.as_mut() {
                    window.server_weekly_points += delta;
                }
                self.calibrator.observe(
                    self.pending_local_since_weekly_sample,
                    delta,
                    snapshot.observed_at,
                );
                self.pending_local_since_weekly_sample = 0.0;
            }

            self.weekly_used_percent = Some(weekly.used_percent.clamp(0.0, 100.0));
            self.weekly_resets_at = Some(weekly.resets_at);
            self.last_weekly_used_percent = Some(weekly.used_percent);
            self.last_weekly_resets_at = Some(weekly.resets_at);
        }

        self.recalculate(snapshot.observed_at);
    }

    pub fn observe_usage(&mut self, event: &UsageEvent) {
        if event.weighted_tokens <= 0.0 || !event.weighted_tokens.is_finite() {
            return;
        }
        self.tick(event.observed_at);
        if !self.official_five_hour_available {
            self.ensure_window(event.observed_at);
        }
        if let Some(window) = self.window.as_mut() {
            if window.direct_official_used_percent.is_none() {
                window.local_weighted_tokens += event.weighted_tokens;
            }
        }
        self.pending_local_since_weekly_sample += event.weighted_tokens;
        self.recalculate(event.observed_at);
    }

    pub fn tick(&mut self, now: DateTime<Utc>) {
        let expired = self
            .window
            .as_ref()
            .map(|window| now >= window.ends_at)
            .unwrap_or(false);
        if expired {
            self.window = None;
            self.history.clear();
            self.official_five_hour_available = false;
        }
        if let Some(window) = self.window.as_mut() {
            if window
                .override_ends_at
                .map(|end| now >= end)
                .unwrap_or(false)
            {
                window.override_ends_at = None;
            }
        }
    }

    pub fn request_override(
        &mut self,
        phrase: &str,
        now: DateTime<Utc>,
    ) -> Result<OverrideStatus, OverrideError> {
        self.tick(now);
        let exhausted = self
            .window
            .as_ref()
            .map(|window| window.displayed_used_percent >= 100.0)
            .unwrap_or(false);
        if !exhausted {
            return Err(OverrideError::GateOpen);
        }
        if phrase != OVERRIDE_PHRASE {
            return Err(OverrideError::PhraseMismatch);
        }
        let window = self.window.as_mut().expect("exhausted window exists");
        if window.override_used {
            return Err(OverrideError::AlreadyUsed);
        }
        match window.override_requested_at {
            None => {
                window.override_requested_at = Some(now);
                Ok(OverrideStatus::CountdownStarted)
            }
            Some(requested) if now < requested + Duration::seconds(60) => {
                Ok(OverrideStatus::Waiting)
            }
            Some(_) => {
                window.override_used = true;
                window.override_ends_at = Some((now + Duration::minutes(15)).min(window.ends_at));
                Ok(OverrideStatus::Activated)
            }
        }
    }

    pub fn snapshot(
        &mut self,
        now: DateTime<Utc>,
        hook_installed: bool,
        classification_healthy: bool,
    ) -> GateSnapshotV1 {
        self.tick(now);
        self.recalculate(now);
        let used = self
            .window
            .as_ref()
            .map(|window| window.displayed_used_percent)
            .unwrap_or(0.0);
        let override_active = self
            .window
            .as_ref()
            .and_then(|window| window.override_ends_at)
            .map(|end| now < end)
            .unwrap_or(false);
        let data_available = self.app_server_connected || self.session_logs_connected;
        let state = if !data_available || !self.official_bucket_format_supported {
            GateState::Unavailable
        } else if override_active {
            GateState::Override
        } else if used >= 100.0 {
            GateState::Exhausted
        } else if used >= 50.0 {
            GateState::Warning
        } else {
            GateState::Open
        };
        let confidence = if self.official_five_hour_available
            && (self.app_server_connected || self.session_logs_connected)
        {
            MeterConfidence::Official
        } else if self.calibrator.is_calibrated() && self.session_logs_connected {
            MeterConfidence::Calibrated
        } else if self.app_server_connected {
            MeterConfidence::Coarse
        } else {
            MeterConfidence::Offline
        };
        let source_label = if !self.official_bucket_format_supported {
            "Unsupported server buckets"
        } else {
            match confidence {
                MeterConfidence::Official => "Official 5-hour bucket",
                MeterConfidence::Calibrated => "Weekly meter + calibrated local usage",
                MeterConfidence::Coarse => "Official weekly meter",
                MeterConfidence::Offline => "Local estimate",
            }
        }
        .to_string();
        let burn_rate = self.burn_rate_per_hour(now);
        let projected = burn_rate.and_then(|rate| {
            if rate > 0.0 && used < 100.0 {
                Some(now + Duration::seconds((((100.0 - used) / rate) * 3600.0) as i64))
            } else {
                None
            }
        });
        let status_message = match state {
            GateState::Open => "Five-hour gate is open",
            GateState::Warning => "Approaching the five-hour limit",
            GateState::Exhausted if !hook_installed => {
                "Limit reached · Mac app gate is not installed"
            }
            GateState::Exhausted if !classification_healthy => {
                "Limit reached · Desktop gate needs repair"
            }
            GateState::Exhausted => "Limit reached · New Mac app prompts are blocked",
            GateState::Override => "Emergency pass is active",
            GateState::Unavailable if !self.official_bucket_format_supported => {
                "Unknown server buckets · Gate is fail-open"
            }
            GateState::Unavailable => "Usage sources unavailable · Gate is fail-open",
        }
        .to_string();

        GateSnapshotV1 {
            version: 1,
            state,
            five_hour_used_percent: used,
            five_hour_remaining_percent: (100.0 - used).clamp(0.0, 100.0),
            weekly_used_percent: self.weekly_used_percent,
            weekly_remaining_percent: self
                .weekly_used_percent
                .map(|value| (100.0 - value).clamp(0.0, 100.0)),
            window_started_at: self.window.as_ref().map(|window| window.started_at),
            window_ends_at: self.window.as_ref().map(|window| window.ends_at),
            weekly_resets_at: self.weekly_resets_at,
            allowance_weekly_points: self.settings.allowance_weekly_points,
            launch_at_login: self.settings.launch_at_login,
            notifications_enabled: self.settings.notifications_enabled,
            notification_sound_enabled: self.settings.notification_sound_enabled,
            onboarding_completed: self.settings.onboarding_completed,
            available_buckets: self.available_buckets.clone(),
            confidence,
            source_label,
            burn_rate_per_hour: burn_rate,
            projected_exhaustion_at: projected,
            override_requested_at: self
                .window
                .as_ref()
                .and_then(|window| window.override_requested_at),
            override_available_at: self.window.as_ref().and_then(|window| {
                window
                    .override_requested_at
                    .map(|time| time + Duration::seconds(60))
            }),
            override_ends_at: self
                .window
                .as_ref()
                .and_then(|window| window.override_ends_at),
            override_used: self
                .window
                .as_ref()
                .map(|window| window.override_used)
                .unwrap_or(false),
            desktop_hook_installed: hook_installed,
            desktop_classification_healthy: classification_healthy,
            app_server_connected: self.app_server_connected,
            session_logs_connected: self.session_logs_connected,
            status_message,
        }
    }

    pub fn reset_history(&mut self) {
        let settings = self.settings.clone();
        *self = Self::default();
        self.settings = settings;
    }

    fn ensure_window(&mut self, at: DateTime<Utc>) {
        if self.window.is_none() {
            self.window = Some(GateWindow::synthetic(
                at,
                self.settings.allowance_weekly_points,
            ));
            self.history.clear();
        }
    }

    fn recalculate(&mut self, now: DateTime<Utc>) {
        if let Some(window) = self.window.as_mut() {
            let server = window.direct_official_used_percent.unwrap_or_else(|| {
                (window.server_weekly_points / window.allowance_weekly_points) * 100.0
            });
            // A single rounded weekly point is far too noisy to size a five-hour
            // window. Local interpolation becomes authoritative only after the
            // documented five-point calibration threshold has been reached.
            let local = if self.calibrator.is_calibrated() {
                self.calibrator
                    .credits_per_weekly_point()
                    .map(|credits| {
                        (window.local_weighted_tokens / (credits * window.allowance_weekly_points))
                            * 100.0
                    })
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            window.displayed_used_percent = window
                .displayed_used_percent
                .max(server)
                .max(local)
                .clamp(0.0, 10_000.0);
            if self
                .history
                .last()
                .map(|(_, value)| (*value - window.displayed_used_percent).abs() > 0.01)
                .unwrap_or(true)
            {
                self.history.push((now, window.displayed_used_percent));
                if self.history.len() > 256 {
                    self.history.remove(0);
                }
            }
        }
    }

    fn burn_rate_per_hour(&self, now: DateTime<Utc>) -> Option<f64> {
        let cutoff = now - Duration::hours(1);
        let first = self
            .history
            .iter()
            .find(|(time, _)| *time >= cutoff)
            .or_else(|| self.history.first())?;
        let last = self.history.last()?;
        let hours = (last.0 - first.0).num_seconds() as f64 / 3600.0;
        if hours <= 0.01 || last.1 <= first.1 {
            None
        } else {
            Some((last.1 - first.1) / hours)
        }
    }
}

pub fn classify_originator(originator: Option<&str>) -> ClientKind {
    match originator.map(str::to_ascii_lowercase).as_deref() {
        Some("codex desktop") | Some("codex_work_desktop") => ClientKind::Desktop,
        Some(value)
            if value.contains("vscode") || value.contains("jetbrains") || value.contains("ide") =>
        {
            ClientKind::Ide
        }
        Some(value) if value.contains("cli") || value == "codex" => ClientKind::Cli,
        _ => ClientKind::Unknown,
    }
}

pub fn weighted_tokens(input: u64, cached_input: u64, output: u64, model_multiplier: f64) -> f64 {
    (input as f64 + cached_input as f64 * 0.1 + output as f64 * 6.0) * model_multiplier.max(0.0)
}

pub fn model_speed_coefficient(_model: Option<&str>) -> f64 {
    // Version 1 is deliberately neutral until fixture-backed relative costs are published.
    // Keeping the function versioned prevents a future model from silently inheriting a
    // guessed coefficient in the blocking path.
    1.0
}

pub fn should_block_client(
    client: &ClientKind,
    state: &GateState,
    classification_healthy: bool,
) -> bool {
    *client == ClientKind::Desktop && *state == GateState::Exhausted && classification_healthy
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 8, 21, hour, 0, 0).unwrap()
    }

    fn weekly(used: f64, at_time: DateTime<Utc>) -> RateLimitSnapshot {
        RateLimitSnapshot {
            quota_id: "codex".into(),
            observed_at: at_time,
            primary: Some(RateLimitWindow {
                used_percent: used,
                window_minutes: WEEK_MINUTES,
                resets_at: at(0) + Duration::days(7),
            }),
            secondary: None,
        }
    }

    #[test]
    fn classic_ratio_is_six_point_two_five() {
        assert_eq!(100.0 / CLASSIC_ALLOWANCE_WEEKLY_POINTS, 6.25);
    }

    #[test]
    fn weekly_delta_starts_fixed_window_and_never_moves_backwards() {
        let mut engine = MeterEngine::default();
        engine.set_connectivity(true, true);
        engine.observe_rate_limit(&weekly(20.0, at(1)));
        assert!(engine.window.is_none());
        engine.observe_rate_limit(&weekly(21.0, at(2)));
        let first = engine.snapshot(at(2), true, true);
        assert_eq!(first.five_hour_used_percent, 6.25);
        assert_eq!(first.window_ends_at, Some(at(2) + Duration::hours(5)));
        engine.observe_rate_limit(&weekly(20.5, at(3)));
        assert_eq!(
            engine.snapshot(at(3), true, true).five_hour_used_percent,
            6.25
        );
    }

    #[test]
    fn one_second_reset_jitter_is_not_a_weekly_reset() {
        let mut engine = MeterEngine::default();
        engine.set_connectivity(true, true);
        let first = weekly(14.0, at(1));
        engine.observe_rate_limit(&first);
        let mut jittered = weekly(14.0, at(2));
        jittered.primary.as_mut().unwrap().resets_at += Duration::seconds(1);
        engine.observe_rate_limit(&jittered);
        assert!(engine.window.is_none());

        jittered.observed_at = at(3);
        jittered.primary.as_mut().unwrap().used_percent = 15.0;
        engine.observe_rate_limit(&jittered);
        assert_eq!(
            engine.snapshot(at(3), true, true).five_hour_used_percent,
            6.25
        );
    }

    #[test]
    fn version_one_state_discards_its_synthetic_window() {
        let mut engine = MeterEngine {
            state_version: 1,
            weekly_used_percent: Some(15.0),
            weekly_resets_at: Some(at(0) + Duration::days(7)),
            window: Some(GateWindow::synthetic(at(1), 16.0)),
            ..MeterEngine::default()
        };
        engine.window.as_mut().unwrap().displayed_used_percent = 725.0;
        assert!(engine.migrate_persisted_state());
        assert_eq!(engine.state_version, METER_STATE_VERSION);
        assert!(engine.window.is_none());
        assert_eq!(engine.last_weekly_used_percent, Some(15.0));
    }

    #[test]
    fn official_five_hour_wins() {
        let mut engine = MeterEngine::default();
        engine.set_connectivity(true, true);
        engine.observe_rate_limit(&RateLimitSnapshot {
            quota_id: "codex".into(),
            observed_at: at(1),
            primary: Some(RateLimitWindow {
                used_percent: 43.0,
                window_minutes: FIVE_HOURS_MINUTES,
                resets_at: at(5),
            }),
            secondary: Some(RateLimitWindow {
                used_percent: 70.0,
                window_minutes: WEEK_MINUTES,
                resets_at: at(0) + Duration::days(7),
            }),
        });
        let snapshot = engine.snapshot(at(1), true, true);
        assert_eq!(snapshot.five_hour_used_percent, 43.0);
        assert_eq!(snapshot.confidence, MeterConfidence::Official);
    }

    #[test]
    fn official_five_hour_in_an_opaque_plan_bucket_wins() {
        let mut engine = MeterEngine::default();
        engine.set_connectivity(true, true);
        engine.set_official_buckets(&[
            weekly(16.0, at(1)),
            RateLimitSnapshot {
                quota_id: "codex_bengalfox".into(),
                observed_at: at(1),
                primary: Some(RateLimitWindow {
                    used_percent: 3.0,
                    window_minutes: FIVE_HOURS_MINUTES,
                    resets_at: at(5),
                }),
                secondary: Some(RateLimitWindow {
                    used_percent: 4.0,
                    window_minutes: WEEK_MINUTES,
                    resets_at: at(0) + Duration::days(7),
                }),
            },
        ]);
        let snapshot = engine.snapshot(at(1), true, true);
        assert_eq!(snapshot.five_hour_used_percent, 3.0);
        assert_eq!(snapshot.weekly_used_percent, Some(4.0));
        assert_eq!(snapshot.confidence, MeterConfidence::Official);
    }

    #[test]
    fn local_interpolation_waits_for_five_weekly_points() {
        let mut engine = MeterEngine::default();
        engine.set_connectivity(true, true);
        engine.observe_rate_limit(&weekly(10.0, at(0)));
        engine.observe_usage(&UsageEvent {
            source: UsageSource::Desktop,
            session_id: "desktop-1".into(),
            model: Some("gpt-5.6-sol".into()),
            weighted_tokens: 400_000.0,
            observed_at: at(1),
        });
        engine.observe_rate_limit(&weekly(11.0, at(2)));
        engine.observe_usage(&UsageEvent {
            source: UsageSource::Desktop,
            session_id: "desktop-1".into(),
            model: Some("gpt-5.6-sol".into()),
            weighted_tokens: 4_000_000.0,
            observed_at: at(3),
        });
        let snapshot = engine.snapshot(at(3), true, true);
        assert_eq!(snapshot.five_hour_used_percent, 6.25);
        assert_eq!(snapshot.confidence, MeterConfidence::Coarse);
    }

    #[test]
    fn unknown_clients_fail_open_by_classification() {
        assert_eq!(classify_originator(None), ClientKind::Unknown);
        assert_eq!(
            classify_originator(Some("new_surface")),
            ClientKind::Unknown
        );
        assert_eq!(
            classify_originator(Some("Codex Desktop")),
            ClientKind::Desktop
        );
    }

    #[test]
    fn emergency_pass_requires_phrase_wait_and_is_single_use() {
        let mut engine = MeterEngine::default();
        engine.set_connectivity(true, true);
        engine.observe_rate_limit(&weekly(1.0, at(0)));
        engine.observe_rate_limit(&weekly(17.0, at(1)));
        assert!(matches!(
            engine.request_override("wrong", at(1)),
            Err(OverrideError::PhraseMismatch)
        ));
        assert_eq!(
            engine.request_override(OVERRIDE_PHRASE, at(1)).unwrap(),
            OverrideStatus::CountdownStarted
        );
        assert_eq!(
            engine
                .request_override(OVERRIDE_PHRASE, at(1) + Duration::seconds(30))
                .unwrap(),
            OverrideStatus::Waiting
        );
        assert_eq!(
            engine
                .request_override(OVERRIDE_PHRASE, at(1) + Duration::seconds(61))
                .unwrap(),
            OverrideStatus::Activated
        );
        assert!(matches!(
            engine.request_override(OVERRIDE_PHRASE, at(1) + Duration::seconds(62)),
            Err(OverrideError::AlreadyUsed)
        ));
    }

    #[test]
    fn expiration_survives_tick_and_resets_window() {
        let mut engine = MeterEngine::default();
        engine.set_connectivity(true, true);
        engine.observe_rate_limit(&weekly(0.0, at(0)));
        engine.observe_rate_limit(&weekly(1.0, at(1)));
        engine.tick(at(6));
        assert!(engine.window.is_none());
    }

    #[test]
    fn token_weighting_matches_documented_reconstruction() {
        assert_eq!(weighted_tokens(100, 20, 10, 1.0), 162.0);
        assert_eq!(MODEL_COEFFICIENT_VERSION, 1);
        assert_eq!(model_speed_coefficient(Some("unknown-future-model")), 1.0);
    }

    #[test]
    fn estimator_cannot_move_more_than_twenty_percent_inside_a_day() {
        let mut calibrator = Calibrator::default();
        calibrator.observe(100.0, 1.0, at(1));
        calibrator.observe(1_000.0, 1.0, at(2));
        assert_eq!(calibrator.credits_per_weekly_point(), Some(120.0));
    }

    #[test]
    fn unsupported_official_buckets_disable_enforcement_and_remain_visible() {
        let mut engine = MeterEngine::default();
        engine.set_connectivity(true, true);
        engine.set_official_buckets(&[RateLimitSnapshot {
            quota_id: "new-codex-shape".into(),
            observed_at: at(1),
            primary: Some(RateLimitWindow {
                used_percent: 50.0,
                window_minutes: 1_440,
                resets_at: at(1) + Duration::days(1),
            }),
            secondary: None,
        }]);
        let snapshot = engine.snapshot(at(1), true, true);
        assert_eq!(snapshot.state, GateState::Unavailable);
        assert_eq!(snapshot.available_buckets[0].quota_id, "new-codex-shape");
    }

    #[test]
    fn only_a_positively_classified_desktop_prompt_is_blocked() {
        assert!(should_block_client(
            &ClientKind::Desktop,
            &GateState::Exhausted,
            true
        ));
        for client in [ClientKind::Cli, ClientKind::Ide, ClientKind::Unknown] {
            assert!(!should_block_client(&client, &GateState::Exhausted, true));
        }
    }
}
