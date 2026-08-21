export type GateState = 'open' | 'warning' | 'exhausted' | 'override' | 'unavailable';
export type MeterConfidence = 'official' | 'calibrated' | 'coarse' | 'offline';

export interface RateLimitWindow {
  usedPercent: number;
  windowMinutes: number;
  resetsAt: string;
}

export interface RateLimitSnapshot {
  quotaId: string;
  observedAt: string;
  primary: RateLimitWindow | null;
  secondary: RateLimitWindow | null;
}

export interface GateSnapshotV1 {
  version: 1;
  state: GateState;
  fiveHourUsedPercent: number;
  fiveHourRemainingPercent: number;
  weeklyUsedPercent: number | null;
  weeklyRemainingPercent: number | null;
  windowStartedAt: string | null;
  windowEndsAt: string | null;
  weeklyResetsAt: string | null;
  allowanceWeeklyPoints: number;
  launchAtLogin: boolean;
  notificationsEnabled: boolean;
  notificationSoundEnabled: boolean;
  onboardingCompleted: boolean;
  availableBuckets: RateLimitSnapshot[];
  confidence: MeterConfidence;
  sourceLabel: string;
  burnRatePerHour: number | null;
  projectedExhaustionAt: string | null;
  overrideRequestedAt: string | null;
  overrideAvailableAt: string | null;
  overrideEndsAt: string | null;
  overrideUsed: boolean;
  desktopHookInstalled: boolean;
  desktopClassificationHealthy: boolean;
  appServerConnected: boolean;
  sessionLogsConnected: boolean;
  statusMessage: string;
}

export interface Settings {
  allowanceWeeklyPoints: number;
  launchAtLogin: boolean;
  notificationsEnabled: boolean;
  notificationSoundEnabled: boolean;
  onboardingCompleted: boolean;
}

export interface OverrideResponse {
  status: 'countdown_started' | 'waiting' | 'activated';
  snapshot: GateSnapshotV1;
}

export interface UpdateCheckResponse {
  currentVersion: string;
  availableVersion: string | null;
  notes: string | null;
}
