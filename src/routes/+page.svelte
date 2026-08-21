<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window';
  import {
    checkForUpdates, completeOnboarding, deleteHistory, getState, hideMainWindow,
    installDesktopHook, installUpdate, isInApplications, openApplicationsFolder,
    removeDesktopHook, repairDesktopHook, requestOverride, sendTestNotification, updateSettings
  } from '$lib/api';
  import type { GateSnapshotV1 } from '$lib/types';

  let snapshot: GateSnapshotV1 | null = null;
  let allowance = 16;
  let launchAtLogin = true;
  let notificationsEnabled = true;
  let notificationSoundEnabled = true;
  let phrase = '';
  let busy = '';
  let error = '';
  let settingsOpen = false;
  let detailsOpen = false;
  let passOpen = false;
  let onboardingOpen = false;
  let releaseOpen = false;
  let installedInApplications = false;
  let alertsTested = false;
  let availableUpdate: string | null = null;
  let currentVersion = '';
  let releaseNotes: string | null = null;
  let updateMessage = 'Check and install new versions without downloading a ZIP';
  let now = Date.now();

  $: if (typeof window !== 'undefined') {
    const hasStatus = snapshot?.state === 'warning' || snapshot?.state === 'exhausted' || snapshot?.state === 'override' || snapshot?.state === 'unavailable';
    const height = onboardingOpen ? 570 : settingsOpen ? 650 : detailsOpen ? 545 : releaseOpen ? 475 : passOpen ? 410 : hasStatus ? 475 : 390;
    void getCurrentWindow().setSize(new LogicalSize(380, height)).catch(() => {});
  }

  onMount(() => {
    let active = true;
    const refresh = async () => {
      try {
        const next = await getState();
        if (!active) return;
        snapshot = next;
        if (!settingsOpen) {
          allowance = next.allowanceWeeklyPoints;
          launchAtLogin = next.launchAtLogin;
          notificationsEnabled = next.notificationsEnabled;
          notificationSoundEnabled = next.notificationSoundEnabled;
        }
        onboardingOpen = !next.onboardingCompleted;
      } catch (cause) {
        if (active) error = String(cause);
      }
    };
    refresh();
    isInApplications().then((value) => (installedInApplications = value)).catch(() => {});
    const updateTimer = window.setTimeout(() => void checkUpdates(true), 1800);
    const poll = window.setInterval(refresh, 1500);
    const clock = window.setInterval(() => (now = Date.now()), 1000);
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') void hideMainWindow();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => {
      active = false;
      clearInterval(poll);
      clearInterval(clock);
      clearTimeout(updateTimer);
      window.removeEventListener('keydown', onKeyDown);
    };
  });

  function remainingLabel(value: string | null, idle = 'Starts with your next use'): string {
    if (!value) return idle;
    const milliseconds = new Date(value).getTime() - now;
    if (milliseconds <= 0) return 'Resetting now';
    const hours = Math.floor(milliseconds / 3_600_000);
    const minutes = Math.floor((milliseconds % 3_600_000) / 60_000);
    return hours === 0 ? `Resets in ${minutes} min` : `Resets in ${hours}h ${minutes}m`;
  }

  function clockLabel(value: string | null): string {
    if (!value) return '—';
    return new Date(value).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
  }

  function percent(value: number | null): string {
    return value == null ? '—' : `${Math.max(0, Math.round(value))}%`;
  }

  function barStyle(remaining: number | null): string {
    const value = Math.max(0, Math.min(100, remaining ?? 0));
    const color = value > 50 ? '#67d99b' : value > 20 ? '#f0b45a' : '#ff716d';
    return `--value:${value}%;--meter:${color}`;
  }

  function confidenceLabel(value: GateSnapshotV1['confidence'] | undefined): string {
    if (value === 'official') return 'Official';
    if (value === 'calibrated') return 'Calibrated';
    if (value === 'coarse') return 'Estimated';
    return 'Offline';
  }

  function statusTitle(state: GateSnapshotV1['state'] | undefined): string {
    if (state === 'exhausted') return 'New prompts are paused on this Mac';
    if (state === 'override') return 'Emergency access is active';
    if (state === 'warning') return 'Your five-hour budget is getting low';
    return 'Usage information is temporarily unavailable';
  }

  function statusDescription(state: GateSnapshotV1['state'] | undefined): string {
    if (state === 'exhausted') return `You can send a new prompt after ${clockLabel(snapshot?.windowEndsAt ?? null)}. Anything already running can finish.`;
    if (state === 'override') return `You can send new prompts until ${clockLabel(snapshot?.overrideEndsAt ?? null)}.`;
    if (state === 'warning') return 'QuotaBar will pause your next new prompt if this reaches 0%.';
    return 'QuotaBar will not pause any prompts until the connection returns.';
  }

  function burnRateLabel(): string {
    const rate = snapshot?.burnRatePerHour;
    if (rate == null || rate <= 0) return 'Not enough activity yet';
    if (rate >= 100) return 'Recalculating';
    return `${rate.toFixed(1)}% per hour`;
  }

  async function run(label: string, action: () => Promise<GateSnapshotV1>) {
    busy = label; error = '';
    try {
      snapshot = await action();
      if (!settingsOpen && snapshot) {
        allowance = snapshot.allowanceWeeklyPoints;
        launchAtLogin = snapshot.launchAtLogin;
        notificationsEnabled = snapshot.notificationsEnabled;
        notificationSoundEnabled = snapshot.notificationSoundEnabled;
      }
    } catch (cause) { error = String(cause); }
    finally { busy = ''; }
  }

  async function saveSettings() {
    await run('settings', () => updateSettings({
      allowanceWeeklyPoints: allowance,
      launchAtLogin,
      notificationsEnabled,
      notificationSoundEnabled,
      onboardingCompleted: snapshot?.onboardingCompleted ?? true
    }));
  }

  async function usePass() {
    busy = 'override'; error = '';
    try { const response = await requestOverride(phrase); snapshot = response.snapshot; }
    catch (cause) { error = String(cause); }
    finally { busy = ''; }
  }

  async function deleteAllHistory() {
    if (window.confirm('Delete all QuotaBar usage history and calibration data from this Mac?')) await run('history', deleteHistory);
  }

  async function checkUpdates(silent = false) {
    if (!silent) busy = 'update';
    if (!silent) updateMessage = 'Checking for updates…';
    try {
      const result = await checkForUpdates();
      currentVersion = result.currentVersion;
      availableUpdate = result.availableVersion;
      releaseNotes = result.notes;
      updateMessage = result.availableVersion
        ? `Version ${result.availableVersion} is ready to install`
        : `QuotaBar ${result.currentVersion} is up to date`;
    } catch {
      if (!silent) updateMessage = 'Could not check right now. Try again later.';
    } finally {
      if (!silent) busy = '';
    }
  }

  async function testAlerts() {
    busy = 'alerts'; error = '';
    try {
      await sendTestNotification();
      alertsTested = true;
    } catch (cause) { error = String(cause); }
    finally { busy = ''; }
  }

  async function finishOnboarding() {
    busy = 'onboarding'; error = '';
    try {
      snapshot = await completeOnboarding();
      onboardingOpen = false;
    } catch (cause) { error = String(cause); }
    finally { busy = ''; }
  }

  async function applyUpdate() {
    busy = 'update';
    updateMessage = 'Installing update… QuotaBar will reopen.';
    try {
      await installUpdate();
    } catch {
      updateMessage = 'The update could not be installed. Try again later.';
      busy = '';
    }
  }

  function goBack() { settingsOpen = false; detailsOpen = false; passOpen = false; releaseOpen = false; error = ''; }
