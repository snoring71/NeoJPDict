/**
 * Svelte action: call `close` when the user presses outside `node` or hits Escape.
 * Put it on the element that wraps both a popup menu and its toggle button, so the
 * toggle's own click still works.
 */
export function dismiss(node: HTMLElement, close: () => void) {
  let onClose = close;
  const onPointer = (e: PointerEvent) => {
    if (!node.contains(e.target as Node)) onClose();
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") onClose();
  };
  document.addEventListener("pointerdown", onPointer, true);
  document.addEventListener("keydown", onKey);
  return {
    update(next: () => void) {
      onClose = next;
    },
    destroy() {
      document.removeEventListener("pointerdown", onPointer, true);
      document.removeEventListener("keydown", onKey);
    },
  };
}
