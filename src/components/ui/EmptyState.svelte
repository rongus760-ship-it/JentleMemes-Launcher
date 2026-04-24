<script lang="ts">
  export let title: string;
  export let description: string = "";
  export let icon: any = null;
  export let compact = false;
</script>

<div class="ui-empty {compact ? 'ui-empty-compact' : ''}">
  {#if icon}
    <div class="ui-empty-icon">
      <svelte:component this={icon} size={compact ? 28 : 40} strokeWidth={1.5} />
    </div>
  {/if}
  <h3 class="ui-empty-title">{title}</h3>
  {#if description}
    <p class="ui-empty-desc">{description}</p>
  {/if}
  {#if $$slots.default}
    <div class="ui-empty-actions">
      <slot />
    </div>
  {/if}
</div>

<style>
  .ui-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 48px 24px;
    color: var(--text-secondary);
    gap: 12px;
  }
  .ui-empty-compact {
    padding: 24px 16px;
    gap: 8px;
  }
  .ui-empty-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 56px;
    height: 56px;
    border-radius: 999px;
    background: var(--surface-1);
    border: 1px solid var(--border);
    color: var(--text-secondary);
  }
  .ui-empty-compact .ui-empty-icon {
    width: 40px;
    height: 40px;
  }
  .ui-empty-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
    margin: 0;
  }
  .ui-empty-desc {
    font-size: 12px;
    line-height: 1.45;
    margin: 0;
    max-width: 360px;
  }
  .ui-empty-actions {
    margin-top: 8px;
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    justify-content: center;
  }
</style>
