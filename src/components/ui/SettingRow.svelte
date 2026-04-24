<script lang="ts">
  export let label: string;
  export let hint: string | undefined = undefined;
  /** If true, the control renders under the label instead of to the right. */
  export let stacked: boolean = false;
  export let labelFor: string | undefined = undefined;
</script>

<div class="ui-row" class:ui-row-stacked={stacked}>
  <div class="min-w-0">
    {#if labelFor}
      <label for={labelFor} class="block text-sm font-medium text-[var(--text)]">{label}</label>
    {:else}
      <div class="text-sm font-medium text-[var(--text)]">{label}</div>
    {/if}
    {#if hint}
      <p class="ui-hint mt-1">{hint}</p>
    {/if}
    {#if $$slots.description}
      <div class="mt-1">
        <slot name="description" />
      </div>
    {/if}
  </div>
  <div class="{stacked ? '' : 'justify-self-end'} flex items-center gap-2">
    <slot />
  </div>
  {#if $$slots.extra}
    <div class="col-span-full mt-2">
      <slot name="extra" />
    </div>
  {/if}
</div>