</script>

<svelte:head><title>QuotaBar</title></svelte:head>

<main class:locked={snapshot?.state === 'exhausted'}>
  <header>
    {#if onboardingOpen}
      <span class="header-spacer"></span>
      <strong class="view-title">Welcome to QuotaBar</strong>
      <button class="icon-button" aria-label="Hide QuotaBar" title="Hide QuotaBar" on:click={hideMainWindow}><svg viewBox="0 0 20 20" aria-hidden="true"><path d="m5.5 5.5 9 9m0-9-9 9" /></svg></button>
    {:else if settingsOpen || detailsOpen || passOpen || releaseOpen}
      <button class="icon-button back" aria-label="Back" on:click={goBack}><svg viewBox="0 0 20 20" aria-hidden="true"><path d="m12.5 4.5-5 5 5 5" /></svg></button>
      <strong class="view-title">{settingsOpen ? 'Settings' : detailsOpen ? 'How it works' : releaseOpen ? 'What’s new' : 'Emergency pass'}</strong>
      <button class="icon-button" aria-label="Hide QuotaBar" title="Hide QuotaBar" on:click={hideMainWindow}><svg viewBox="0 0 20 20" aria-hidden="true"><path d="m5.5 5.5 9 9m0-9-9 9" /></svg></button>
    {:else}
      <div class="brand"><span class="mini-logo" aria-hidden="true"><span></span></span><span>QuotaBar</span></div>
      <div class="header-actions"><button class="icon-button release-button" aria-label="See what’s new" title="What’s new" on:click={() => (releaseOpen = true)}><svg viewBox="0 0 20 20" aria-hidden="true"><path d="M5.2 8.1a4.8 4.8 0 0 1 9.6 0c0 5 2 5.2 2 5.2H3.2s2-.2 2-5.2ZM8.2 15.7h3.6" /></svg>{#if availableUpdate}<i></i>{/if}</button><button class="icon-button" aria-label="Open settings" title="Settings" on:click={() => (settingsOpen = true)}><svg viewBox="0 0 20 20" aria-hidden="true"><path d="M10 6.8A3.2 3.2 0 1 0 10 13.2 3.2 3.2 0 0 0 10 6.8Z" /><path d="M16 11.1v-2.2l-1.8-.5a6 6 0 0 0-.5-1.1l.9-1.7L13 4l-1.7.9a6 6 0 0 0-1.1-.5L9.6 2.6H7.4l-.5 1.8a6 6 0 0 0-1.1.5L4.1 4 2.5 5.6l.9 1.7a6 6 0 0 0-.5 1.1l-1.8.5v2.2l1.8.5a6 6 0 0 0 .5 1.1l-.9 1.7L4.1 16l1.7-.9a6 6 0 0 0 1.1.5l.5 1.8h2.2l.5-1.8a6 6 0 0 0 1.1-.5l1.7.9 1.6-1.6-.9-1.7a6 6 0 0 0 .5-1.1l1.9-.5Z" /></svg></button><button class="icon-button" aria-label="Hide QuotaBar" title="Hide QuotaBar" on:click={hideMainWindow}><svg viewBox="0 0 20 20" aria-hidden="true"><path d="m5.5 5.5 9 9m0-9-9 9" /></svg></button></div>
    {/if}
  </header>

  {#if onboardingOpen}
    <section class="onboarding-intro"><span class="welcome-logo"><span></span></span><h1>Let’s set up your menu-bar guardrail</h1><p>Three quick checks make sure QuotaBar starts automatically and can warn you before your budget reaches zero.</p></section>
    <section class="setup-list">
      <article><span class:done={installedInApplications}>1</span><div><strong>Keep QuotaBar in Applications</strong><p>{installedInApplications ? 'QuotaBar is installed in the right place.' : 'Open Applications, then drag QuotaBar there from Downloads.'}</p></div>{#if !installedInApplications}<button on:click={openApplicationsFolder}>Open</button>{/if}</article>
      <article><span class:done={alertsTested}>2</span><div><strong>Turn on warning alerts</strong><p>macOS notifications with a sound at 75%, 50%, 30%, and 0% remaining.</p></div><button disabled={busy !== ''} on:click={testAlerts}>{alertsTested ? 'Tested' : 'Enable & test'}</button></article>
      <article><span class:done={snapshot?.desktopHookInstalled}>3</span><div><strong>Pause new Mac prompts at zero</strong><p>Only new prompts in the Codex app on this Mac are paused.</p></div>{#if !snapshot?.desktopHookInstalled}<button disabled={busy !== ''} on:click={() => run('hook', installDesktopHook)}>Turn on</button>{/if}</article>
    </section>
    <button class="primary save" disabled={busy !== ''} on:click={finishOnboarding}>Finish setup</button>
    <p class="onboarding-foot">QuotaBar will open automatically when you restart your Mac.</p>
    {#if error}<p class="error">{error}</p>{/if}
  {:else if releaseOpen}
    <section class="page-intro release-intro"><span class="release-symbol">↗</span><h1>{availableUpdate ? `QuotaBar ${availableUpdate} is ready` : 'What’s new in QuotaBar'}</h1><p>{availableUpdate ? 'Update inside the app. QuotaBar will download, replace itself, and reopen.' : `You’re using QuotaBar ${currentVersion || '0.4.0'}.`}</p></section>
    <section class="release-card">
      <strong>Latest improvements</strong>
      <ul><li>Reliable alerts at 75%, 50%, 30%, and 0% remaining</li><li>Default macOS notification sound and an alert test</li><li>Automatic update checks and one-click installation</li><li>Clearer menu-bar text and first-run setup</li></ul>
      {#if releaseNotes}<p class="release-note">{releaseNotes}</p>{/if}
    </section>
    {#if availableUpdate}<button class="primary save" disabled={busy !== ''} on:click={applyUpdate}>{busy === 'update' ? 'Installing…' : `Update to ${availableUpdate}`}</button>{:else}<button class="secondary-action" disabled={busy !== ''} on:click={() => checkUpdates(false)}>{busy === 'update' ? 'Checking…' : 'Check for updates'}</button>{/if}
  {:else if !settingsOpen && !detailsOpen && !passOpen}
    <section class="hero state-{snapshot?.state ?? 'unavailable'}">
      <div class="eyebrow-row"><span>Five-hour budget <button type="button" class="help" aria-label="What is the five-hour budget?" data-tip="A personal limit that resets five hours after Codex usage begins.">?</button></span></div>
      <div class="hero-number">
        {#if snapshot?.windowStartedAt}<strong>{percent(snapshot.fiveHourRemainingPercent)}</strong><span>remaining</span>
        {:else}<strong class="ready">Ready</strong><span>starts with your next use</span>{/if}
      </div>
      <div class="progress large" style={barStyle(snapshot?.fiveHourRemainingPercent ?? null)}><span></span></div>
      {#if snapshot?.windowStartedAt}
        <div class="reset-row"><span>{remainingLabel(snapshot.windowEndsAt)}</span>{#if snapshot.projectedExhaustionAt && snapshot.state !== 'exhausted'}<span>At this pace: {clockLabel(snapshot.projectedExhaustionAt)}</span>{/if}</div>
      {/if}
    </section>

    <section class="week-card">
      <div><span class="section-label">Weekly usage <button type="button" class="help" aria-label="What is weekly usage?" data-tip="Your official account-wide Codex limit reported by OpenAI.">?</button></span><strong>{percent(snapshot?.weeklyRemainingPercent ?? null)} left</strong></div>
      <div class="progress" style={barStyle(snapshot?.weeklyRemainingPercent ?? null)}><span></span></div>
      <span class="muted">{remainingLabel(snapshot?.weeklyResetsAt ?? null, 'Waiting for account meter')}</span>
    </section>

    {#if snapshot?.state === 'warning' || snapshot?.state === 'exhausted' || snapshot?.state === 'override' || snapshot?.state === 'unavailable'}
      <section class="status-card state-{snapshot.state}">
        <span class="status-icon" aria-hidden="true">{snapshot.state === 'exhausted' ? '!' : snapshot.state === 'unavailable' ? '?' : '✓'}</span>
        <div><strong>{statusTitle(snapshot.state)}</strong><p>{statusDescription(snapshot.state)}</p></div>
      </section>
    {/if}
    {#if snapshot?.state === 'exhausted'}<button class="primary emergency" on:click={() => (passOpen = true)}>Use emergency pass</button>{/if}
    {#if error}<p class="error">{error}</p>{/if}
    <nav class="bottom-actions" aria-label="QuotaBar actions">
      <button on:click={() => (detailsOpen = true)}><svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="10" cy="10" r="7.5" /><path d="M10 9.2v4.2M10 6.5h.01" /></svg>How it works</button>
      <span></span><div class:healthy={snapshot?.desktopHookInstalled && snapshot?.desktopClassificationHealthy} class="gate-state"><i></i>Pause at 0%: {snapshot?.desktopHookInstalled ? 'On' : 'Off'} <button type="button" class="help tip-left" aria-label="What does pause at zero mean?" data-tip="At 0%, QuotaBar stops the next new Codex prompt on this Mac until the timer resets.">?</button></div>
    </nav>
  {:else if passOpen}
    <section class="page-intro"><span class="pass-symbol">15</span><h1>Need a little more time?</h1><p>You can open desktop prompts for 15 minutes, or until the current window resets.</p></section>
    <section class="form-card">
      {#if snapshot?.overrideRequestedAt && !snapshot.overrideEndsAt}<div class="countdown"><span>Cooling-off period</span><strong>{remainingLabel(snapshot.overrideAvailableAt, '60 seconds')}</strong></div>{/if}
      <label for="pass-phrase">Type this phrase to continue</label><code>Use my one-time 15-minute pass</code>
      <input id="pass-phrase" bind:value={phrase} placeholder="Type the phrase exactly" />
      <button class="primary" disabled={busy !== '' || snapshot?.overrideUsed} on:click={usePass}>{snapshot?.overrideRequestedAt ? 'Activate 15-minute pass' : 'Start 60-second wait'}</button>
      <p class="fine-print">You can use this once per five-hour window. Usage continues counting.</p>
    </section>
    {#if error}<p class="error">{error}</p>{/if}
  {:else if settingsOpen}
    <section class="settings-section"><h2>General</h2><div class="settings-group">
      <label class="setting-row"><span><strong>Open at login</strong><small>Keep the meter available automatically</small></span><input type="checkbox" bind:checked={launchAtLogin} /></label>
      <label class="setting-row"><span><strong>Usage notifications</strong><small>Alerts at 75%, 50%, 30%, and 0% remaining</small></span><input type="checkbox" bind:checked={notificationsEnabled} /></label>
      <label class="setting-row"><span><strong>Notification sound</strong><small>Play the standard macOS alert sound</small></span><input type="checkbox" bind:checked={notificationSoundEnabled} /></label>
      <div class="gate-setting"><div><strong>Test your alerts</strong><small>Send a sample notification now</small></div><button disabled={busy !== ''} on:click={testAlerts}>Test</button></div>
    </div></section>
    <section class="settings-section"><h2>Usage allowed every five hours</h2><div class="settings-group budget-setting">
      <div class="setting-heading"><span><strong>{allowance === 16 ? 'Standard' : `${Math.round((allowance / 16) * 100)}% of standard`} <button type="button" class="help" aria-label="What is the standard limit?" data-tip="Standard matches the historical five-hour limit. Move left if you want QuotaBar to pause you sooner.">?</button></strong><small>Choose how quickly QuotaBar should pause new prompts</small></span></div>
      <input class="range" type="range" min="1" max="16" step="1" bind:value={allowance} /><div class="range-labels"><span>Pause sooner</span><span>Standard</span></div>
    </div></section>
    <section class="settings-section"><h2>Pause new prompts at 0%</h2><div class="settings-group">
      <div class="gate-setting"><div><strong>{snapshot?.desktopHookInstalled ? (snapshot.desktopClassificationHealthy ? 'On' : 'Needs attention') : 'Off'}</strong><small>Stops a new Codex prompt on this Mac when the five-hour budget is empty</small></div>
        {#if !snapshot?.desktopHookInstalled}<button disabled={busy !== ''} on:click={() => run('hook', installDesktopHook)}>Turn on</button>{:else if !snapshot.desktopClassificationHealthy}<button disabled={busy !== ''} on:click={() => run('hook', repairDesktopHook)}>Fix</button>{/if}
      </div>
      {#if snapshot?.desktopHookInstalled}<button class="text-danger" disabled={busy !== ''} on:click={() => run('hook', removeDesktopHook)}>Turn off prompt pausing</button>{/if}
    </div></section>
    <section class="settings-section"><h2>Updates</h2><div class="settings-group">
      <div class="gate-setting"><div><strong>{availableUpdate ? `QuotaBar ${availableUpdate}` : 'Keep QuotaBar current'}</strong><small>{updateMessage}</small></div>
        <button disabled={busy !== ''} on:click={availableUpdate ? applyUpdate : () => checkUpdates(false)}>{availableUpdate ? 'Install' : busy === 'update' ? 'Checking…' : 'Check'}</button>
      </div>
    </div></section>
    <button class="primary save" disabled={busy !== ''} on:click={saveSettings}>Save changes</button>
    <button class="delete" disabled={busy !== ''} on:click={deleteAllHistory}>Delete local usage history…</button>
    {#if error}<p class="error">{error}</p>{/if}
  {:else if detailsOpen}
    <section class="page-intro details-intro"><span class="info-symbol">i</span><h1>How it works</h1><p>QuotaBar helps you spread your Codex usage across the week.</p></section>
    <section class="steps">
      <article><span>1</span><div><strong>Check your weekly total</strong><p>Usage from the Codex app, CLI, IDE, web, cloud, and your other devices all counts.</p></div></article>
      <article><span>2</span><div><strong>Start a five-hour budget</strong><p>The timer begins when QuotaBar detects new usage and resets five hours later.</p></div></article>
      <article><span>3</span><div><strong>Pause at zero</strong><p>At 0%, QuotaBar stops you from sending a new prompt in Codex on this Mac. Anything already running can finish.</p></div></article>
    </section>
    <details class="troubleshooting"><summary>Troubleshooting information</summary>
      <section class="details-list">
        <div><span>OpenAI usage connection</span><strong>{snapshot?.appServerConnected ? 'Online' : 'Reconnecting'}</strong></div><div><span>Local activity updates</span><strong>{snapshot?.sessionLogsConnected ? 'Working' : 'Unavailable'}</strong></div>
        <div><span>Prompt pausing</span><strong>{snapshot?.desktopHookInstalled ? (snapshot.desktopClassificationHealthy ? 'Working' : 'Needs attention') : 'Off'}</strong></div><div><span>Estimate quality</span><strong>{confidenceLabel(snapshot?.confidence)}</strong></div>
        <div><span>Current pace</span><strong>{burnRateLabel()}</strong></div>
      </section>
      {#if snapshot && snapshot.availableBuckets.length > 0}<div class="bucket-info">{#each snapshot.availableBuckets as bucket}<code>{bucket.quotaId}: {[bucket.primary, bucket.secondary].filter(Boolean).map((window) => `${window?.windowMinutes}m`).join(' + ') || 'no windows'}</code>{/each}</div>{/if}
    </details>
  {/if}
</main>

<style>
  :global(*){box-sizing:border-box}:global(html){background:transparent;color-scheme:dark;font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text",sans-serif}:global(body){margin:0;min-width:360px;overflow-x:hidden;background:transparent;color:#f5f5f7}:global(button),:global(input){font:inherit}
  main{min-height:100vh;padding:13px 14px 14px;background:linear-gradient(180deg,rgba(35,35,38,.98),rgba(24,24,26,.98));border:1px solid rgba(255,255,255,.11);border-radius:18px;box-shadow:0 18px 55px rgba(0,0,0,.42)}
  header{height:38px;display:flex;align-items:center;justify-content:space-between;margin-bottom:10px}.header-actions{display:flex;align-items:center;gap:1px}.brand{display:flex;align-items:center;gap:9px;font-size:14px;font-weight:680;letter-spacing:-.015em}.mini-logo{width:23px;height:23px;display:grid;place-items:center;border-radius:7px;background:linear-gradient(145deg,#25342e,#111814);box-shadow:inset 0 1px 0 rgba(255,255,255,.12)}.mini-logo span{width:12px;height:12px;border:3px solid #6bdd9f;border-right-color:rgba(107,221,159,.24);border-radius:50%;position:relative}.mini-logo span::after{content:'';position:absolute;width:5px;height:3px;right:-4px;bottom:-2px;border-radius:2px;transform:rotate(45deg);background:#6bdd9f}
  .icon-button{position:relative;width:30px;height:30px;display:grid;place-items:center;padding:0;color:#a9a9ae;border:0;border-radius:8px;background:transparent;cursor:pointer}.icon-button:hover{color:#fff;background:rgba(255,255,255,.07)}.icon-button svg{width:18px;fill:none;stroke:currentColor;stroke-width:1.45;stroke-linecap:round;stroke-linejoin:round}.release-button i{position:absolute;top:4px;right:4px;width:6px;height:6px;border-radius:50%;background:#67d99b;box-shadow:0 0 0 2px #242426}.view-title{font-size:13px;font-weight:650}.header-spacer{width:30px}
  .hero{padding:17px;border:1px solid rgba(255,255,255,.09);border-radius:15px;background:radial-gradient(circle at 88% 5%,rgba(103,217,155,.13),transparent 38%),rgba(255,255,255,.035)}.hero.state-exhausted{background:radial-gradient(circle at 88% 5%,rgba(255,113,109,.14),transparent 40%),rgba(255,255,255,.035)}
  .eyebrow-row,.reset-row,.week-card>div:first-child,.bottom-actions,.setting-row,.setting-heading,.gate-setting{display:flex;align-items:center;justify-content:space-between}.eyebrow-row{color:#b4b4ba;font-size:11px;font-weight:590}
  .help{position:relative;display:inline-grid;width:14px;height:14px;padding:0;place-items:center;margin-left:3px;color:#9b9ba1;border:1px solid rgba(255,255,255,.18);border-radius:50%;background:transparent;font-size:9px;font-weight:650;line-height:1;vertical-align:1px;cursor:help}.help::after{content:attr(data-tip);position:absolute;z-index:20;top:21px;left:-7px;width:220px;padding:8px 9px;color:#ededf0;border:1px solid rgba(255,255,255,.13);border-radius:8px;background:#353538;box-shadow:0 8px 24px rgba(0,0,0,.38);font-size:10px;font-weight:450;line-height:1.4;text-align:left;opacity:0;pointer-events:none;transform:translateY(-3px);transition:.14s}.help:hover::after,.help:focus::after{opacity:1;transform:translateY(0)}.help.tip-left::after{right:-6px;left:auto}
  .hero-number{display:flex;align-items:baseline;gap:8px;margin:12px 0 13px}.hero-number strong{font-size:42px;line-height:1;letter-spacing:-.055em;font-variant-numeric:tabular-nums}.hero-number strong.ready{font-size:34px}.hero-number span{color:#8f8f95;font-size:12px}
  .progress{height:6px;overflow:hidden;border-radius:20px;background:rgba(255,255,255,.08)}.progress span{display:block;width:var(--value);height:100%;border-radius:inherit;background:var(--meter);box-shadow:0 0 12px color-mix(in srgb,var(--meter) 38%,transparent);transition:width .35s ease}.progress.large{height:7px}.reset-row{margin-top:9px;color:#88888e;font-size:10px}
  .week-card{margin-top:9px;padding:13px 14px;border-radius:13px;background:rgba(255,255,255,.035)}.week-card>div:first-child{margin-bottom:10px}.week-card strong{font-size:13px}.section-label{color:#b8b8bd;font-size:11px}.muted{display:block;margin-top:8px;color:#85858b;font-size:10px}
  .status-card{display:grid;grid-template-columns:28px 1fr;gap:10px;align-items:start;margin-top:9px;padding:13px;border:1px solid rgba(103,217,155,.13);border-radius:13px;background:rgba(103,217,155,.055)}.status-card.state-exhausted{border-color:rgba(255,113,109,.17);background:rgba(255,113,109,.06)}.status-card.state-unavailable{border-color:rgba(255,255,255,.08);background:rgba(255,255,255,.03)}.status-icon{width:25px;height:25px;display:grid;place-items:center;border-radius:50%;color:#102219;background:#67d99b;font-size:13px;font-weight:800}.state-exhausted .status-icon{color:#34100f;background:#ff716d}.state-unavailable .status-icon{color:#29292c;background:#9a9aa0}.status-card strong{display:block;margin:1px 0 3px;font-size:12px}.status-card p{margin:0;color:#99999f;font-size:10.5px;line-height:1.42}
  button{color:#ececf0;border:0;cursor:pointer}button:disabled{opacity:.45;cursor:default}.primary{width:100%;padding:10px 13px;border-radius:10px;color:#102219;background:#69dda0;font-size:12px;font-weight:680;box-shadow:inset 0 1px 0 rgba(255,255,255,.25)}.primary:hover{background:#78e5ab}.primary.emergency{margin-top:9px;color:#331211;background:#ff8783}
  .bottom-actions{height:38px;margin-top:6px;color:#8d8d93}.bottom-actions button{display:inline-flex;align-items:center;gap:6px;padding:7px 4px;color:#939399;background:transparent;font-size:10.5px}.bottom-actions button:hover{color:#fff}.bottom-actions svg{width:14px;fill:none;stroke:currentColor;stroke-width:1.5;stroke-linecap:round}.bottom-actions>span{flex:1}.gate-state{display:inline-flex;align-items:center;gap:6px;font-size:10.5px}.gate-state i{width:6px;height:6px;border-radius:50%;background:#77777d}.gate-state.healthy i{background:#67d99b}.error{margin:9px 0 0;padding:9px 10px;color:#ffaaa7;border-radius:9px;background:rgba(255,113,109,.09);font-size:10px;line-height:1.4}
  .page-intro{padding:16px 12px 17px;text-align:center}.page-intro h1{margin:12px 0 6px;font-size:19px;letter-spacing:-.025em}.page-intro p{max-width:300px;margin:auto;color:#929298;font-size:11px;line-height:1.5}.pass-symbol,.info-symbol{width:48px;height:48px;display:grid;place-items:center;margin:auto;border-radius:15px;color:#102219;background:linear-gradient(145deg,#7be7ad,#4ecf8a);font-size:16px;font-weight:750;box-shadow:0 9px 28px rgba(67,198,131,.2),inset 0 1px 0 rgba(255,255,255,.4)}.info-symbol{border-radius:50%;font-family:Georgia,serif;font-style:italic}
  .onboarding-intro{padding:7px 14px 15px;text-align:center}.onboarding-intro h1{max-width:300px;margin:12px auto 7px;font-size:20px;line-height:1.12;letter-spacing:-.035em}.onboarding-intro p{max-width:320px;margin:0 auto;color:#96969c;font-size:10.5px;line-height:1.45}.welcome-logo{width:52px;height:52px;display:grid;place-items:center;margin:auto;border-radius:16px;background:linear-gradient(145deg,#263c32,#101713);box-shadow:0 12px 34px rgba(63,202,132,.16),inset 0 1px 0 rgba(255,255,255,.14)}.welcome-logo span{position:relative;width:27px;height:27px;border:6px solid #69dda0;border-right-color:rgba(105,221,160,.22);border-radius:50%}.welcome-logo span::after{content:'';position:absolute;right:-8px;bottom:-4px;width:10px;height:5px;border-radius:3px;background:#69dda0;transform:rotate(45deg)}.setup-list{overflow:hidden;border:1px solid rgba(255,255,255,.08);border-radius:13px;background:rgba(255,255,255,.035)}.setup-list article{display:grid;grid-template-columns:27px 1fr auto;gap:9px;align-items:center;min-height:70px;padding:10px 11px;border-bottom:1px solid rgba(255,255,255,.065)}.setup-list article:last-child{border-bottom:0}.setup-list article>span{width:23px;height:23px;display:grid;place-items:center;border-radius:7px;color:#a8a8ad;background:rgba(255,255,255,.07);font-size:10px;font-weight:700}.setup-list article>span.done{color:#102219;background:#67d99b}.setup-list strong{display:block;font-size:11px}.setup-list p{margin:3px 0 0;color:#88888e;font-size:9px;line-height:1.35}.setup-list button,.secondary-action{padding:6px 9px;border-radius:7px;color:#bff4d4;background:rgba(103,217,155,.11);font-size:9.5px}.onboarding-foot{margin:9px 0 0;color:#77777d;text-align:center;font-size:9.5px}.release-intro{padding-top:9px}.release-symbol{width:44px;height:44px;display:grid;place-items:center;margin:auto;border-radius:14px;color:#102219;background:#67d99b;font-size:18px;font-weight:700}.release-card{padding:14px;border:1px solid rgba(255,255,255,.08);border-radius:13px;background:rgba(255,255,255,.035)}.release-card>strong{font-size:11.5px}.release-card ul{padding:0;margin:11px 0 0;list-style:none}.release-card li{position:relative;margin:8px 0;padding-left:14px;color:#aaaab0;font-size:10px;line-height:1.35}.release-card li::before{content:'';position:absolute;top:5px;left:0;width:6px;height:6px;border-radius:50%;background:#67d99b}.release-note{margin:12px 0 0;padding-top:10px;border-top:1px solid rgba(255,255,255,.07);color:#838389;font-size:9.5px;line-height:1.4}.secondary-action{width:100%;margin-top:12px;padding:10px}
  .form-card,.settings-group,.details-list{overflow:hidden;border:1px solid rgba(255,255,255,.08);border-radius:13px;background:rgba(255,255,255,.035)}.form-card{padding:14px}.form-card label{display:block;color:#aaaab0;font-size:10px}.form-card code{display:block;margin:7px 0 10px;color:#d6d6da;font:10px ui-monospace,SFMono-Regular,Menlo,monospace}.form-card input{width:100%;margin-bottom:9px;padding:10px 11px;color:#f5f5f7;border:1px solid rgba(255,255,255,.13);outline:none;border-radius:9px;background:rgba(0,0,0,.2);font-size:11px}.form-card input:focus{border-color:rgba(103,217,155,.6);box-shadow:0 0 0 3px rgba(103,217,155,.09)}.fine-print{margin:9px 0 0;color:#797980;text-align:center;font-size:9.5px}.countdown{display:flex;align-items:center;justify-content:space-between;margin:-2px 0 13px;padding:10px;border-radius:9px;color:#aaaab0;background:rgba(240,180,90,.08);font-size:10px}.countdown strong{color:#f0c178}
  .settings-section{margin-top:13px}.settings-section h2{margin:0 0 7px 4px;color:#8f8f95;font-size:10px;font-weight:620;text-transform:uppercase;letter-spacing:.07em}.setting-row{min-height:57px;padding:10px 12px;border-bottom:1px solid rgba(255,255,255,.065)}.setting-row:last-child{border-bottom:0}.setting-row span,.setting-heading span,.gate-setting div{display:grid;gap:3px}.setting-row strong,.setting-heading strong,.gate-setting strong{font-size:11.5px;font-weight:590}.setting-row small,.setting-heading small,.gate-setting small{color:#85858b;font-size:9.5px}.setting-row input[type=checkbox]{width:31px;height:18px;appearance:none;padding:2px;border-radius:20px;background:#55555a;transition:.2s}.setting-row input[type=checkbox]::after{content:'';display:block;width:14px;height:14px;border-radius:50%;background:#fff;box-shadow:0 1px 3px rgba(0,0,0,.4);transition:transform .2s}.setting-row input[type=checkbox]:checked{background:#52ce88}.setting-row input[type=checkbox]:checked::after{transform:translateX(13px)}
  .budget-setting{padding:12px}.range{width:100%;margin:13px 0 3px;accent-color:#67d99b}.range-labels{display:flex;justify-content:space-between;color:#77777d;font-size:9px}.gate-setting{min-height:58px;padding:10px 12px;gap:10px}.gate-setting div{min-width:0}.gate-setting small{max-width:245px;line-height:1.35}.gate-setting button{flex:none;padding:6px 10px;border-radius:7px;color:#bff4d4;background:rgba(103,217,155,.11);font-size:10px}.text-danger{width:100%;padding:10px;color:#ff918d;border-top:1px solid rgba(255,255,255,.065);background:transparent;font-size:10.5px}.save{margin-top:14px}.delete{width:100%;margin-top:7px;padding:9px;color:#c77f7c;background:transparent;font-size:10px}
  .details-intro{padding-top:8px;padding-bottom:13px}.steps{overflow:hidden;border:1px solid rgba(255,255,255,.08);border-radius:13px;background:rgba(255,255,255,.035)}.steps article{display:grid;grid-template-columns:27px 1fr;gap:9px;padding:11px 12px;border-bottom:1px solid rgba(255,255,255,.065)}.steps article:last-child{border-bottom:0}.steps article>span{width:23px;height:23px;display:grid;place-items:center;color:#bdf3d2;border-radius:7px;background:rgba(103,217,155,.1);font-size:10px;font-weight:700}.steps strong{display:block;margin:1px 0 3px;font-size:11px}.steps p{margin:0;color:#8f8f95;font-size:9.5px;line-height:1.42}.details-list{margin-top:9px}.details-list>div{display:flex;justify-content:space-between;padding:9px 10px;border-bottom:1px solid rgba(255,255,255,.065);font-size:9.5px}.details-list>div:last-child{border-bottom:0}.details-list span{color:#8f8f95}.details-list strong{font-weight:580}details{padding:10px 12px;border-radius:10px;background:rgba(255,255,255,.03);color:#8e8e94;font-size:10px}details summary{cursor:pointer}.troubleshooting{margin-top:10px}.bucket-info{margin-top:8px;padding-top:2px}.bucket-info code{display:block;margin-top:5px;font-size:8.5px}
</style>
