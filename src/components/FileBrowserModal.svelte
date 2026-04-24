<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { fade, scale } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import { Folder, File, X, ChevronLeft, Loader2 } from "lucide-svelte";

  export let open = false;
  /** Путь относительно каталога данных (слэши `/`) */
  export let initialRelPath = "";
  export let onClose: () => void;

  let entries: { name: string; is_dir: boolean; size: number }[] = [];
  let current = "";
  let busy = false;

  $: if (open) {
    current = initialRelPath.replace(/\\/g, "/").replace(/^\/+/, "");
    void refresh();
  }

  async function refresh() {
    busy = true;
    try {
      entries = (await invoke("list_data_subdir_entries", { relPath: current })) as typeof entries;
    } catch {
      entries = [];
    } finally {
      busy = false;
    }
  }

  function enterDir(name: string) {
    current = current ? `${current}/${name}` : name;
    void refresh();
  }

  function goUp() {
    const parts = current.split("/").filter(Boolean);
    parts.pop();
    current = parts.join("/");
    void refresh();
  }

  function fmtSize(n: number) {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1024 / 1024).toFixed(1)} MB`;
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-[20000] flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm"
    transition:fade={{ duration: 180 }}
    on:click|self={onClose}
  >
    <div
      class="w-full max-w-lg max-h-[min(520px,80vh)] bg-jm-card border border-white/15 rounded-2xl shadow-2xl flex flex-col overflow-hidden"
      transition:scale={{ duration: 220, start: 0.96, easing: quintOut }}
      on:click|stopPropagation
    >
      <div class="flex items-center justify-between px-4 py-3 border-b border-white/10 shrink-0">
        <div class="min-w-0 flex items-center gap-2">
          <button
            type="button"
            class="p-1.5 rounded-lg hover:bg-white/10 text-[var(--text-secondary)] disabled:opacity-30"
            disabled={!current}
            on:click={goUp}
            aria-label="Назад"
          >
            <ChevronLeft size={20} />
          </button>
          <span class="text-xs font-mono text-[var(--text-secondary)] truncate" title={current || "/"}
            >/{current || ""}</span
          >
        </div>
        <button
          type="button"
          class="p-1.5 rounded-lg hover:bg-white/10 text-[var(--text-secondary)]"
          on:click={onClose}
          aria-label="Закрыть"
        >
          <X size={20} />
        </button>
      </div>
      <div class="flex-1 min-h-0 overflow-y-auto custom-scrollbar p-2">
        {#if busy}
          <div class="flex items-center justify-center py-16 text-jm-accent gap-2">
            <Loader2 size={22} class="animate-spin" />
            <span class="text-sm">Загрузка…</span>
          </div>
        {:else if entries.length === 0}
          <p class="text-sm text-[var(--text-secondary)] text-center py-12">Пусто или папка не найдена</p>
        {:else}
          <ul class="space-y-0.5">
            {#each entries as ent (ent.name)}
              <li>
                {#if ent.is_dir}
                  <button
                    type="button"
                    class="w-full flex items-center gap-2 px-3 py-2 rounded-xl text-left text-sm hover:bg-jm-accent/15 border border-transparent hover:border-jm-accent/25 transition-all"
                    on:click={() => enterDir(ent.name)}
                  >
                    <Folder size={16} class="text-jm-accent shrink-0" />
                    <span class="text-white truncate">{ent.name}</span>
                  </button>
                {:else}
                  <div
                    class="flex items-center gap-2 px-3 py-2 rounded-xl text-sm text-[var(--text-secondary)] border border-white/5"
                  >
                    <File size={16} class="shrink-0 opacity-70" />
                    <span class="truncate flex-1">{ent.name}</span>
                    <span class="text-[10px] opacity-70 shrink-0">{fmtSize(ent.size)}</span>
                  </div>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </div>
      <p class="text-[10px] text-[var(--text-secondary)] px-4 py-2 border-t border-white/5">
        Только каталог данных лаунчера. Внешние пути недоступны.
      </p>
    </div>
  </div>
{/if}
