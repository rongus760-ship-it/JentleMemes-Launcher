<script lang="ts">
  import { slide } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import { ChevronDown } from "lucide-svelte";

  /** Список версий модпака (вместо нативного &lt;select&gt;) */
  export let versions: any[] = [];
  export let selectedId: string = "";
  export let onSelect: (id: string) => void = () => {};
  export let emptyHint: string = "Нет версий";
  /** Компактные строки (вкладка «Сборка») */
  export let compact = false;
  /**
   * Свернуть список за кнопкой (по умолчанию вкладка «Сборка»).
   * В модалке «Ядро» передавайте collapsible={false}, чтобы сразу видеть список.
   */
  export let collapsible = true;

  let expanded = false;

  $: effectiveId = String(selectedId || "");
  $: summaryLabel = (() => {
    const hit = versions.find((v) => String(v.id) === effectiveId);
    return hit?.name || hit?.id || effectiveId || "Выберите версию";
  })();
</script>

<div class="flex flex-col gap-2 min-w-0">
  {#if collapsible}
    <button
      type="button"
      on:click={() => (expanded = !expanded)}
      class="w-full flex items-center justify-between gap-3 min-h-[44px] px-4 py-2.5 rounded-xl border border-white/10 bg-black/50 text-left text-white hover:bg-black/65 hover:border-jm-accent/35 transition-colors"
      aria-expanded={expanded}
    >
      <div class="min-w-0 flex flex-col gap-0.5">
        <span class="text-[10px] font-bold uppercase tracking-wide text-[var(--text-secondary)]"
          >Текущий выбор</span
        >
        <span class="font-bold text-sm truncate text-jm-accent-light">{summaryLabel}</span>
      </div>
      <ChevronDown
        size={20}
        class="shrink-0 text-[var(--text-secondary)] transition-transform duration-200 {expanded
          ? 'rotate-180'
          : ''}"
      />
    </button>
  {/if}

  {#if !collapsible || expanded}
    <div
      transition:slide={{ duration: 220, easing: quintOut }}
      class="rounded-xl border border-white/10 bg-black/40 overflow-hidden flex flex-col {compact
        ? 'max-h-48'
        : 'max-h-64'}"
      role="listbox"
      aria-label="Версии сборки"
    >
      <div class="overflow-y-auto custom-scrollbar divide-y divide-white/5">
        {#each versions as v (v.id)}
          {@const vid = String(v.id)}
          <button
            type="button"
            role="option"
            aria-selected={effectiveId === vid}
            on:click={() => {
              onSelect(vid);
              if (collapsible) expanded = false;
            }}
            class="w-full text-left px-4 {compact ? 'py-2.5' : 'py-3'} text-sm transition-colors flex items-center gap-3 {effectiveId === vid
              ? 'bg-jm-accent/15 text-jm-accent-light border-l-2 border-jm-accent pl-[14px]'
              : 'text-white hover:bg-white/[0.06] border-l-2 border-transparent'}"
          >
            <span class="font-bold truncate flex-1 min-w-0">{v.name || v.id}</span>
            {#if effectiveId === vid}
              <span class="text-xs font-black text-jm-accent shrink-0">✓</span>
            {/if}
          </button>
        {:else}
          <p class="p-4 text-xs text-[var(--text-secondary)]">{emptyHint}</p>
        {/each}
      </div>
    </div>
  {/if}
</div>
