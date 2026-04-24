import type { Action } from "svelte/action";

/** Переносит узел в `body` (или селектор), чтобы `position:fixed` и z-index не терялись внутри main. */
export const portal: Action<HTMLElement, string | undefined> = (node, targetSelector) => {
  let target: HTMLElement = document.body;
  if (typeof document !== "undefined" && targetSelector) {
    const el = document.querySelector(targetSelector);
    if (el instanceof HTMLElement) target = el;
  }
  target.appendChild(node);
  return {
    destroy() {
      node.parentNode?.removeChild(node);
    },
  };
};
