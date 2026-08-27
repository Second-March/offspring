<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import FormatFields from "$lib/components/FormatFields.svelte";
  import Neaticon from "$lib/components/Neaticon.svelte";
  import WindowControls from "$lib/components/WindowControls.svelte";
  import ConfirmDialog, { type ConfirmSpec } from "$lib/components/ConfirmDialog.svelte";
  import UpdateBanner from "$lib/components/UpdateBanner.svelte";
  import SettingsPane from "$lib/components/SettingsPane.svelte";
  import ToolPane from "$lib/components/ToolPane.svelte";
  import { TOOLS, type ToolId } from "$lib/tools";
  import { ensureTools } from "$lib/settingsUtils";
  import { windowDrag } from "$lib/windowDrag";
  import * as api from "$lib/api";
  import type { Preset, Settings, FfmpegStatus, UpdateInfo } from "$lib/types";

  let presets = $state<Preset[]>([]);
  let selectedId = $state<string | null>(null);
  let selectedToolId = $state<ToolId | null>(null);
  let settings = $state<Settings>({});
  let ffmpeg = $state<FfmpegStatus>({ found: false, path: null });
  // Build variant: "standard" includes the FFmpeg downloader, in-app
  // updater, and Win11 modern-menu integration. "studio" compiles
  // those out and the UI hides their buttons accordingly. Default
  // "standard" until the backend confirms; if the backend ever fails
  // to return we render the standard UI, which is strictly more
  // permissive (the underlying commands will return their own
  // studio-stub errors if mismatched).
  let buildVariant = $state<"standard" | "studio">("standard");
  let isStudio = $derived(buildVariant === "studio");
  // Platform marker. Defaults to "windows" until the async lookup
  // resolves — first paint on Windows therefore shows the right UI
  // immediately; first paint on Mac briefly shows the Windows-flavoured
  // text before the swap. Acceptable given the lookup is a single
  // Tauri command that resolves in <10ms.
  let platform = $state<"windows" | "macos" | "linux">("windows");
  let isMac = $derived(platform === "macos");
  // The frameless custom titlebar (drag region + window controls) only
  // exists on Windows — macOS keeps its native chrome + traffic lights.
  let frameless = $derived(platform === "windows");
  let tab = $state<"tools" | "presets">("presets");
  let showSettings = $state(false);
  let saving = $state(false);
  let savedTick = $state(0);
  // Surfaced in the header when a save is rejected. Null when the last
  // save succeeded or none has been attempted.
  let saveError = $state<string | null>(null);
  // False until presets + settings have actually been read from disk.
  // Every write path checks this so a failed load can't be persisted
  // back as empty/default state.
  let loaded = $state(false);
  // Right-click menu for preset rows. Non-null when visible.
  let ctxMenu = $state<{ x: number; y: number; preset: Preset } | null>(null);

  // The single themed dialog every confirm/alert in the app funnels
  // through — native confirm()/alert() pop the OS dialog inside the
  // Tauri shell, which breaks the app's visual language.
  let confirmDialog = $state<ConfirmSpec | null>(null);

  // Drag-and-drop reorder state. `dragId` is the preset being dragged;
  // `dragOver` is the row the cursor is currently over with a position
  // indicator telling us whether to drop above or below it. The drop-line
  // is rendered between rows based on this.
  let dragId = $state<string | null>(null);
  let dragOver = $state<{ id: string; pos: "above" | "below" } | null>(null);

  // FFmpeg download state (fed by the `ffmpeg-download` event from Rust)
  let dl = $state<{
    active: boolean;
    phase: string;
    percent: number | null;
    message: string | null;
    error: string | null;
  }>({ active: false, phase: "", percent: null, message: null, error: null });

  // Update-check state. We cache the most recent result in sessionStorage so
  // switching tabs in the webview doesn't re-hit GitHub on every mount.
  let update = $state<UpdateInfo | null>(null);
  const UPDATE_CACHE_KEY = "offspring.updateInfo";
  const UPDATE_DISMISS_KEY = "offspring.updateDismissedFor";

  // In-app update download state. `phase` drives the banner button:
  //   idle        — update detected, download not started
  //   downloading — streaming the installer in the background
  //   ready       — installer on disk, ready to run
  //   error       — download failed; fall back to browser download
  let upd = $state<{
    phase: "idle" | "downloading" | "ready" | "error";
    percent: number | null;
    message: string | null;
  }>({ phase: "idle", percent: null, message: null });

  // Manual "Check for updates" button state. `checking` drives the
  // spinner, `lastChecked` is the wall-clock time of the most recent
  // successful check, and `manualResult` is a one-shot status line
  // ("You're on the latest version.") shown after a manual check even
  // when no update is available. `currentVersion` is filled by the
  // first `check_for_updates` call — even a network-failed check
  // populates it from `CARGO_PKG_VERSION`, so we always have something
  // to display.
  let updateCheck = $state<{
    checking: boolean;
    lastChecked: number | null;
    manualResult: string | null;
  }>({ checking: false, lastChecked: null, manualResult: null });
  let currentVersion = $state<string>("");

  const selected = $derived(presets.find((p) => p.id === selectedId) ?? null);
  const selectedTool = $derived(TOOLS.find((t) => t.id === selectedToolId) ?? null);

  function showDialog(spec: ConfirmSpec) {
    confirmDialog = spec;
  }

  onMount(async () => {
    await reload();

    // Flush any pending auto-save (and any not-yet-run integration
    // sync) before the window closes. Registered via Tauri's
    // onCloseRequested API rather than `beforeunload` — WebView2 on
    // Windows doesn't fire beforeunload for native window-close actions.
    await getCurrentWindow().onCloseRequested(async (event) => {
      if (!loaded) return;
      const json = JSON.stringify(presets);
      const dirty = json !== lastSavedJson;
      if (!dirty && !syncPending) return;
      event.preventDefault();
      clearTimeout(saveTimer);
      clearTimeout(syncTimer);
      try {
        if (dirty) {
          // Full save: JSON + shell sync in one command.
          await api.savePresets(presets);
          lastSavedJson = json;
        } else {
          // JSON already on disk — just run the deferred shortcut sync.
          await api.syncIntegrations();
        }
        syncPending = false;
        getCurrentWindow().destroy();
      } catch (err) {
        // The backend refused the state (a negative width, a zero fps…).
        // Give the user the choice instead of silently losing the edit
        // or silently blocking the close.
        confirmDialog = {
          title: "Couldn't save your changes",
          message: `${err}\n\nClose anyway and lose the latest edits?`,
          confirmLabel: "Close anyway",
          onConfirm: () => getCurrentWindow().destroy(),
        };
      }
    });

    // No automatic update check at launch — we explicitly do NOT touch
    // the network without user action. The Settings → "Check for
    // updates" button is the only path that pings GitHub. We still
    // populate `currentVersion` (purely local — reads CARGO_PKG_VERSION
    // from Rust) so the Settings page can display "Current version: X"
    // without forcing a request. Network failures here would just
    // leave the field blank, which is fine.
    api.getAppVersion().then((v) => { currentVersion = v; }).catch(() => {});
    // Warm-start the banner from sessionStorage so cross-route nav
    // (e.g. progress → main) doesn't re-show a dismissed banner — but
    // this is a pure cache read with no network and no auto-download.
    try {
      const cached = sessionStorage.getItem(UPDATE_CACHE_KEY);
      const dismissedFor = sessionStorage.getItem(UPDATE_DISMISS_KEY);
      if (cached) {
        const parsed = JSON.parse(cached) as UpdateInfo;
        if (parsed.current) currentVersion = parsed.current;
        if (parsed.update_available && dismissedFor !== parsed.latest) {
          update = parsed;
        }
      }
    } catch {}

    // Subscribe to update download events before kicking off the check so
    // we never miss a "done" emitted from an auto-started download.
    await api.onUpdateDownload((e) => {
      if (e.phase === "downloading") {
        upd.phase = "downloading";
        upd.percent = e.percent;
        upd.message = e.message;
      } else if (e.phase === "done") {
        upd.phase = "ready";
        upd.percent = 100;
        upd.message = null;
      } else if (e.phase === "error") {
        upd.phase = "error";
        upd.percent = null;
        upd.message = e.message;
      }
    });

    // Subscribe to FFmpeg download events so the Settings pane can show
    // progress inline and flip the header badge when the install completes.
    await api.onFfmpegDownload(async (e) => {
      dl.phase = e.phase;
      dl.percent = e.percent;
      dl.message = e.message;
      if (e.phase === "error") {
        dl.active = false;
        dl.error = e.message ?? "Download failed";
      } else if (e.phase === "done") {
        dl.active = false;
        dl.error = null;
        // Re-check status so the header badge flips green immediately.
        ffmpeg = await api.ffmpegStatus();
      } else {
        dl.active = true;
        dl.error = null;
      }
    });
  });

  function startDownloadFfmpeg() {
    dl = { active: true, phase: "starting", percent: 0, message: "Starting…", error: null };
    api.downloadFfmpeg().catch((err) => {
      dl.active = false;
      dl.error = String(err);
    });
  }

  /// Manual update check. Only the Settings "Check for updates" button
  /// calls this — we deliberately do NOT trigger it automatically at
  /// app launch or on any background timer. Hitting GitHub is always
  /// an explicit user action.
  ///
  /// On a successful check that finds an update, we just show the
  /// banner — the installer is NOT downloaded automatically. The user
  /// has to click the banner's "Download" button to start a transfer.
  async function checkUpdate(opts: { manual?: boolean } = {}) {
    // Respect an in-session dismiss for this specific version: closing
    // the banner keeps it closed until the app is relaunched (or a
    // newer version lands). A manual re-check bypasses this — if the
    // user explicitly asks, honour it.
    const dismissedFor = opts.manual
      ? null
      : sessionStorage.getItem(UPDATE_DISMISS_KEY);

    if (opts.manual) {
      updateCheck.checking = true;
      updateCheck.manualResult = null;
    }
    try {
      const info = await api.checkForUpdates();
      sessionStorage.setItem(UPDATE_CACHE_KEY, JSON.stringify(info));
      if (info.current) currentVersion = info.current;
      updateCheck.lastChecked = Date.now();
      if (info.update_available && dismissedFor !== info.latest) {
        update = info;
        if (opts.manual) {
          updateCheck.manualResult = `Version ${info.latest} is available.`;
        }
      } else {
        update = null;
        if (opts.manual) {
          updateCheck.manualResult = info.latest
            ? `You're on the latest version (${info.current}).`
            : `Couldn't reach the update server. Try again later.`;
        }
      }
    } catch {
      // Network failure: only surface to the user on a manual check
      // (they asked, so we owe them feedback). Silent paths stay silent.
      if (opts.manual) {
        updateCheck.manualResult = "Couldn't reach the update server. Try again later.";
      }
    } finally {
      if (opts.manual) updateCheck.checking = false;
    }
  }

  // Level 2 behaviour: as soon as we know a newer version is out there,
  // eagerly stream the installer in the background. No user interaction
  // needed until they're ready to restart. Skipped if there's no direct
  // installer URL on the release (we fall back to opening the release page).
  function maybeStartDownload(info: UpdateInfo) {
    if (!info.installer_url) return;
    if (upd.phase === "downloading" || upd.phase === "ready") return;
    upd = { phase: "downloading", percent: 0, message: "Starting…" };
    api.downloadUpdate(info.latest, info.installer_url).catch((err) => {
      upd.phase = "error";
      upd.percent = null;
      upd.message = String(err);
    });
  }

  async function onUpdateClick() {
    if (!update) return;
    if (upd.phase === "ready") {
      // Installer is on disk — run it silently and exit. Inno Setup's
      // /RESTARTAPPLICATIONS will re-launch Offspring after the swap.
      try {
        await api.installUpdate(update.latest);
      } catch (err) {
        upd.phase = "error";
        upd.message = String(err);
      }
      return;
    }
    if (upd.phase === "error" || !update.installer_url) {
      // Download failed or there's no installer asset — open the release
      // page so the user can grab it manually. Via the Rust shell-open
      // command, NOT plugin-opener's openUrl: the JS path fails silently
      // in packaged builds (seen on both WebView2 and macOS), which left
      // this button dead exactly when the user needed the fallback.
      try {
        await api.openExternalUrl(update.installer_url || update.html_url);
      } catch (err) {
        saveError = String(err);
      }
      return;
    }
    // "idle" or still "downloading" — if we haven't kicked off yet, do so
    // now; otherwise the click is a no-op while progress ticks.
    if (upd.phase === "idle") maybeStartDownload(update);
  }

  function dismissUpdate() {
    if (!update) return;
    sessionStorage.setItem(UPDATE_DISMISS_KEY, update.latest);
    update = null;
  }

  async function reload() {
    // Load presets + settings under a guard. These two initialise to `[]`
    // and `{}`, so a rejection here used to leave the editor holding
    // empty placeholders that LOOKED like real state: the next settings
    // toggle serialised `{}` back to disk and reset every preference,
    // and a save would have written an empty preset list. Nothing may be
    // written until we have actually read what's on disk.
    try {
      presets = await api.listPresets();
      settings = await api.getSettings();
      lastSavedJson = JSON.stringify(presets);
      loaded = true;
      saveError = null;
    } catch (err) {
      loaded = false;
      saveError =
        `Couldn't read your presets and settings: ${err}. ` +
        `Saving is disabled so nothing gets overwritten — restart Offspring to retry.`;
      return;
    }
    ensureTools(settings);
    ffmpeg = await api.ffmpegStatus();
    // Refresh the build-variant marker once per reload. Cheap (no
    // network, just a constant lookup in Rust) and tolerant of older
    // builds without this command — fall back to "standard".
    try { buildVariant = await api.getBuildVariant(); } catch { buildVariant = "standard"; }
    try { platform = await api.getPlatform(); } catch { platform = "windows"; }
    if (!selectedId && !selectedToolId && presets.length > 0) selectedId = presets[0].id;
    // First-run guidance: if FFmpeg is missing on app open, open Settings
    // in the editor pane so the big "Download FFmpeg" button is the first
    // thing they see instead of a silently-broken app.
    if (!ffmpeg.found) showSettings = true;
    // macOS one-time Services-onboarding hint. We don't queue behind
    // the FFmpeg-missing prompt — that one just routes to a tab, no
    // modal is shown — so showing the hint immediately is fine.
    if (platform === "macos" && !settings.seen_macos_services_hint) {
      showMacosServicesHint();
    }
  }

  // ---- Preset auto-save -------------------------------------------------
  //
  // Presets save themselves ~800ms after edits stop, the same way the
  // tool settings always have. This replaced the manual "Save and Sync"
  // button: forgetting to press it was the #1 way to lose an edit.
  //
  // Two tiers, because the original one-tier version was unusable: the
  // full save command rewrites registry entries + SendTo shortcuts, and
  // running that after every typing pause made editing feel broken.
  //   1. The debounced auto-save writes ONLY presets.json (cheap).
  //   2. The shell-integration sync trails on its own longer debounce,
  //      and is flushed on window close so shortcuts never stay stale.
  //
  // `lastSavedJson` is the serialized state most recently persisted;
  // `lastAttemptedJson` stops a failed save from retry-looping every
  // debounce tick (a new attempt fires only when the user edits again).
  // Comparing JSON snapshots also cannot drift as preset fields are
  // added: JSON.stringify reads every property, which is also what
  // subscribes the effect to all of them.
  let lastSavedJson = "";
  let lastAttemptedJson = "";
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let syncTimer: ReturnType<typeof setTimeout> | undefined;
  // True from a successful JSON save until the trailing integration
  // sync has actually run — the close handler flushes it.
  let syncPending = false;

  $effect(() => {
    const json = JSON.stringify(presets);
    if (!loaded) return;
    if (json === lastSavedJson || json === lastAttemptedJson) return;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void savePresetsNow(), 800);
    return () => clearTimeout(saveTimer);
  });

  async function savePresetsNow() {
    if (!loaded) return;
    const json = JSON.stringify(presets);
    lastAttemptedJson = json;
    saving = true;
    saveError = null;
    try {
      await api.savePresetsJson(presets);
      lastSavedJson = json;
      savedTick++;
      scheduleIntegrationSync();
    } catch (err) {
      // A preset the backend refuses — a negative width, a zero fps, a
      // CRF out of range — surfaces here instead of only in the console.
      saveError = String(err);
    } finally {
      saving = false;
    }
  }

  /// Trailing shell-integration sync: rewrite the right-click / SendTo
  /// entries once the user has stopped editing for a while. Re-scheduled
  /// on every successful save, so a burst of edits costs one sync.
  function scheduleIntegrationSync() {
    syncPending = true;
    clearTimeout(syncTimer);
    syncTimer = setTimeout(async () => {
      try {
        await api.syncIntegrations();
        syncPending = false;
      } catch (err) {
        // Leave syncPending set so the close-time flush retries.
        saveError = String(err);
      }
    }, 2500);
  }

  async function saveSettings() {
    if (!loaded) {
      saveError = "Can't save — settings were never loaded successfully.";
      return;
    }
    saveError = null;
    try {
      await api.saveSettings(settings);
      ffmpeg = await api.ffmpegStatus();
    } catch (err) {
      saveError = String(err);
    }
  }

  function genId(name: string): string {
    const base = name.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_|_$/g, "");
    let id = base || "preset";
    let n = 1;
    while (presets.some((p) => p.id === id)) {
      n++;
      id = `${base}_${n}`;
    }
    return id;
  }

  function addPreset() {
    const fresh: Preset = {
      id: genId("new_preset"),
      name: "New preset",
      enabled: true,
      format: "gif",
      suffix: "_new",
      width: 500,
      height: null,
      fps: 24,
      crop: null,
      palette_colors: 128,
      dither: "bayer",
      bayer_scale: 3,
      loop_mode: "forever",
      crf: 23,
      preset_speed: "medium",
      video_bitrate: null,
      audio_bitrate: "128k",
      use_cuda: false,
      target_max_mb: null,
      icon: null,
      order: presets.length,
    };
    presets = [...presets, fresh];
    selectedId = fresh.id;
    selectedToolId = null;
  }

  function duplicatePreset(p: Preset) {
    const copy: Preset = {
      ...p,
      id: genId(p.name + " copy"),
      name: p.name + " copy",
      order: presets.length,
    };
    presets = [...presets, copy];
    selectedId = copy.id;
    selectedToolId = null;
  }

  function deletePreset(p: Preset) {
    confirmDialog = {
      title: `Delete "${p.name}"?`,
      message: isMac
        ? "The preset is removed from the right-click menu as well. This can't be undone."
        : "This also removes its right-click and SendTo entries. This can't be undone.",
      confirmLabel: "Delete preset",
      onConfirm: () => {
        presets = presets.filter((x) => x.id !== p.id);
        if (selectedId === p.id) selectedId = presets[0]?.id ?? null;
      },
    };
  }

  function onDragStart(e: DragEvent, p: Preset) {
    dragId = p.id;
    // Firefox won't start a drag without data on the transfer. The payload
    // itself is unused — we key off `dragId` in component state, which
    // survives the serialization restrictions dataTransfer imposes during
    // the drag (only type strings are readable until drop).
    e.dataTransfer?.setData("text/plain", p.id);
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }

  // WebView2 requires preventDefault on BOTH dragenter AND dragover for the
  // element to register as a valid drop target. Skipping dragenter leaves
  // the cursor stuck in the "forbidden" state even while over a child row.
  function onDragEnter(e: DragEvent) {
    if (!dragId) return;
    e.preventDefault();
  }

  function onDragOver(e: DragEvent, p: Preset) {
    if (!dragId) return;
    // Always preventDefault while a drag is active — including over the
    // source row — so the browser shows the "move" cursor instead of
    // "forbidden". Dropping on the source is a no-op (handled in onDrop)
    // but the user shouldn't be punished with a scary cursor for moving
    // over their own row on the way somewhere else.
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    if (dragId === p.id) {
      // Clear any prior indicator so we don't draw a drop line on the
      // source row itself.
      if (dragOver) dragOver = null;
      return;
    }
    // Above / below split at the row's vertical midpoint so the insertion
    // point feels natural as the cursor moves past an item.
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const pos: "above" | "below" = e.clientY < rect.top + rect.height / 2 ? "above" : "below";
    if (!dragOver || dragOver.id !== p.id || dragOver.pos !== pos) {
      dragOver = { id: p.id, pos };
    }
  }

  function onDrop(e: DragEvent, target: Preset) {
    e.preventDefault();
    const src = dragId;
    const over = dragOver;
    dragId = null;
    dragOver = null;
    if (!src || !over || src === target.id) return;
    const from = presets.findIndex((x) => x.id === src);
    if (from < 0) return;
    const copy = [...presets];
    const [moved] = copy.splice(from, 1);
    // Re-derive the insertion index against the spliced array, since
    // removing an earlier element shifts everything after it.
    let insertBefore = copy.findIndex((x) => x.id === target.id);
    if (over.pos === "below") insertBefore += 1;
    copy.splice(insertBefore, 0, moved);
    copy.forEach((x, k) => (x.order = k));
    presets = copy;
  }

  function onDragEnd() {
    dragId = null;
    dragOver = null;
  }

  // macOS-only Services-onboarding hint. Surfaces the System Settings
  // → Keyboard → Services pane via a deep-link URL because macOS hides
  // newly-discovered service providers by default, so without this
  // nudge the right-click "Offspring…" entry never appears even though
  // our NSServices declaration is correct.
  //
  // State machine:
  //   closed  — not visible
  //   idle    — initial: explanation + "Open Services Settings" / "I'll do it later"
  //   waiting — after Open Settings was clicked: keep dialog open, poll
  //             for the user to actually tick the box; auto-dismiss the
  //             moment the check returns true so the user gets visible
  //             confirmation that what they did worked. Without this we
  //             closed the dialog and the user had no anchor for the
  //             multi-step path inside System Settings.
  //
  // Effect lower in the file polls `is_macos_service_enabled` every 2s
  // and on every window-focus event while in the waiting phase. The
  // poll is cheap (single `defaults read` call) and stops as soon as
  // the dialog leaves the waiting phase.
  //
  // Deliberately NOT folded into ConfirmDialog: the waiting phase stays
  // open across button presses and closes itself on an async signal,
  // which the one-shot confirm contract can't express.
  type MacosHintState =
    | { phase: "closed" }
    | { phase: "idle" }
    | { phase: "waiting" };
  let macosHint = $state<MacosHintState>({ phase: "closed" });

  function showMacosServicesHint() {
    macosHint = { phase: "idle" };
  }

  async function macosHintOpenSettings() {
    try {
      await api.openExternalUrl(
        "x-apple.systempreferences:com.apple.preference.keyboard?Services",
      );
    } catch (err) {
      console.warn("open services pane failed:", err);
    }
    // Don't dismiss — stay open so the user has the instructions while
    // they're inside System Settings. Auto-detect kicks in via the
    // effect below.
    macosHint = { phase: "waiting" };
    await checkMacosServiceEnabled();
  }

  // One-shot probe. The waiting-phase effect calls this on focus and
  // on a 2s timer; the user can also trigger it with the "Check now"
  // button. When the service flips to enabled we close the dialog and
  // persist the seen flag — that's the user's "it worked" signal.
  async function checkMacosServiceEnabled() {
    try {
      const enabled = await api.isMacosServiceEnabled();
      if (enabled) {
        macosHint = { phase: "closed" };
        await dismissMacosServicesHint();
      }
    } catch (err) {
      console.warn("is_macos_service_enabled failed:", err);
    }
  }

  // User explicitly bailed out of the dialog without enabling. Persist
  // the seen flag so we don't nag again on next launch — but they can
  // still re-open it from Settings → Finder integration.
  function macosHintDismiss() {
    macosHint = { phase: "closed" };
    void dismissMacosServicesHint();
  }

  // Persist the "user has seen the hint" flag so the dialog doesn't
  // re-fire on next launch. Re-openable from Settings → Finder
  // integration via the same showMacosServicesHint() entry point.
  async function dismissMacosServicesHint() {
    if (settings.seen_macos_services_hint) return;
    settings.seen_macos_services_hint = true;
    try { await api.saveSettings(settings); } catch (err) {
      console.warn("save settings (macos hint flag) failed:", err);
    }
  }

  // While the hint is in its "waiting" phase (user clicked Open
  // Settings, we're waiting for them to tick the box), poll for the
  // service-enabled state. Two triggers:
  //   1. Every window-focus event (cheap, fires when the user comes
  //      back from System Settings).
  //   2. A 2s timer as a fallback in case focus events miss (e.g.
  //      System Settings was already in front).
  // Both stop the moment phase leaves "waiting", whether because we
  // detected the enable or the user dismissed.
  $effect(() => {
    if (macosHint.phase !== "waiting") return;
    let cancelled = false;
    const tick = () => { if (!cancelled) void checkMacosServiceEnabled(); };
    const id = setInterval(tick, 2000);
    const unlistenP = getCurrentWindow().onFocusChanged(({ payload }) => {
      if (payload) tick();
    });
    return () => {
      cancelled = true;
      clearInterval(id);
      void unlistenP.then((fn) => fn()).catch(() => {});
    };
  });

  function switchTab(next: "tools" | "presets") {
    tab = next;
    showSettings = false;
    if (next === "tools") {
      selectedId = null;
      if (!selectedToolId) selectedToolId = TOOLS[0]?.id ?? null;
    } else {
      selectedToolId = null;
      if (!selectedId && presets.length > 0) selectedId = presets[0]?.id ?? null;
    }
  }

  function toggleSettings() {
    showSettings = !showSettings;
  }

  function resetDefaults() {
    confirmDialog = {
      title: "Reset to defaults?",
      message:
        "All presets will be replaced with the built-in defaults. Your custom presets and edits will be permanently lost. This can't be undone.",
      confirmLabel: "Reset presets",
      onConfirm: async () => {
        presets = await api.resetPresetsToDefaults();
        // The backend just wrote this state — mark it saved so the
        // auto-save effect doesn't immediately re-save it.
        lastSavedJson = JSON.stringify(presets);
        selectedId = presets[0]?.id ?? null;
      },
    };
  }
