import { getCurrentWindow } from "@tauri-apps/api/window";

// Manual window drag for the frameless Windows titlebar.
//
// Tauri's native `data-tauri-drag-region` starts the OS window-move loop
// on *mousedown*. On Windows that means the very click that focuses the
// window from another app is misread as the start of a drag: the window
// sticks to the cursor with no button held until you click again. Same
// fix toqe and plaza landed on independently: wait for real pointer
// movement past a small threshold before handing off to the OS — a plain
// focus-click never moves, so it never drags. Double-click still
// maximises/restores, matching the native behaviour we replaced.
const THRESHOLD = 4; // px of travel before a press becomes a drag
const INTERACTIVE =
  'button, a, input, select, textarea, label, [role="button"], [role="tab"], [data-no-drag]';

/** Svelte action. `enabled` should be true only when the window is
 *  actually frameless (Windows builds) — on macOS the native titlebar
 *  still owns dragging and a startDragging() from a header click would
 *  yank the window around. */
export function windowDrag(node: HTMLElement, enabled: boolean) {
  let on = enabled;

  function onMouseDown(e: MouseEvent) {
    if (!on || e.button !== 0) return;
    if ((e.target as HTMLElement).closest(INTERACTIVE)) return;
    const sx = e.clientX;
    const sy = e.clientY;
    const onMove = (ev: MouseEvent) => {
      if (Math.abs(ev.clientX - sx) < THRESHOLD && Math.abs(ev.clientY - sy) < THRESHOLD) return;
      cleanup();
      // Don't swallow rejections silently: a missing
      // `core:window:allow-start-dragging` capability grant lands here
      // and an invisible no-op titlebar is brutal to diagnose.
      getCurrentWindow().startDragging().catch((err) => {
        console.warn("startDragging failed (missing capability grant?):", err);
      });
    };
    const cleanup = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", cleanup);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", cleanup);
  }

  function onDblClick(e: MouseEvent) {
    if (!on) return;
    if ((e.target as HTMLElement).closest(INTERACTIVE)) return;
    getCurrentWindow().toggleMaximize().catch((err) => {
      console.warn("toggleMaximize failed (missing capability grant?):", err);
    });
  }

  node.addEventListener("mousedown", onMouseDown);
  node.addEventListener("dblclick", onDblClick);
  return {
    update(next: boolean) {
      on = next;
    },
    destroy() {
      node.removeEventListener("mousedown", onMouseDown);
      node.removeEventListener("dblclick", onDblClick);
    },
  };
}
