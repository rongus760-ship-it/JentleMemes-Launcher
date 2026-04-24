<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { fade, scale } from "svelte/transition";
  import Fuse from "fuse.js";
  import { Search, CornerDownLeft, ArrowUpDown } from "lucide-svelte";
  import {
    commands as commandsStore,
    paletteOpen,
    closePalette,
    togglePalette,
    type Command,
  } from "../lib/commandRegistry";

  let query = "";
  let selected = 0;
  let allCommands: Command[] = [];
  let inputEl: HTMLInputElement | null = null;
  let listEl: HTMLDivElement | null = null;

  const unsub = commandsStore.subscribe((v) => {
    allCommands = v;
  });
  onDestroy(unsub);

  let fuse: Fuse<Command> = new Fuse(allCommands, {
    keys: [
      { name: "title", weight: 2 },
      { name: "description", weight: 1 },
      { name: "keywords", weight: 1.5 },
      { name: "group", weight: 0.5 },
    ],
    threshold: 0.35,
    ignoreLocation: true,
    minMatchCharLength: 1,
  });

  $: {
    fuse = new Fuse(allCommands, {
      keys: [
        { name: "title", weight: 2 },
        { name: "description", weight: 1 },
        { name: "keywords", weight: 1.5 },
        { name: "group", weight: 0.5 },
      ],
      threshold: 0.35,
      ignoreLocation: true,
      minMatchCharLength: 1,
    });
  }

  $: results = query.trim()
    ? fuse.search(query.trim()).map((r) => r.item)
    : allCommands;

  $: if (selected >= results.length) selected = Math.max(0, results.length - 1);

  $: groupedResults = (() => {
    const map = new Map<string, Command[]>();
    for (const c of results) {
      const g = c.group || "Команды";
      if (!map.has(g)) map.set(g, []);
      map.get(g)!.push(c);
    }
    return Array.from(map.entries());
  })();

  let isOpen = false;
  paletteOpen.subscribe(async (v) => {
    isOpen = v;
    if (v) {
      query = "";
      selected = 0;
      await tick();
      inputEl?.focus();
    }
  });

  async function runCommand(c: Command) {
    closePalette();
    try {
      await c.run();
    } catch (e) {
      console.error("[CommandPalette] run error:", e);
    }
  }

  function flatIndexOf(c: Command): number {
    return results.findIndex((x) => x.id === c.id);
  }

  function handleKey(e: KeyboardEvent) {
    if (!isOpen) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (results.length > 0) selected = (selected + 1) % results.length;
      scrollSelectedIntoView();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      if (results.length > 0) selected = (selected - 1 + results.length) % results.length;
      scrollSelectedIntoView();
    } else if (e.key === "Enter") {
      e.preventDefault();
      const c = results[selected];
      if (c) void runCommand(c);
    } else if (e.key === "Escape") {
      e.preventDefault();
      closePalette();
    }
  }

  function scrollSelectedIntoView() {
    tick().then(() => {
      const el = listEl?.querySelector<HTMLElement>("[data-selected=\"true\"]");
      el?.scrollIntoView({ block: "nearest" });
    });
  }

  function handleGlobal(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && (e.key === "k" || e.key === "K")) {
      e.preventDefault();
      togglePalette();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleGlobal);
    const openListener = () => {
      paletteOpen.set(true);
    };
    window.addEventListener("jm_open_palette", openListener as EventListener);
    return () => {
      window.removeEventListener("keydown", handleGlobal);
      window.removeEventListener("jm_open_palette", openListener as EventListener);
    };
  });
</script>

<svelte:window on:keydown={handleKey} />

