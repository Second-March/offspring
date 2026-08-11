<script lang="ts" module>
  /** One spec type serves three shapes:
   *  - destructive confirms (default "Cancel" + danger primary),
   *  - neutral info-with-action hints (custom cancel label + plain primary),
   *  - plain alerts (hideCancel + "OK" primary) — replaces native alert(),
   *    which pops the jarring OS dialog inside a Tauri shell. */
  export interface ConfirmSpec {
    title: string;
    message: string;
    confirmLabel: string;
    onConfirm: () => void | Promise<void>;
    cancelLabel?: string;
    confirmClass?: string;
    onCancel?: () => void | Promise<void>;
    hideCancel?: boolean;
  }
</script>

<script lang="ts">
  let {
    spec,
    onclose,
  }: {
    spec: ConfirmSpec;
    /** Called after either button (or backdrop/Escape) settles — the
     *  owner clears its dialog state here. */
    onclose: () => void;
  } = $props();

  async function cancel() {
    const fn = spec.onCancel;
    onclose();
    if (fn) await fn();
  }

  async function confirm() {
    const fn = spec.onConfirm;
    onclose();
    await fn();
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") cancel();
  }}
/>

<!-- Themed confirmation modal. Backdrop click and Escape both cancel.
     Using <dialog> would be cleaner but requires .showModal() plumbing
     and steals focus globally — a custom overlay gives us simpler
     control over the close-on-backdrop and Escape behavior. -->
<div class="modal-backdrop" role="presentation" onclick={cancel}>
  <div
    class="modal"
    role="alertdialog"
    aria-labelledby="confirm-title"
    aria-describedby="confirm-message"
    onclick={(e) => e.stopPropagation()}
  >
    <h3 id="confirm-title" class="modal-title">{spec.title}</h3>
    <p id="confirm-message" class="modal-message">{spec.message}</p>
    <div class="modal-actions">
      {#if !spec.hideCancel}
        <button class="ghost" onclick={cancel}>
          {spec.cancelLabel ?? "Cancel"}
        </button>
      {/if}
      <button class={spec.confirmClass ?? "primary danger"} onclick={confirm}>
        {spec.confirmLabel}
      </button>
    </div>
  </div>
</div>

<style>
  /* Full-screen scrim with a centered card. The scrim catches outside
     clicks and dims the app so the user's eye lands on the card.
     z-index sits above the ctx-menu so a stacked confirmation wins. */
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
    white-space: pre-line;
  }
  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .modal-actions :global(button.danger),
  .modal-actions button.danger {
    background: var(--c-danger, #b91c1c);
    border-color: var(--c-danger, #b91c1c);
    color: #fff;
  }
  .modal-actions :global(button.danger:hover),
  .modal-actions button.danger:hover {
    background: var(--c-danger-hover, #991414);
    border-color: var(--c-danger-hover, #991414);
  }
  @keyframes modal-fade {
    from { opacity: 0; }
    to   { opacity: 1; }
  }
  /* Enter from scale(0.97) + fade — never scale(0), no bounce; utility
     dialogs should feel instant, not springy. */
  @keyframes modal-pop {
    from { opacity: 0; transform: translateY(4px) scale(0.97); }
    to   { opacity: 1; transform: translateY(0) scale(1); }
  }
  @media (prefers-reduced-motion: reduce) {
    .modal { animation: modal-fade var(--dur-base) ease-out; }
  }
</style>
