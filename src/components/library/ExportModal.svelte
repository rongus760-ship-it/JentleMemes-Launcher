<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  export let instanceId: string;
  export let instanceName: string;
  export let onClose: () => void;
  export let showToast: (msg: string) => void;

  type Fmt = "zip" | "mrpack" | "jentlepack";
  let format: Fmt = "zip";
  let folders: Record<string, boolean> = {};
  let exporting = false;
  let showAdvanced = false;

  const folderLabels: Record<string, string> = {
    mods: "Моды",
    config: "Конфигурация",
    resourcepacks: "Ресурспаки",
    shaderpacks: "Шейдеры",
    saves: "Миры",
    scripts: "Скрипты",
    logs: "Логи",
    crash_reports: "Краш-репорты",
    options: "Настройки игры",
    screenshots: "Скриншоты",
    schematics: "Схематики",
  };
  const commonFolders = ["mods", "config", "resourcepacks", "shaderpacks", "saves"];
  $: advancedFolders = Object.keys(folders).filter((k) => !commonFolders.includes(k));

  onMount(() => {
    invoke("list_instance_folders", { id: instanceId })
      .then((dirs: unknown) => {
        const state: Record<string, boolean> = {};
        for (const d of (dirs as string[]) || []) {
          state[d] = ["mods", "config", "resourcepacks", "shaderpacks"].includes(d);
        }
        folders = state;
      })
      .catch(() => {
        folders = { mods: true, config: true, resourcepacks: true, shaderpacks: true };
      });
  });

  function toggleFolder(key: string) {
    folders = { ...folders, [key]: !folders[key] };
  }

  function toggleAll(on: boolean) {
    folders = Object.fromEntries(Object.keys(folders).map((k) => [k, on]));
  }

  $: selectedFolders = Object.entries(folders)
    .filter(([, v]) => v)
    .map(([k]) => k);

  function pickExportFormat(f: string) {
    if (f === "zip" || f === "mrpack" || f === "jentlepack") format = f;
  }

  async function doExport() {
    exporting = true;
    try {
      const cmd =
        format === "mrpack"
          ? "export_mrpack"
          : format === "jentlepack"
            ? "export_jentlepack"
            : "export_instance";
      const res = await invoke(cmd, { id: instanceId, selectedFolders });
      showToast(res as string);
      onClose();
    } catch (e) {
      showToast(`Ошибка экспорта: ${e}`);
    } finally {
      exporting = false;
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-[110] bg-black/80 backdrop-blur-md flex items-center justify-center p-4"
  transition:fade={{ duration: 200 }}
  on:click={onClose}
  role="presentation"
>
  <div
    class="bg-jm-card border border-white/10 p-8 rounded-3xl w-full max-w-md shadow-2xl max-h-[85vh] overflow-y-auto custom-scrollbar jm-modal-enter"
    transition:scale={{ duration: 280, start: 0.96, easing: quintOut }}
    on:click|stopPropagation
    role="dialog"
  >
    <h3 class="text-2xl font-bold text-white mb-6">Экспорт «{instanceName}»</h3>

    <div class="mb-6">
      <label class="text-sm text-[var(--text-secondary)] mb-2 block">Формат</label>
      <div class="flex gap-2">
        {#each [["zip", ".zip"], ["mrpack", ".mrpack"], ["jentlepack", ".jentlepack"]] as pair (pair[0])}
          <button
            type="button"
            on:click={() => pickExportFormat(pair[0])}
            class="flex-1 py-2.5 rounded-xl font-bold text-xs transition-colors border {format === pair[0]
              ? 'bg-jm-accent text-black border-jm-accent'
              : 'bg-white/5 text-[var(--text-secondary)] border-white/10 hover:border-white/30'} jm-tap-scale"
          >
            {pair[1]}
          </button>
        {/each}
      </div>
      {#if format === "jentlepack"}
        <p class="text-[11px] text-[var(--text-secondary)] mt-2 leading-relaxed">
          Файл <span class="text-jm-accent font-mono">.jentlepack</span> — ZIP с манифестом
          <span class="font-mono">jentlepack.json</span>.
        </p>
      {/if}
    </div>

    <div class="mb-4">
      <div class="flex items-center justify-between mb-2">
        <label class="text-sm text-[var(--text-secondary)]">Включить папки</label>
        <div class="flex gap-2 text-[10px]">
          <button type="button" on:click={() => toggleAll(true)} class="text-jm-accent hover:underline font-bold"
            >Все</button
          >
          <button
            type="button"
            on:click={() => toggleAll(false)}
            class="text-[var(--text-secondary)] hover:underline font-bold">Нет</button
          >
        </div>
      </div>
      <div class="grid grid-cols-2 gap-2">
        {#each commonFolders.filter((k) => k in folders) as key (key)}
          <label
            class="flex items-center gap-2 cursor-pointer p-3 bg-black/30 rounded-xl border border-white/5 hover:border-white/20 transition-colors"
          >
            <input
              type="checkbox"
              checked={folders[key]}
              on:change={() => toggleFolder(key)}
              class="w-4 h-4 accent-jm-accent cursor-pointer"
            />
            <div>
              <span class="text-white text-sm font-bold block">{folderLabels[key] || key}</span>
              <span class="text-[10px] text-[var(--text-secondary)]">/{key}</span>
            </div>
          </label>
        {/each}
      </div>
    </div>

    {#if advancedFolders.length > 0}
      <div class="mb-6">
        <button
          type="button"
          on:click={() => (showAdvanced = !showAdvanced)}
          class="flex items-center gap-2 text-sm text-[var(--text-secondary)] hover:text-jm-accent font-bold transition-colors mb-2"
        >
          <svg
            width="10"
            height="10"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            class="transition-transform duration-200"
            class:rotate-180={showAdvanced}><polyline points="6 9 12 15 18 9" /></svg
          >
          Расширенные ({advancedFolders.length})
        </button>
        {#if showAdvanced}
          <div class="grid grid-cols-2 gap-2">
            {#each advancedFolders as key (key)}
              <label
                class="flex items-center gap-2 cursor-pointer p-2.5 bg-black/20 rounded-lg border border-white/5 hover:border-white/15 transition-colors"
              >
                <input
                  type="checkbox"
                  checked={folders[key]}
                  on:change={() => toggleFolder(key)}
                  class="w-3.5 h-3.5 accent-jm-accent cursor-pointer"
                />
                <div>
                  <span class="text-white text-xs font-bold block">{folderLabels[key] || key}</span>
                  <span class="text-[9px] text-[var(--text-secondary)]">/{key}</span>
                </div>
              </label>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    <div class="flex gap-3">
      <button
        type="button"
        on:click={doExport}
        disabled={exporting || selectedFolders.length === 0}
        class="flex-1 bg-jm-accent hover:bg-jm-accent-light text-black py-3 rounded-xl font-bold transition-colors disabled:opacity-50 jm-tap-scale"
      >
        {exporting ? "Экспорт..." : "Экспортировать"}
      </button>
      <button
        type="button"
        on:click={onClose}
        class="px-6 py-3 bg-white/10 hover:bg-white/20 text-white rounded-xl font-bold transition-colors jm-tap-scale"
        >Отмена</button
      >
    </div>
  </div>
</div>
