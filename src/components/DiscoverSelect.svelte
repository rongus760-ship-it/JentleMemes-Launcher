<script lang="ts">
  import { scale } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import { onMount, onDestroy } from "svelte";

  export let label = "";
  export let value: string;
  export let options: { value: string; label: string }[] = [];
  export let disabled = false;
  export let onChange: (v: string) => void = () => {};

  let isOpen = false;
  let rootEl: HTMLDivElement;

  const safeOptions = () => (Array.isArray(options) ? options : []);
  $: selectedOption =
    safeOptions().find((o) => o.value === value) || ({ label: "Любое", value: "" } as { value: string; label: string });

  function onDocDown(e: MouseEvent) {
    if (rootEl && !rootEl.contains(e.target as Node)) isOpen = false;
  }

  onMount(() => document.addEventListener("mousedown", onDocDown));
  onDestroy(() => document.removeEventListener("mousedown", onDocDown));
</script>

<div
  bind:this={rootEl}
  class="flex flex-col relative {disabled ? 'opacity-50 pointer-events-none' : ''}"
>
  <button
    type="button"
    on:click={() => (isOpen = !isOpen)}
    class="bg-black/50 border border-white/10 rounded-lg px-3 py-2 text-white cursor-pointer select-none hover:border-jm-accent transition-colors flex justify-between items-center min-w-[110px] text-xs shadow-inner jm-tap-scale"
  >
    {#if label}<span class="text-[var(--text-secondary)] mr-1.5">{label}:</span>{/if}
    <span class="font-bold truncate">{selectedOption.label}</span>
  </button>
  {#if isOpen}
    <div
      transition:scale={{ duration: 200, start: 0.97, easing: quintOut }}
      class="absolute top-[100%] mt-2 w-full bg-[var(--input-bg)]/95 backdrop-blur-xl border border-white/10 rounded-xl z-[20000] max-h-60 overflow-y-auto custom-scrollbar shadow-[0_10px_40px_rgba(0,0,0,0.8)]"
    >
      {#each safeOptions() as o (o.value)}
        <button
          type="button"
          on:click={() => {
            onChange(o.value);
            isOpen = false;
          }}
          class="w-full text-left px-4 py-2 cursor-pointer transition-colors text-sm {value === o.value
            ? 'bg-jm-accent/20 text-jm-accent-light'
            : 'text-white hover:bg-white/10'}"
        >
          {o.label}
        </button>
      {/each}
    </div>
  {/if}
</div>