{#if isOpen}
  <div
    class="jm-palette-backdrop"
    transition:fade={{ duration: 140 }}
    on:click|self={() => closePalette()}
    role="presentation"
  >
    <div class="jm-palette-shell" transition:scale={{ duration: 180, start: 0.96, opacity: 0 }}>
      <div class="jm-palette-input-row">
        <Search size={16} style="color: var(--text-secondary); flex-shrink: 0;" />
        <input
          bind:this={inputEl}
          bind:value={query}
          on:input={() => (selected = 0)}
          placeholder="Команды, разделы, действия…"
          type="text"
          autocomplete="off"
          spellcheck="false"
          class="jm-palette-input"
        />
        <kbd class="jm-palette-kbd">Esc</kbd>
      </div>

      <div class="jm-palette-list custom-scrollbar" bind:this={listEl}>
        {#if results.length === 0}
          <div class="jm-palette-empty">
            <div class="font-medium" style:color="var(--text)">Ничего не найдено</div>
            <div class="text-[11px] mt-1" style:color="var(--text-secondary)">
              Попробуйте другой запрос — поиск fuzzy по названию, группе и ключевым словам.
            </div>
          </div>
        {:else}
          {#each groupedResults as [group, items] (group)}
            <div class="jm-palette-group">
              <div class="jm-palette-group-label">{group}</div>
              {#each items as c (c.id)}
                {@const globalIdx = flatIndexOf(c)}
                {@const isSel = globalIdx === selected}
                <button
                  type="button"
                  class="jm-palette-item"
                  class:is-selected={isSel}
                  data-selected={isSel ? "true" : undefined}
                  on:click={() => runCommand(c)}
                  on:mouseenter={() => (selected = globalIdx)}
                >
                  {#if c.icon}
                    <span class="jm-palette-item-icon">
                      <svelte:component this={c.icon} size={14} strokeWidth={2} />
                    </span>
                  {:else}
                    <span class="jm-palette-item-icon jm-palette-item-icon-empty" aria-hidden="true"></span>
                  {/if}
                  <span class="jm-palette-item-title">{c.title}</span>
                  {#if c.description}
                    <span class="jm-palette-item-desc">{c.description}</span>
                  {/if}
                  {#if c.shortcut}
                    <kbd class="jm-palette-item-shortcut">{c.shortcut}</kbd>
                  {/if}
                </button>
              {/each}
            </div>
          {/each}
        {/if}
      </div>

      <div class="jm-palette-footer">
        <div class="flex items-center gap-3">
          <span class="flex items-center gap-1">
            <ArrowUpDown size={11} /> навигация
          </span>
          <span class="flex items-center gap-1">
            <CornerDownLeft size={11} /> выбор
          </span>
        </div>
        <span>JentleMemes 2.0</span>
      </div>
    </div>
  </div>
{/if}

<style>
  .jm-palette-backdrop {
    position: fixed;
    inset: 0;
    z-index: 10070;
    background: rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(6px) saturate(1.1);
    -webkit-backdrop-filter: blur(6px) saturate(1.1);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: clamp(6vh, 12vh, 120px);
  }
  .jm-palette-shell {
    width: min(640px, 94vw);
    max-height: 70vh;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-modal, 0 20px 50px rgba(0, 0, 0, 0.5));
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .jm-palette-input-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-1);
  }
  .jm-palette-input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: 0;
    outline: none;
    font-size: 14px;
    font-weight: 500;
    color: var(--text);
    font-family: var(--font-sans);
  }
  .jm-palette-input::placeholder {
    color: var(--text-secondary);
  }
  .jm-palette-kbd {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 600;
    padding: 2px 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    background: var(--surface-2);
  }
  .jm-palette-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 6px;
  }
  .jm-palette-empty {
    padding: 28px 16px;
    text-align: center;
  }
  .jm-palette-group {
    margin-bottom: 4px;
  }
  .jm-palette-group-label {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-secondary);
    padding: 8px 10px 4px;
  }
  .jm-palette-item {
    display: grid;
    grid-template-columns: auto 1fr auto;
    grid-auto-flow: row dense;
    align-items: center;
    gap: 6px 10px;
    width: 100%;
    padding: 8px 10px;
    border: 0;
    background: transparent;
    border-radius: var(--radius-sm);
    text-align: left;
    cursor: pointer;
    color: var(--text);
    transition: background 100ms var(--ease-standard);
  }
  .jm-palette-item:hover,
  .jm-palette-item.is-selected {
    background: var(--surface-hover);
  }
  .jm-palette-item.is-selected {
    background: var(--accent-softer);
  }
  .jm-palette-item-icon {
    display: inline-flex;
    width: 22px;
    height: 22px;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm);
    background: var(--surface-1);
    color: var(--accent-light);
  }
  .jm-palette-item-icon-empty {
    background: transparent;
  }
  .jm-palette-item-title {
    font-size: 13px;
    font-weight: 500;
    line-height: 1.3;
    color: var(--text);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .jm-palette-item-desc {
    grid-column: 2 / 3;
    font-size: 11px;
    line-height: 1.3;
    color: var(--text-secondary);
    margin-top: -2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .jm-palette-item-shortcut {
    grid-column: 3 / 4;
    grid-row: 1 / 2;
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 600;
    padding: 2px 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    background: var(--surface-1);
    white-space: nowrap;
  }
  .jm-palette-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 16px;
    font-size: 10px;
    color: var(--text-secondary);
    border-top: 1px solid var(--border);
    background: var(--surface-1);
  }
  .is-selected .jm-palette-item-icon {
    background: var(--accent-soft);
  }
</style>
