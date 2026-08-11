<script lang="ts">
  import type { UpdateInfo } from "$lib/types";

  export interface UpdateDownloadState {
    phase: "idle" | "downloading" | "ready" | "error";
    percent: number | null;
    message: string | null;
  }

  let {
    update,
    upd,
    isMac,
    onaction,
    ondismiss,
  }: {
    update: UpdateInfo;
    upd: UpdateDownloadState;
    isMac: boolean;
    onaction: () => void;
    ondismiss: () => void;
  } = $props();
</script>

<aside class="update-banner" role="status">
  <span class="update-icon" aria-hidden="true">⬆</span>
  <span class="update-text">
    {#if upd.phase === "downloading"}
      Downloading <strong>{update.latest}</strong>{upd.percent != null ? ` — ${Math.round(upd.percent)}%` : "…"}
    {:else if upd.phase === "ready"}
      Version <strong>{update.latest}</strong> is downloaded and
      {isMac ? "ready to open." : "ready to install."}
    {:else if upd.phase === "error"}
      Update <strong>{update.latest}</strong> couldn't download automatically.
    {:else}
      Version <strong>{update.latest}</strong> is available (you have {update.current}).
    {/if}
  </span>
  {#if upd.phase === "downloading"}
    <div class="update-bar" aria-hidden="true">
      <div
        class="update-bar-fill"
        class:indet={upd.percent == null}
        style={upd.percent != null ? `width: ${Math.round(upd.percent)}%;` : ""}
      ></div>
    </div>
  {:else}
    <!-- No `disabled` guard needed: this branch only renders when the
         phase is NOT "downloading" (the progress bar takes over then). -->
    <button type="button" class="update-btn" onclick={onaction}>
      {#if upd.phase === "ready"}
        <!-- macOS gets a .dmg, not a silent installer: we mount it and
             quit so the user can drag the new bundle over the running
             one. "Restart and install" would be a promise we don't keep. -->
        {isMac ? "Quit and open installer" : "Restart and install"}
      {:else if upd.phase === "error"}
        Open download page
      {:else}
        Download
      {/if}
    </button>
  {/if}
  <button
    type="button"
    class="update-close"
    aria-label="Dismiss update notice"
    onclick={ondismiss}
  >×</button>
</aside>

<style>
  .update-banner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 16px;
    background: var(--c-accent, #3b82f6);
    color: #fff;
    font-size: var(--fs-14, 14px);
    border-bottom: 1px solid rgba(0, 0, 0, 0.15);
  }
  .update-icon {
    font-weight: bold;
    opacity: 0.9;
  }
  .update-text {
    flex: 1;
  }
  .update-text strong {
    font-weight: 600;
  }
  .update-btn {
    background: rgba(255, 255, 255, 0.22);
    color: #fff;
    border: 1px solid rgba(255, 255, 255, 0.35);
    padding: 4px 12px;
    border-radius: var(--r-sm, 6px);
    font-weight: 500;
    cursor: pointer;
  }
  .update-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.32);
  }
  .update-btn:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .update-bar {
    width: 140px;
    height: 6px;
    background: rgba(255, 255, 255, 0.25);
    border-radius: var(--r-pill, 999px);
    overflow: hidden;
  }
  .update-bar-fill {
    height: 100%;
    background: #fff;
    transition: width 200ms ease;
  }
  .update-bar-fill.indet {
    width: 40%;
    animation: update-slide 1.2s ease-in-out infinite;
  }
  @keyframes update-slide {
    0%   { transform: translateX(-120%); }
    100% { transform: translateX(260%); }
  }
  .update-close {
    background: transparent;
    color: #fff;
    border: none;
    font-size: 18px;
    line-height: 1;
    padding: 2px 6px;
    cursor: pointer;
    opacity: 0.8;
  }
  .update-close:hover {
    opacity: 1;
  }
</style>
