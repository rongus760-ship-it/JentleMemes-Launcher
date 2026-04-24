<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { LAUNCHER_VERSION } from "./version";
  import { showToast } from "./lib/jmEvents";
  import { Minus, Square, Maximize2, X, Layers, Command } from "lucide-svelte";

  export let ingameOverlayEnabled = false;

  function openPalette() {
    window.dispatchEvent(new CustomEvent("jm_open_palette"));
  }

  let isMaximized = false;
  let interval: ReturnType<typeof setInterval>;

  async function refreshMax() {
    try {
      isMaximized = await invoke<boolean>("window_is_maximized");
    } catch {
      /* ignore */
    }
  }

  onMount(() => {
    void refreshMax();
    interval = setInterval(() => void refreshMax(), 500);
  });

  onDestroy(() => clearInterval(interval));

  function handleDrag(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    void invoke("window_drag");
  }

  async function openIngameOverlay() {
    if (!ingameOverlayEnabled) {
      showToast("Включите «Оверлей в игре» в расширенных настройках и сохраните");
      return;
    }
    const { toggleIngameOverlay } = await import("./lib/ingameOverlayToggle");
    await toggleIngameOverlay();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  on:mousedown={handleDrag}
  class="jm-titlebar h-9 flex justify-between items-center select-none shrink-0 z-[12000] relative overflow-hidden border-b border-[var(--border)] bg-[var(--header-bg)]"
>
  <div
    class="text-[11px] font-bold px-4 flex-grow h-full flex items-center pointer-events-none tracking-wide"
    style:color="var(--text-secondary)"
  >
    <span class="opacity-90">JentleMemes</span>
    <span class="mx-2 opacity-30">·</span>
    <span class="font-mono text-[10px] opacity-60">{LAUNCHER_VERSION}</span>
  </div>
  <div class="flex h-full">
    <button
      type="button"
      title="Быстрый поиск (Ctrl+K)"
      on:click={openPalette}
      class="px-3 hover:bg-jm-accent/15 transition-all duration-200 h-full flex items-center gap-1.5 jm-tap-scale"
      style:color="var(--text-secondary)"
    >
      <Command size={13} />
      <span class="text-[10px] font-mono opacity-70 hidden sm:inline">⌘K</span>
    </button>
    <button
      type="button"
      title={ingameOverlayEnabled ? "Оверлей в игре" : "Оверлей выключен в настройках"}
      on:click={() => void openIngameOverlay()}
      class="px-3 hover:bg-jm-accent/15 transition-all duration-200 h-full flex items-center justify-center jm-tap-scale"
      class:opacity-40={!ingameOverlayEnabled}
      style:color="var(--text-secondary)"
    >
      <Layers size={15} />
    </button>
    <button
      type="button"
      on:click={() => invoke("window_minimize")}
      class="px-4 hover:bg-jm-accent/15 transition-all duration-200 h-full flex items-center justify-center jm-tap-scale"
      style:color="var(--text-secondary)"
    >
      <Minus size={14} />
    </button>
    <button
      type="button"
      on:click={() => {
        void invoke("window_maximize");
        setTimeout(() => void refreshMax(), 100);
      }}
      class="px-4 hover:bg-jm-accent/15 transition-all duration-200 h-full flex items-center justify-center jm-tap-scale"
      style:color="var(--text-secondary)"
    >
      {#if isMaximized}
        <Square size={12} />
      {:else}
        <Maximize2 size={12} />
      {/if}
    </button>
    <button
      type="button"
      on:click={() => invoke("window_close")}
      class="px-4 hover:bg-red-500/90 hover:text-white transition-all duration-200 h-full flex items-center justify-center jm-tap-scale"
      style:color="var(--text-secondary)"
    >
      <X size={14} />
    </button>
  </div>
</div>
