<script context="module" lang="ts">
  import type { ComponentType } from "svelte";

  export interface NavItem {
    id: string;
    label: string;
    icon?: ComponentType;
    hidden?: boolean;
  }
</script>

<script lang="ts">
  export let items: NavItem[] = [];
  export let active: string;
  export let onChange: (id: string) => void = () => {};
</script>

<nav class="flex flex-col gap-0.5">
  {#each items.filter((i) => !i.hidden) as item (item.id)}
    <button
      type="button"
      class="ui-nav-item"
      class:is-active={item.id === active}
      data-section={item.id}
      on:click={() => onChange(item.id)}
    >
      {#if item.icon}
        <svelte:component this={item.icon} size={16} strokeWidth={2} />
      {/if}
      <span class="truncate">{item.label}</span>
    </button>
  {/each}
</nav>
