<script lang="ts">
  export let items: Array<{ id: string; label: string; icon?: any; badge?: string | number }>;
  export let value: string;
  export let variant: "pills" | "underline" | "segmented" = "pills";
  export let size: "sm" | "md" = "md";
  export let onChange: (id: string) => void = () => {};

  function select(id: string) {
    if (value === id) return;
    value = id;
    onChange(id);
  }

  function handleKey(e: KeyboardEvent, id: string) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      select(id);
    }
  }
</script>

<div class="ui-tabs ui-tabs-{variant} ui-tabs-{size}" role="tablist">
  {#each items as item (item.id)}
    {@const active = item.id === value}
    <button
      type="button"
      role="tab"
      aria-selected={active}
      data-tab={item.id}
      class="ui-tab"
      class:is-active={active}
      on:click={() => select(item.id)}
      on:keydown={(e) => handleKey(e, item.id)}
    >
      {#if item.icon}
        <svelte:component this={item.icon} size={size === "sm" ? 13 : 15} strokeWidth={2} />
      {/if}
      <span class="ui-tab-label">{item.label}</span>
      {#if item.badge !== undefined && item.badge !== null && item.badge !== ""}
        <span class="ui-tab-badge">{item.badge}</span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .ui-tabs {
    display: inline-flex;
    gap: 2px;
  }
  .ui-tab {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 7px 12px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
    background: transparent;
    border: 0;
    border-radius: var(--radius-sm);
    cursor: pointer;
    position: relative;
    transition: color 140ms var(--ease-standard), background 140ms var(--ease-standard);
  }
  .ui-tabs-sm .ui-tab {
    padding: 5px 10px;
    font-size: 11px;
  }
  .ui-tab:hover {
    color: var(--text);
    background: var(--surface-hover);
  }
  .ui-tab.is-active {
    color: var(--text);
    font-weight: 600;
  }

  /* Pills variant */
  .ui-tabs-pills .ui-tab.is-active {
    background: var(--accent-softer);
    color: var(--accent-light);
  }

  /* Underline variant */
  .ui-tabs-underline {
    border-bottom: 1px solid var(--border);
    padding-bottom: 0;
    gap: 4px;
  }
  .ui-tabs-underline .ui-tab {
    border-radius: 0;
    padding-bottom: 9px;
  }
  .ui-tabs-underline .ui-tab.is-active::after {
    content: "";
    position: absolute;
    bottom: -1px;
    left: 8px;
    right: 8px;
    height: 2px;
    border-radius: 2px 2px 0 0;
    background: var(--accent);
  }

  /* Segmented variant */
  .ui-tabs-segmented {
    padding: 3px;
    background: var(--surface-1);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  .ui-tabs-segmented .ui-tab.is-active {
    background: var(--accent-soft);
    color: var(--accent-light);
  }

  .ui-tab-badge {
    display: inline-flex;
    align-items: center;
    padding: 0 6px;
    min-width: 16px;
    height: 16px;
    font-size: 10px;
    font-weight: 700;
    color: var(--accent-light);
    background: var(--accent-softer);
    border-radius: 999px;
  }
</style>
