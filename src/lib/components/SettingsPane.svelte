<script lang="ts">
  import * as api from "$lib/api";
  import type { Settings, FfmpegStatus, UpdateInfo } from "$lib/types";
  import type { ConfirmSpec } from "./ConfirmDialog.svelte";
  import Neaticon from "./Neaticon.svelte";

  export interface FfmpegDlState {
    active: boolean;
    phase: string;
    percent: number | null;
    message: string | null;
    error: string | null;
  }

  export interface UpdateCheckState {
    checking: boolean;
    lastChecked: number | null;
    manualResult: string | null;
  }

  let {
    settings,
    ffmpeg,
    dl,
    isMac,
    isStudio,
    currentVersion,
    updateCheck,
    update,
    onSaveSettings,
    onDownloadFfmpeg,
    onCheckUpdate,
    onResetDefaults,
    onShowMacosHint,
    showDialog,
  }: {
    /** The page's $state proxy — fields are mutated in place, then
     *  persisted via onSaveSettings, matching FormatFields' pattern. */
    settings: Settings;
    ffmpeg: FfmpegStatus;
    dl: FfmpegDlState;
    isMac: boolean;
    isStudio: boolean;
    currentVersion: string;
    updateCheck: UpdateCheckState;
    update: UpdateInfo | null;
    onSaveSettings: () => Promise<void>;
    onDownloadFfmpeg: () => void;
    onCheckUpdate: () => void;
    onResetDefaults: () => void;
    onShowMacosHint: () => void;
    showDialog: (spec: ConfirmSpec) => void;
  } = $props();

  /** alert() replacement — same visual language as every other dialog. */
  function notify(title: string, message: string) {
    showDialog({
      title,
      message,
      confirmLabel: "OK",
      confirmClass: "primary",
      hideCancel: true,
      onConfirm: () => {},
    });
  }

  async function restartExplorer() {
    try {
      await api.restartExplorer();
    } catch (err) {
      notify("Couldn't restart Explorer", String(err));
    }
  }

  async function reinstallModernMenu() {
    try {
      await api.setupModernMenu();
      settings.modern_menu_enabled = true;
      try {
        Object.assign(settings, await api.getSettings());
      } catch {}
      showDialog({
        title: "Modern menu reinstalled",
        message:
          "Windows Explorer will restart briefly so the new right-click entries appear.",
        confirmLabel: "Restart Explorer",
        confirmClass: "primary",
        hideCancel: true,
        onConfirm: restartExplorer,
      });
    } catch (err) {
      // A failed registration leaves the classic menu untouched
      // (setup_modern_menu only commits the setting after
      // Add-AppxPackage succeeds), so say so — otherwise this reads as
      // "the app is broken" when the user in fact still has a working
      // right-click menu one click away under "Show more options".
      notify(
        "Couldn't set up the Windows 11 top-level menu",
        String(err) +
          "\n\nOffspring itself is fine: right-click a file and choose " +
          '"Show more options" (or press Shift+F10) to use it from the classic menu.',
      );
    }
  }
</script>

