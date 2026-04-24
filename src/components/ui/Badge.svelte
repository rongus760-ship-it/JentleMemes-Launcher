<script lang="ts">
  export let variant: "default" | "accent" | "success" | "warning" | "danger" | "neutral" = "default";
  export let size: "sm" | "md" = "sm";
  export let dot = false;

  $: cls =
    [
      "ui-badge",
      `ui-badge-${variant}`,
      size === "md" ? "ui-badge-md" : "",
      dot ? "ui-badge-dot" : "",
    ]
      .filter(Boolean)
      .join(" ");
</script>

<span class={cls}>
  {#if dot}
    <span class="ui-badge-indicator" aria-hidden="true"></span>
  {/if}
  <slot />
</span>

<style>
  .ui-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 2px 7px;
    border-radius: 999px;
    line-height: 1.3;
    white-space: nowrap;
    transition: background 160ms var(--ease-standard), color 160ms var(--ease-standard);
  }
  .ui-badge-md {
    font-size: 11px;
    padding: 3px 9px;
  }
  .ui-badge-default {
    background: color-mix(in srgb, var(--text-secondary) 14%, transparent);
    color: var(--text-secondary);
  }
  .ui-badge-accent {
    background: var(--accent-softer);
    color: var(--accent-light);
  }
  .ui-badge-success {
    background: color-mix(in srgb, #22c55e 18%, transparent);
    color: #86efac;
  }
  .ui-badge-warning {
    background: color-mix(in srgb, #f59e0b 18%, transparent);
    color: #fcd34d;
  }
  .ui-badge-danger {
    background: color-mix(in srgb, #ef4444 18%, transparent);
    color: #fca5a5;
  }
  .ui-badge-neutral {
    background: var(--surface-1);
    color: var(--text-secondary);
    border: 1px solid var(--border);
  }
  .ui-badge-indicator {
    width: 6px;
    height: 6px;
    border-radius: 999px;
    background: currentColor;
  }
</style>
