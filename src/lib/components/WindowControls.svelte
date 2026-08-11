<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";

  // Minimize / maximize / close for the frameless (Windows) build.
  // Same pill capsule as toqe's .winctl / plaza's .window-controls, with
  // thin line glyphs so the buttons don't read heavier than the nav.
  async function win(action: "minimize" | "toggleMaximize" | "close") {
    try {
      const w = getCurrentWindow();
      if (action === "minimize") await w.minimize();
      else if (action === "toggleMaximize") await w.toggleMaximize();
      else await w.close();
    } catch {
      // Not in a Tauri window (vite dev in a browser) — ignore.
    }
  }
</script>

<div class="winctl" aria-label="Window controls">
  <button type="button" title="Minimize" aria-label="Minimize" onclick={() => win("minimize")}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" aria-hidden="true">
      <path d="M6 12h12" />
    </svg>
  </button>
  <button type="button" title="Maximize / restore" aria-label="Maximize or restore" onclick={() => win("toggleMaximize")}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <rect x="6.25" y="6.25" width="11.5" height="11.5" rx="1.6" />
    </svg>
  </button>
  <button type="button" class="winctl-close" title="Close" aria-label="Close" onclick={() => win("close")}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" aria-hidden="true">
      <path d="M6.75 6.75l10.5 10.5M17.25 6.75l-10.5 10.5" />
    </svg>
  </button>
</div>

<style>
  .winctl {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    margin-left: 4px;
    padding: 3px;
    border-radius: var(--r-pill);
    /* Transparent so the pill reads the same tone as the bar behind it —
       just the outline defines the capsule, no darker fill. */
    background: transparent;
    border: 1px solid var(--c-border);
  }
  .winctl button {
    width: 32px;
    height: 24px;
    min-height: 0;
    border: none;
    background: transparent;
    color: var(--c-text-3);
    cursor: pointer;
    border-radius: var(--r-pill);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    line-height: 0;
    box-shadow: none;
    transition:
      transform var(--dur-fast) var(--ease-out-strong),
      background var(--dur-base) ease,
      color var(--dur-base) ease;
  }
  .winctl button:hover {
    background: var(--c-surface-2);
    color: var(--c-text);
  }
  .winctl button:active {
    transform: scale(0.96);
  }
  .winctl .winctl-close:hover {
    background: var(--c-danger);
    color: #fff;
  }
  .winctl svg {
    width: 14px;
    height: 14px;
    display: block;
  }
</style>