</script>

<svelte:window onclick={() => (ctxMenu = null)} />

{#if ctxMenu}
  <div
    class="ctx-menu"
    style="left: {ctxMenu.x}px; top: {ctxMenu.y}px;"
    role="menu"
    onclick={(e) => e.stopPropagation()}
  >
    <button
      type="button"
      role="menuitem"
      onclick={() => { duplicatePreset(ctxMenu!.preset); ctxMenu = null; }}
    ><Neaticon name="copy" /> Duplicate</button>
    <button
      type="button"
      role="menuitem"
      class="danger"
      onclick={() => { deletePreset(ctxMenu!.preset); ctxMenu = null; }}
    ><Neaticon name="trash" /> Delete</button>
  </div>
{/if}

{#if confirmDialog}
  <ConfirmDialog spec={confirmDialog} onclose={() => (confirmDialog = null)} />
{/if}

<!-- macOS Services-onboarding hint. State machine in script:
       idle    — initial copy + two buttons
       waiting — kept open after Open Settings click; auto-detects when
                 the user actually ticks the box, then closes itself. -->
{#if macosHint.phase !== "closed"}
  <div
    class="modal-backdrop"
    role="presentation"
    onclick={macosHintDismiss}
  >
    <div
      class="modal"
      role="alertdialog"
      aria-labelledby="macos-hint-title"
      aria-describedby="macos-hint-message"
      onclick={(e) => e.stopPropagation()}
    >
      {#if macosHint.phase === "idle"}
        <h3 id="macos-hint-title" class="modal-title">One more step on macOS</h3>
        <p id="macos-hint-message" class="modal-message">
          To send files to Offspring from Finder's right-click → Services
          menu, enable <strong>"Offspring…"</strong> in System Settings →
          Keyboard → Keyboard Shortcuts → Services → Files and Folders.
          macOS hides newly-installed Services entries by default.
        </p>
        <div class="modal-actions">
          <button class="ghost" onclick={macosHintDismiss}>
            I'll do it later
          </button>
          <button class="primary" onclick={macosHintOpenSettings}>
            Open Services Settings
          </button>
        </div>
      {:else if macosHint.phase === "waiting"}
        <h3 id="macos-hint-title" class="modal-title">Waiting for you to enable Offspring…</h3>
        <p id="macos-hint-message" class="modal-message">
          In the System Settings window that just opened, scroll to
          <strong>Files and Folders</strong> and tick the box next to
          <strong>"Offspring…"</strong>. This dialog will close
          automatically once it's enabled.
        </p>
        <div class="modal-actions">
          <button class="ghost" onclick={macosHintDismiss}>
            Close
          </button>
          <button class="primary" onclick={checkMacosServiceEnabled}>
            Check now
          </button>
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if update && update.update_available}
  <UpdateBanner {update} {upd} {isMac} onaction={onUpdateClick} ondismiss={dismissUpdate} />
{/if}

<main class="shell">
  <!-- The topbar doubles as the window titlebar on frameless (Windows)
       builds: empty space drags the window, double-click maximises, and
       the pill on the right carries minimize/maximize/close. -->
  <header class="topbar" class:frameless use:windowDrag={frameless}>
    <div class="brand">
      <img class="brand-mark" src="/favicon.png" alt="" draggable="false" />
      <div class="brand-text">
        <h1>Offspring</h1>
        <span class="tiny">Right-click tools powered by FFmpeg<br>Developed by <span
            class="brand-link"
            role="link"
            tabindex="0"
            onclick={() => api.openExternalUrl("https://secondmarch.xyz/").catch((e) => console.error("openExternalUrl failed", e))}
            onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); api.openExternalUrl("https://secondmarch.xyz/").catch((err) => console.error("openExternalUrl failed", err)); } }}
          >Second March</span></span>
      </div>
    </div>

    <nav class="tabs">
      <button class={tab === "tools" ? "tab active" : "tab"} onclick={() => switchTab("tools")}>Tools</button>
      <button class={tab === "presets" ? "tab active" : "tab"} onclick={() => switchTab("presets")}>Presets</button>
    </nav>

    <div class="topbar-right">
      <span class="badge {ffmpeg.found ? 'ok' : 'warn'}" title={ffmpeg.path ?? ''}>
        <span class="dot {ffmpeg.found ? 'ok' : 'warn'}"></span>
        FFmpeg {ffmpeg.found ? "ready" : "missing"}
      </span>
      {#if saveError}
        <span class="tiny save-error" title={saveError}>{saveError}</span>
      {:else if saving}
        <span class="tiny saved">Saving…</span>
      {:else if savedTick > 0}
        <span class="tiny saved"><Neaticon name="check" /> Saved</span>
      {/if}
      {#if frameless}
        <WindowControls />
      {/if}
    </div>
  </header>

  <section class="panes">
    <aside class="sidebar">
      {#if tab === "tools"}
        <div class="sidebar-head">
          <span class="tiny">TOOLS</span>
        </div>
        <ul class="tool-list fill">
          {#each TOOLS as t (t.id)}
            <li
              class="row-item tool-row"
              class:active={selectedToolId === t.id}
              onclick={() => { selectedToolId = t.id; selectedId = null; showSettings = false; }}
              onkeydown={(e) => e.key === "Enter" && ((selectedToolId = t.id), (selectedId = null), (showSettings = false))}
              role="button"
              tabindex="0"
            >
              <input
                type="checkbox"
                checked={settings.tools?.[t.id]?.enabled ?? (t.id === "overlay" ? false : true)}
                onclick={(e) => e.stopPropagation()}
                onchange={(e) => {
                  ensureTools(settings);
                  const v = (e.currentTarget as HTMLInputElement).checked;
                  settings.tools![t.id].enabled = v;
                  saveSettings();
                }}
                title="Enable tool"
              />
              <span class="tool-name">{t.name}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <div class="sidebar-head">
          <span class="tiny">PRESETS</span>
          <button class="ghost" onclick={addPreset} title="Add preset"><Neaticon name="plus" /> Add</button>
        </div>
        <ul class="preset-list">
          {#each presets as p (p.id)}
            <li
              class="row-item"
              class:active={selectedId === p.id}
              class:dragging={dragId === p.id}
              class:drop-above={dragOver?.id === p.id && dragOver?.pos === "above"}
              class:drop-below={dragOver?.id === p.id && dragOver?.pos === "below"}
              draggable="true"
              ondragstart={(e) => onDragStart(e, p)}
              ondragenter={onDragEnter}
              ondragover={(e) => onDragOver(e, p)}
              ondrop={(e) => onDrop(e, p)}
              ondragend={onDragEnd}
              onclick={() => { selectedId = p.id; selectedToolId = null; showSettings = false; }}
              oncontextmenu={(e) => {
                e.preventDefault();
                selectedId = p.id;
                selectedToolId = null;
                showSettings = false;
                ctxMenu = { x: e.clientX, y: e.clientY, preset: p };
              }}
              onkeydown={(e) => e.key === "Enter" && ((selectedId = p.id), (selectedToolId = null), (showSettings = false))}
              role="button"
              tabindex="0"
            >
              <span class="grip" aria-hidden="true" title="Drag to reorder">≡</span>
              <input
                type="checkbox"
                checked={p.enabled}
                onclick={(e) => e.stopPropagation()}
                onchange={(e) => {
                  p.enabled = (e.currentTarget as HTMLInputElement).checked;
                }}
                title={isMac ? "Enable this preset" : "Show in right-click menu"}
              />
              <span class="fmt-tag {p.format}">{p.format.toUpperCase()}</span>
              <span class="preset-name">{p.name}</span>
            </li>
          {/each}
        </ul>
      {/if}

      <div class="sidebar-foot">
        <button class="ghost settings-btn" class:active={showSettings} onclick={toggleSettings}>
          <Neaticon name="gear-setting" />
          Settings
        </button>
      </div>
    </aside>

    <section class="editor">
      {#if showSettings}
        <SettingsPane
          {settings}
          {ffmpeg}
          {dl}
          {isMac}
          {isStudio}
          {currentVersion}
          {updateCheck}
          {update}
          onSaveSettings={saveSettings}
          onDownloadFfmpeg={startDownloadFfmpeg}
          onCheckUpdate={() => checkUpdate({ manual: true })}
          onResetDefaults={resetDefaults}
          onShowMacosHint={showMacosServicesHint}
          {showDialog}
        />
      {:else if tab === "tools" && selectedTool}
        <ToolPane tool={selectedTool} {settings} onSaveSettings={saveSettings} {showDialog} />
      {:else if tab === "presets" && selected}
        <div class="editor-head">
          <input
            class="title-input"
            type="text"
            bind:value={selected.name}
            placeholder="Preset name"
          />
          <div class="row">
            <button class="ghost" onclick={() => duplicatePreset(selected!)}><Neaticon name="copy" /> Duplicate</button>
            <button class="danger" onclick={() => deletePreset(selected!)}><Neaticon name="trash" /> Delete</button>
          </div>
        </div>
        {#if !isMac}
          <p class="muted tiny">Shortcut appears in right-click → Send To as <code>Offspring - {selected.name}.lnk</code></p>
        {/if}

        <div class="fields">
          <FormatFields preset={selected} />
        </div>
      {:else}
        <div class="empty">
          {#if tab === "tools"}
            <h2>No tool selected</h2>
            <p class="muted">Pick one from the sidebar.</p>
          {:else}
            <h2>No preset selected</h2>
            <p class="muted">Pick one from the sidebar or add a new one.</p>
          {/if}
        </div>
      {/if}
    </section>
  </section>
</main>

<style>
  .shell { display: flex; flex-direction: column; height: 100vh; }

  .topbar {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    padding: 8px 16px;
    border-bottom: 1px solid var(--c-border);
    background: var(--c-surface);
    /* Chrome isn't content: dragging or double-clicking the bar
       shouldn't leave a stray text selection. */
    -webkit-user-select: none;
    user-select: none;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .brand-mark {
    width: 34px;
    height: 34px;
    border-radius: 8px;
    object-fit: cover;
    flex: 0 0 auto;
  }
  .brand-text { display: flex; flex-direction: column; gap: 0; }
  .brand h1 { font-size: var(--fs-20); line-height: 1.1; }
  /* Inline "Second March" credit. Reads as plain text — only the cursor
     and hover-fade hint that it's interactive. We use a <span role="link">
     rather than <a href> or <button> because both of those drag UA chrome
     (anchor underline+colour, button background+border) that the user
     specifically didn't want. The span has zero defaults to fight. */
  .brand-link {
    cursor: pointer;
    transition: opacity var(--dur-base) ease;
    /* Outline only on keyboard focus so mouse users see no chrome. */
    outline: none;
  }
  .brand-link:hover,
  .brand-link:focus-visible { opacity: 0.6; }
  .brand-link:focus-visible { outline: 1px dotted currentColor; outline-offset: 2px; }
  .tabs {
    display: flex;
    gap: 2px;
    background: var(--c-surface-3);
    padding: 2px;
    border-radius: var(--r-md);
  }
  .tab {
    background: transparent;
    border: none;
    color: var(--c-text-3);
    padding: 4px 12px;
    min-height: 0;
    border-radius: var(--r-sm);
    font-size: var(--fs-14);
    transition: color var(--dur-base) ease, background var(--dur-base) ease;
  }
  .tab:hover { background: transparent; color: var(--c-text); }
  .tab.active {
    background: var(--c-surface);
    color: var(--c-text);
    box-shadow: var(--shadow-whisper);
  }
  .topbar-right { justify-self: end; display: flex; align-items: center; gap: 8px; }
  .saved {
    color: var(--c-text-3);
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  /* Save failures used to be invisible outside the console. Kept on one
     line with the full text in the tooltip so a long backend message
     can't push the window controls out of the header. */
  .save-error {
    color: var(--c-danger);
    max-width: 40ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .panes { display: grid; grid-template-columns: 260px 1fr; flex: 1; min-height: 0; }

  .sidebar {
    border-right: 1px solid var(--c-border);
    display: flex;
    flex-direction: column;
    background: var(--c-surface-2);
    min-height: 0;
  }
  .sidebar-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 6px 10px 4px;
    letter-spacing: 0.08em;
    color: var(--c-text-3);
  }
  .sidebar-head button {
    min-height: 0; padding: 2px 8px; font-size: var(--fs-12);
    display: inline-flex; align-items: center; gap: 4px;
  }
  .preset-list {
    list-style: none;
    padding: 2px 6px;
    margin: 0;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
  }
  .row-item {
    display: flex; align-items: center; gap: 6px;
    padding: 4px 6px;
    border-radius: var(--r-sm);
    cursor: pointer;
    transition: background var(--dur-fast) ease;
    font-size: var(--fs-13, 13px);
  }
  .ctx-menu {
    position: fixed;
    z-index: 1000;
    min-width: 140px;
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: var(--c-surface);
    border: 1px solid var(--c-border);
    border-radius: var(--r-sm);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.2);
    animation: ctx-pop var(--dur-fast) var(--ease-out-strong);
  }
  @keyframes ctx-pop {
    from { opacity: 0; transform: scale(0.97); }
    to   { opacity: 1; transform: scale(1); }
  }
  .ctx-menu button {
    all: unset;
    padding: 6px 10px;
    border-radius: var(--r-sm);
    font-size: var(--fs-13, 13px);
    cursor: pointer;
    color: var(--c-text);
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ctx-menu button:hover { background: var(--c-surface-2); }
  .ctx-menu button.danger { color: var(--c-danger, #b91c1c); }
  .ctx-menu button.danger:hover { background: var(--c-danger-tint, rgba(185, 28, 28, 0.12)); }

  /* macOS Services hint modal — matches ConfirmDialog's visual language
     but stays bespoke (see the state-machine comment in the script). */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 2000;
    background: rgba(0, 0, 0, 0.42);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    animation: modal-fade var(--dur-fast) ease-out;
  }
  .modal {
    background: var(--c-surface);
    color: var(--c-text);
    border: 1px solid var(--c-border);
    border-radius: var(--r-md, 8px);
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.28);
    padding: 22px 22px 18px;
    max-width: 420px;
    width: 100%;
    animation: modal-pop var(--dur-base) var(--ease-out-strong);
  }
  .modal-title {
    margin: 0 0 8px;
    font-size: var(--fs-18, 18px);
    font-weight: 500;
  }
  .modal-message {
    margin: 0 0 18px;
    color: var(--c-text-2);
    font-size: var(--fs-14, 14px);
    line-height: 1.5;
  }
  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  @keyframes modal-fade {
    from { opacity: 0; }
    to   { opacity: 1; }
  }
  @keyframes modal-pop {
    from { opacity: 0; transform: translateY(4px) scale(0.97); }
    to   { opacity: 1; transform: translateY(0) scale(1); }
  }

  .row-item:hover { background: var(--c-surface); }
  .row-item.active {
    background: var(--c-surface);
    box-shadow: var(--shadow-ring);
  }
  .fmt-tag {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    padding: 1px 5px;
    border-radius: 3px;
    background: var(--c-surface-3);
    color: var(--c-text-2);
    flex: 0 0 auto;
  }
  .fmt-tag.gif { background: #FEF3C7; color: #92400E; }
  .fmt-tag.mp4 { background: var(--c-primary-tint); color: #0D47A1; }
  .fmt-tag.prores { background: #EDE9FE; color: #5B21B6; }
  .preset-name {
    flex: 1;
    font-size: var(--fs-14);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .grip {
    flex: 0 0 auto;
    color: var(--c-text-3);
    font-size: 14px;
    line-height: 1;
    padding: 0 2px;
    cursor: grab;
    user-select: none;
    opacity: 0.4;
    transition: opacity var(--dur-fast) ease;
  }
  .row-item:hover .grip,
  .row-item.active .grip { opacity: 1; }
  .row-item.dragging {
    opacity: 0.4;
  }
  .row-item.drop-above {
    box-shadow: inset 0 2px 0 0 var(--c-primary);
  }
  .row-item.drop-below {
    box-shadow: inset 0 -2px 0 0 var(--c-primary);
  }
  .row-item[draggable="true"] { cursor: pointer; }
  .row-item[draggable="true"]:active .grip { cursor: grabbing; }

  .tool-list {
    list-style: none;
    padding: 2px 6px;
    margin: 0;
    flex: 0 0 auto;
  }
  .tool-list.fill {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .tool-row { padding: 6px; }
  .tool-name {
    flex: 1;
    font-size: var(--fs-14);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sidebar-foot {
    padding: 8px;
    border-top: 1px solid var(--c-border);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .settings-btn {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: flex-start;
    gap: 8px;
    font-size: var(--fs-14);
    padding: 8px 10px;
  }
  .settings-btn :global(.neaticon) {
    width: 16px;
    height: 16px;
  }
  .sidebar-foot button.active {
    color: var(--c-text);
    background: var(--c-surface);
    box-shadow: var(--shadow-ring);
  }

  .editor {
    padding: 12px 18px 16px;
    overflow-y: auto;
    background: var(--c-surface);
    min-height: 0;
  }
  .editor-head {
    display: flex; justify-content: space-between; align-items: center;
    gap: 12px; margin-bottom: 2px;
  }
  .title-input {
    font-family: var(--font-display);
    font-size: var(--fs-20);
    font-weight: 600;
    border: 1px solid transparent;
    background: transparent;
    padding: 2px 6px;
    min-height: 0;
    border-radius: var(--r-md);
    color: var(--c-text);
  }
  .title-input:hover { background: var(--c-surface-2); border-color: var(--c-border); }
  .title-input:focus {
    background: var(--c-surface);
    border-color: var(--c-primary);
    box-shadow: 0 0 0 3px var(--c-primary-ring);
  }
  .editor-head button {
    font-size: var(--fs-12); padding: 4px 10px; min-height: 0;
    display: inline-flex; align-items: center; gap: 5px;
  }
  .fields { margin-top: 10px; }
  .empty { text-align: center; padding: 40px 20px; color: var(--c-text-3); }
</style>
