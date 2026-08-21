import { invoke } from '@tauri-apps/api/core';
import type { GateSnapshotV1, OverrideResponse, Settings, UpdateCheckResponse } from './types';

const mock: GateSnapshotV1 = {
  version: 1,
  state: 'open',
  fiveHourUsedPercent: 28,
  fiveHourRemainingPercent: 72,
  weeklyUsedPercent: 15,
  weeklyRemainingPercent: 85,
  windowStartedAt: '2026-08-21T14:00:00Z',
  windowEndsAt: '2026-08-21T19:00:00Z',
  weeklyResetsAt: '2026-08-27T12:48:57Z',
  allowanceWeeklyPoints: 16,
  launchAtLogin: true,
  notificationsEnabled: true,
  notificationSoundEnabled: true,
  onboardingCompleted: true,
  availableBuckets: [],
  confidence: 'coarse',
  sourceLabel: 'Official weekly meter',
  burnRatePerHour: 7.2,
  projectedExhaustionAt: null,
  overrideRequestedAt: null,
  overrideAvailableAt: null,
  overrideEndsAt: null,
  overrideUsed: false,
  desktopHookInstalled: true,
  desktopClassificationHealthy: true,
  appServerConnected: true,
  sessionLogsConnected: true,
  statusMessage: 'Five-hour gate is open'
};

function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

export async function getState(): Promise<GateSnapshotV1> {
  return inTauri() ? invoke<GateSnapshotV1>('get_state') : mock;
}

export async function updateSettings(settings: Settings): Promise<GateSnapshotV1> {
  return invoke<GateSnapshotV1>('update_settings', { settings });
}

export async function completeOnboarding(): Promise<GateSnapshotV1> {
  return invoke<GateSnapshotV1>('complete_onboarding');
}

export async function sendTestNotification(): Promise<void> {
  await invoke('send_test_notification');
}

export async function openApplicationsFolder(): Promise<void> {
  await invoke('open_applications_folder');
}

export async function isInApplications(): Promise<boolean> {
  return invoke<boolean>('is_in_applications');
}

export async function requestOverride(phrase: string): Promise<OverrideResponse> {
  return invoke<OverrideResponse>('request_override', { phrase });
}

export async function installDesktopHook(): Promise<GateSnapshotV1> {
  return invoke<GateSnapshotV1>('install_desktop_hook');
}

export async function repairDesktopHook(): Promise<GateSnapshotV1> {
  return invoke<GateSnapshotV1>('repair_desktop_hook');
}

export async function removeDesktopHook(): Promise<GateSnapshotV1> {
  return invoke<GateSnapshotV1>('remove_desktop_hook');
}

export async function deleteHistory(): Promise<GateSnapshotV1> {
  return invoke<GateSnapshotV1>('delete_history');
}

export async function hideMainWindow(): Promise<void> {
  if (inTauri()) await invoke('hide_main_window');
}

export async function checkForUpdates(): Promise<UpdateCheckResponse> {
  return invoke<UpdateCheckResponse>('check_for_updates');
}

export async function installUpdate(): Promise<void> {
  await invoke('install_update');
}
