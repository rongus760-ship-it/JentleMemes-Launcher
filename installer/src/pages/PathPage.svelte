<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { fly } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import { FolderOpen, ChevronLeft, ChevronRight, HardDrive } from "lucide-svelte";

  export let installPath = "";
  export let onBack: () => void;
  export let onNext: () => void;

  type DiskInfo = { available_gb: number; required_gb: number; enough: boolean };

  let diskInfo: DiskInfo | null = null;
  let pathValid = true;

  $: {
    const p = installPath;
    void invoke<DiskInfo>("check_disk_space", { installPath: p }).then((d) => (diskInfo = d));
    void invoke<boolean>("validate_path", { path: p }).then((v) => (pathValid = v));
  }

  $: canProceed = pathValid && (diskInfo?.enough ?? true);
</script>

<div class="flex-1 flex flex-col px-7 py-5 gap-5">
  <div in:fly={{ y: -8, duration: 280, easing: quintOut, opacity: 0 }}>
    <h2 class="text-[16px] font-semibold" style:color="var(--text)">Путь установки</h2>
    <p class="text-[12px] mt-1" style:color="var(--text-secondary)">
      Выберите папку для установки лаунчера.
    </p>
  </div>

  <div
    class="flex items-center gap-2"
    in:fly={{ y: 10, duration: 300, delay: 60, easing: quintOut, opacity: 0 }}
  >
    <div class="flex-1 relative">
      <input
        type="text"
        bind:value={installPath}
        class="ui-input {pathValid ? '' : 'is-invalid'}"
      />
      {#if !pathValid}
        <span
          class="absolute right-3 top-1/2 -translate-y-1/2 text-[11px]"
          style:color="var(--danger)"
        >Недопустимый путь</span>
      {/if}
    </div>
    <button
      type="button"
      class="ui-btn ui-btn-subtle h-10 w-10 shrink-0"
      title="Выбрать папку"
      on:click={async () => {
        const selected = await open({
          directory: true,
          multiple: false,
          title: "Выберите папку для установки",
        });
        if (selected != null) {
          installPath = typeof selected === "string" ? selected : selected[0] ?? installPath;
        }
      }}
    >
      <FolderOpen size={16} />
    </button>
  </div>

  <div
    class="ui-card flex items-center gap-3 px-4 py-3"
    in:fly={{ y: 10, duration: 300, delay: 120, easing: quintOut, opacity: 0 }}
  >
    <HardDrive size={16} style="color: var(--accent)" class="shrink-0" />
    {#if diskInfo}
      <div class="flex-1 min-w-0">
        <div class="flex items-center justify-between text-[13px] gap-2">
          <span style:color="var(--text-secondary)">Свободно на диске</span>
          <span
            class="font-medium tabular-nums"
            style:color={diskInfo.enough ? "var(--accent-light)" : "var(--danger)"}
          >
            {diskInfo.available_gb.toFixed(1)} ГБ
          </span>
        </div>
        <div class="flex items-center justify-between text-[12px] mt-0.5">
          <span style:color="var(--text-secondary)">Требуется</span>
          <span class="tabular-nums" style:color="var(--text-secondary)">
            ~{Math.max(diskInfo.required_gb, 0.05).toFixed(2)} ГБ
          </span>
        </div>
      </div>
    {:else}
      <span class="text-[12px]" style:color="var(--text-secondary)">Проверка…</span>
    {/if}
  </div>

  <div class="flex-1 min-h-2"></div>

  <div
    class="flex items-center justify-between gap-3"
    in:fly={{ y: 8, duration: 280, delay: 160, easing: quintOut, opacity: 0 }}
  >
    <button type="button" on:click={onBack} class="ui-btn ui-btn-ghost">
      <ChevronLeft size={14} />
      <span>Назад</span>
    </button>
    <button
      type="button"
      disabled={!canProceed}
      on:click={onNext}
      class="ui-btn ui-btn-primary h-10 px-5"
    >
      <span>Установить</span>
      <ChevronRight size={14} />
    </button>
  </div>
</div>
