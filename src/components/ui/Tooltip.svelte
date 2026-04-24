<script lang="ts">
  import { onDestroy } from "svelte";

  export let label: string;
  export let placement: "top" | "bottom" | "left" | "right" = "top";
  export let delay = 300;
  export let disabled = false;

  let show = false;
  let timer: ReturnType<typeof setTimeout> | null = null;

  function handleEnter() {
    if (disabled || !label) return;
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => (show = true), delay);
  }
  function handleLeave() {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
    show = false;
  }

  onDestroy(() => {
    if (timer) clearTimeout(timer);
  });
</script>

<span
  class="ui-tooltip-wrapper"
  on:mouseenter={handleEnter}
  on:mouseleave={handleLeave}
  on:focusin={handleEnter}
  on:focusout={handleLeave}
  role="presentation"
>
  <slot />
  {#if show && label && !disabled}
    <span class="ui-tooltip ui-tooltip-{placement}" role="tooltip">{label}</span>
  {/if}
</span>

<style>
  .ui-tooltip-wrapper {
    position: relative;
    display: inline-flex;
  }
  .ui-tooltip {
    position: absolute;
    z-index: 10070;
    pointer-events: none;
    font-size: 11px;
    font-weight: 500;
    line-height: 1.3;
    padding: 5px 8px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--bg) 92%, var(--text) 4%);
    color: var(--text);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-card);
    white-space: nowrap;
    max-width: 220px;
    animation: ui-tooltip-in 160ms var(--ease-out);
  }
  @keyframes ui-tooltip-in {
    from {
      opacity: 0;
      transform: translateY(2px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  .ui-tooltip-top {
    bottom: calc(100% + 6px);
    left: 50%;
    transform: translateX(-50%);
  }
  .ui-tooltip-bottom {
    top: calc(100% + 6px);
    left: 50%;
    transform: translateX(-50%);
  }
  .ui-tooltip-left {
    right: calc(100% + 6px);
    top: 50%;
    transform: translateY(-50%);
  }
  .ui-tooltip-right {
    left: calc(100% + 6px);
    top: 50%;
    transform: translateY(-50%);
  }
</style>