<div class="settings-pane">
  <div class="card">
    <h3>FFmpeg</h3>
    <p class="muted tiny">Leave path blank to use the bundled/managed FFmpeg, or point to your own install.</p>
    <div class="row card-row">
      <input
        type="text"
        value={settings.ffmpeg_path ?? ""}
        oninput={(e) => {
          const v = (e.currentTarget as HTMLInputElement).value;
          settings.ffmpeg_path = v === "" ? null : v;
        }}
        placeholder="(default location)"
      />
      <button onclick={onSaveSettings}>Save</button>
    </div>
    <p class="tiny card-gap-sm">
      Status: <span class="badge {ffmpeg.found ? 'ok' : 'warn'}">
        {ffmpeg.found ? ffmpeg.path : "not found"}
      </span>
    </p>
    {#if !ffmpeg.found && ffmpeg.error}
      <!-- Surface the exact resolution failure inline. Most useful when
           the user has set a custom path that's invalid: instead of just
           "not found", they see "isn't named ffmpeg.exe" or "doesn't
           point at a file" with the path echoed back — usually enough
           to spot the typo. -->
      <p class="tiny warn-line">{ffmpeg.error}</p>
    {/if}

    {#if !ffmpeg.found && isStudio}
      <div class="dl-box">
        <p class="tiny muted">
          <strong>Offspring Studio</strong> doesn't download FFmpeg automatically.
          Grab a <code>win64-gpl</code> build from
          <a href="https://github.com/BtbN/FFmpeg-Builds/releases" target="_blank" rel="noreferrer">BtbN/FFmpeg-Builds</a>,
          extract it, and point the path above at <code>ffmpeg.exe</code>.
        </p>
      </div>
    {:else if !ffmpeg.found && !dl.active && dl.phase !== "done"}
      <div class="dl-box">
        {#if isMac}
          <p class="tiny muted">
            No FFmpeg found. Offspring will download a universal
            static build from
            <a href="https://evermeet.cx/ffmpeg/" target="_blank" rel="noreferrer">evermeet.cx</a>
            into <code>~/Library/Application Support/Offspring/ffmpeg/</code>.
            Or set a custom path above (e.g.
            <code>/opt/homebrew/bin/ffmpeg</code> for a Homebrew install).
          </p>
        {:else}
          <p class="tiny muted">
            No FFmpeg found. Download a static GPL build (~160 MB) from
            <a href="https://github.com/BtbN/FFmpeg-Builds/releases" target="_blank" rel="noreferrer">BtbN/FFmpeg-Builds</a>
            into <code>%LOCALAPPDATA%\Offspring\ffmpeg\</code>.
          </p>
        {/if}
        <button class="primary" onclick={onDownloadFfmpeg}>
          <Neaticon name="download" /> Download FFmpeg
        </button>
        {#if dl.error}
          <p class="tiny err">✕ {dl.error}</p>
        {/if}
      </div>
    {:else if ffmpeg.found && !isStudio && !dl.active}
      <!-- FFmpeg is present, and this build can replace it. The
           downloader used to be reachable ONLY when FFmpeg was missing,
           which meant everyone who installed it once kept that copy
           forever — including the gyan.dev build shipped through 0.5.x,
           whose missing dav1d decoder makes some AV1 files unplayable. -->
      <div class="dl-box">
        {#if ffmpeg.has_dav1d === false && ffmpeg.managed}
          <p class="tiny warn-line">
            Your FFmpeg was installed by an older version of Offspring and
            can't decode some AV1 files (you'd see a "no frames could be
            read" error on those). Updating replaces it with a current
            build.
          </p>
        {:else if ffmpeg.has_dav1d === false}
          <p class="tiny warn-line">
            The FFmpeg at the path above has no dav1d decoder, so some AV1
            files won't decode. Offspring can't replace it because you've
            pointed at your own build — update that build, or clear the
            path above to use Offspring's managed copy.
          </p>
        {:else}
          <p class="tiny muted">
            Offspring manages its own FFmpeg copy. Re-download it if it
            stops working or you want the latest build.
          </p>
        {/if}
        {#if ffmpeg.managed || ffmpeg.has_dav1d !== false}
          <button
            class={ffmpeg.has_dav1d === false && ffmpeg.managed ? "primary" : ""}
            onclick={onDownloadFfmpeg}
          >
            <Neaticon name="download" />
            {ffmpeg.has_dav1d === false ? "Update FFmpeg" : "Re-download FFmpeg"}
          </button>
        {/if}
        {#if dl.error}
          <p class="tiny err">✕ {dl.error}</p>
        {/if}
      </div>
    {:else if dl.active}
      <div class="dl-box">
        <div class="row between">
          <span class="tiny muted">
            {dl.phase === "downloading" ? "Downloading FFmpeg…" :
             dl.phase === "extracting" ? "Extracting archive…" :
             dl.phase === "starting" ? "Starting…" : dl.phase}
          </span>
          <span class="tiny muted">
            {dl.percent != null ? Math.round(dl.percent) + "%" : ""}
          </span>
        </div>
        <div class="bar">
          <div
            class="fill"
            class:indet={dl.percent == null}
            style={dl.percent != null ? `width: ${Math.round(dl.percent)}%;` : ""}
          ></div>
        </div>
        {#if dl.message}
          <p class="tiny muted">{dl.message}</p>
        {/if}
      </div>
    {/if}
  </div>

  {#if isMac}
    <div class="card">
      <h3>Finder integration</h3>
      <p class="muted tiny">
        Right-click any video, image, or audio file in Finder and
        pick <strong>Services → Offspring…</strong> to send it
        here. macOS hides newly-installed Services entries by
        default — if you don't see Offspring in the Services
        submenu, enable it under System Settings → Keyboard →
        Keyboard Shortcuts → Services → Files and Folders.
      </p>
      <div class="row card-row">
        <button
          class="primary"
          onclick={async () => {
            try {
              await api.openExternalUrl(
                "x-apple.systempreferences:com.apple.preference.keyboard?Services",
              );
            } catch (err) {
              notify("Couldn't open System Settings", String(err));
            }
          }}
        >
          Open Services Settings
        </button>
        <button class="ghost" onclick={onShowMacosHint}>
          Show the hint again
        </button>
      </div>
    </div>
  {:else}
    <div class="card">
      <h3>Right-click menu</h3>
      {#if isStudio}
        <p class="muted tiny">
          <strong>Offspring Studio</strong> uses only the classic
          right-click menu (under "Show more options" on Windows 11).
          No certificate is ever installed, no MSIX package is
          registered, and the modern top-level menu is disabled in
          this build. Configure presets and the menu adjusts
          automatically.
        </p>
      {:else}
        <p class="muted tiny">
          By default, Offspring lives under Windows 11's "Show more options" (the classic right-click menu).
          Enabling the modern menu below moves it to the top-level right-click menu.
        </p>

        <div class="card-stack card-row">
          <label class="inline">
            <input
              type="checkbox"
              checked={settings.modern_menu_enabled ?? false}
              onchange={async (e) => {
                const checked = (e.currentTarget as HTMLInputElement).checked;
                settings.modern_menu_enabled = checked;
                await onSaveSettings();
                // Explorer caches the modern-menu handler list — new
                // CLSIDs only become visible once Explorer re-launches.
                // We restart unconditionally so the user gets a
                // consistent "flip toggle → menu updates" experience.
                await restartExplorer();
              }}
            />
            <span>Integrate with the <strong>Windows 11 right-click menu</strong></span>
          </label>
          <label class="inline" title="When on, the modern right-click menu shows TWO separate top-level entries — Offspring Presets and Offspring Tools — instead of one unified Offspring entry. Mirrors the classic right-click menu's split layout. Toggling registers / unregisters separate MSIX packages, so an Explorer restart fires automatically. Disabled when the modern menu integration itself is off.">
            <input
              type="checkbox"
              checked={settings.modern_menu_split_layout ?? false}
              disabled={!(settings.modern_menu_enabled ?? false)}
              onchange={async (e) => {
                settings.modern_menu_split_layout = (e.currentTarget as HTMLInputElement).checked;
                await onSaveSettings();
                // Toggling swaps which MSIX packages are registered
                // (Unified ↔ Presets+Tools); Explorer needs a restart to
                // drop its cached shell-ext list and pick up the new
                // top-level entries.
                await restartExplorer();
              }}
            />
            <span>Split modern menu into <strong>Offspring Presets</strong> + <strong>Offspring Tools</strong> top-level entries</span>
          </label>
          <p class="muted tiny warn-line">
            ⚠ Options above may briefly restart Windows Explorer
            (opened File Explorer windows close).
          </p>
        </div>
      {/if}

      <div class="card-stack card-row">
        <label class="inline" title="Adds an 'Offspring - <preset>' shortcut per enabled preset under right-click → Send to. Off by default — the right-click menu covers the same use-case and is more discoverable.">
          <input
            type="checkbox"
            checked={settings.sendto_enabled ?? false}
            onchange={async (e) => {
              settings.sendto_enabled = (e.currentTarget as HTMLInputElement).checked;
              await onSaveSettings();
              // Re-sync immediately so the SendTo folder reflects the
              // toggle without waiting for the next preset save.
              try {
                await api.syncIntegrations();
              } catch (err) {
                notify("Couldn't update the Send to menu", String(err));
              }
            }}
          />
          <span>Also mirror presets into the <strong>Send to</strong> menu</span>
        </label>
      </div>

      {#if !isStudio}
        <div class="card-divided">
          <p class="muted tiny">
            If the Windows 11 right-click menu entries aren't showing up, click below to re-register the modern-menu package for your user (no admin required).
          </p>
          <button class="card-gap" onclick={reinstallModernMenu}>
            Reinstall Windows 11 "Right-click menu" item
          </button>
        </div>
      {/if}
    </div>
  {/if}

  <div class="card">
    <h3>Updates</h3>
    <p class="muted tiny">
      Current version: <strong>{currentVersion || "…"}</strong>
      {#if isStudio}
        <span class="badge">Studio</span>
      {/if}
    </p>
    {#if isStudio}
      <p class="muted tiny card-gap-sm">
        <strong>Studio</strong> doesn't include the in-app
        updater. Check
        <a href="https://github.com/second-march/offspring/releases" target="_blank" rel="noreferrer">
          the GitHub releases page
        </a>
        manually and download a fresh Studio installer when a new
        version ships.
      </p>
    {:else}
      <div class="row card-row">
        <button onclick={onCheckUpdate} disabled={updateCheck.checking}>
          <Neaticon name="arrows-rotate-2" />
          {updateCheck.checking ? "Checking…" : "Check for updates"}
        </button>
      </div>
      {#if updateCheck.manualResult}
        <p class="tiny muted card-gap-sm">{updateCheck.manualResult}</p>
      {/if}
      {#if update?.update_available && update.release_notes}
        <!-- Release notes from the latest GitHub release body — rendered
             as plain text with preserved newlines (no markdown library;
             the GitHub-side authoring keeps the source readable as-is). -->
        <div class="release-notes">
          <div class="tiny muted release-notes-head">
            What's new in {update.latest}
          </div>
          <pre class="release-notes-body">{update.release_notes}</pre>
        </div>
      {/if}
    {/if}
  </div>

  <div class="card">
    <h3>Data folders</h3>
    <p class="muted tiny">
      {#if isMac}
        Presets & settings: <code>~/Library/Application Support/Offspring</code>.
        Logs: <code>~/Library/Application Support/Offspring/debug.log</code>
      {:else}
        Presets & settings: <code>%APPDATA%\Offspring</code>.
        Logs: <code>%LOCALAPPDATA%\Offspring\debug.log</code>
      {/if}
    </p>
    <div class="row card-row">
      <button onclick={api.openDataFolder}><Neaticon name="folder" /> Open data folder</button>
      <button
        onclick={api.openLogFolder}
        title={isMac
          ? "Reveals debug.log in Finder"
          : "Opens %LOCALAPPDATA%\\Offspring with debug.log selected"}
      ><Neaticon name="folder" /> Open log folder</button>
      {#if !isMac}
        <button onclick={api.syncIntegrations}><Neaticon name="arrows-rotate-2" /> Re-sync right-click menus</button>
      {/if}
    </div>
  </div>

  <div class="card">
    <h3>Reset</h3>
    <p class="muted tiny">
      Replace all presets with the built-in defaults. Custom presets
      and edits are permanently lost.
    </p>
    <div class="row card-row">
      <button class="danger" onclick={onResetDefaults}>
        <Neaticon name="arrow-rotate-left-2" /> Reset presets to defaults
      </button>
    </div>
  </div>
</div>

<style>
  .settings-pane {
    display: grid;
    gap: 10px;
    max-width: 640px;
    width: 100%;
  }
  .settings-pane .card {
    padding: var(--sp-3);
  }
  .settings-pane h3 {
    margin-bottom: 2px;
  }
  /* Consistent vertical rhythm inside cards — replaces the pile of
     inline style="margin-top: …" attributes this pane accumulated. */
  .card-row {
    margin-top: 12px;
  }
  .card-gap {
    margin-top: 8px;
  }
  .card-gap-sm {
    margin-top: 8px;
  }
  .card-stack {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .card-divided {
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px solid var(--c-border);
  }
  .card-divided p {
    margin: 0;
  }

  label.inline {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: var(--fs-13, 13px);
    color: var(--c-text);
    margin: 0;
    cursor: pointer;
  }
  label.inline input[type="checkbox"] {
    margin-top: 2px;
    flex-shrink: 0;
  }

  .warn-line {
    color: var(--c-warning, #d97706);
    margin-top: 6px;
    font-weight: 500;
  }

  /* FFmpeg download block — fits inside the FFmpeg settings card. */
  .dl-box {
    margin-top: 10px;
    padding: 10px 12px;
    background: var(--c-canvas-muted);
    border: 1px solid var(--c-border);
    border-radius: var(--r-lg);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .dl-box a {
    color: var(--c-primary);
    text-decoration: underline;
    text-decoration-color: var(--c-primary-ring);
    text-underline-offset: 2px;
  }
  .dl-box .row.between {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }
  .dl-box .bar {
    height: 6px;
    background: var(--c-surface-3);
    border-radius: var(--r-pill);
    overflow: hidden;
  }
  .dl-box .fill {
    height: 100%;
    background: var(--c-primary);
    transition: width 200ms ease;
  }
  .dl-box .fill.indet {
    width: 40%;
    animation: slide 1.2s ease-in-out infinite;
  }
  @keyframes slide {
    0%   { transform: translateX(-100%); }
    100% { transform: translateX(250%); }
  }
  .dl-box .err {
    color: var(--c-danger);
  }

  /* "What's new in X.Y.Z" panel under the manual-check button. Capped
     height with overflow-y so a long changelog doesn't push the rest of
     the pane off-screen. */
  .release-notes {
    margin-top: 12px;
    border-top: 1px solid var(--c-border, rgba(0, 0, 0, 0.1));
    padding-top: 10px;
  }
  .release-notes-head {
    font-weight: 600;
    margin-bottom: 6px;
  }
  .release-notes-body {
    margin: 0;
    max-height: 200px;
    overflow-y: auto;
    white-space: pre-wrap;
    word-break: break-word;
    font-family: var(--font-mono, monospace);
    font-size: var(--fs-12);
    color: var(--c-text-2);
    background: var(--c-surface-2, rgba(0, 0, 0, 0.04));
    padding: 8px 10px;
    border-radius: 4px;
    line-height: 1.4;
  }
</style>
