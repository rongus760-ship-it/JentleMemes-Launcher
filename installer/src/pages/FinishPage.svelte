<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { fly } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import { CheckCircle, Rocket, Sparkles } from "lucide-svelte";

  export let installPath: string;

  let launchOnClose = true;
  let runWizard = true;

  async function handleClose() {
    if (launchOnClose) {
      try {
        await invoke("launch_app", {
          installPath,
          args: runWizard ? ["--onboarding"] : [],
        });
      } catch {
        await invoke("launch_app", { installPath });
      }
    }
    try {
      await invoke("exit_app");
    } catch {
      await invoke("close_window");
    }
  }
</script>

<div class="flex-1 flex flex-col items-center justify-center px-8 gap-5 text-center">
  <div
    class="w-20 h-20 rounded-[var(--radius-lg)] flex items-center justify-center border jm-tick-in"
    style:background="color-mix(in srgb, var(--accent) 10%, var(--surface-1))"
    style:border-color="color-mix(in srgb, var(--accent) 40%, transparent)"
    style:color="var(--accent-light)"
  >
    <CheckCircle size={32} />
  </div>

  <div in:fly={{ y: 10, duration: 340, delay: 120, easing: quintOut, opacity: 0 }}>
    <h2 class="text-[18px] font-semibold tracking-tight" style:color="var(--text)">
      Установка завершена
    </h2>
    <p class="text-[12px] mt-1" style:color="var(--text-secondary)">
      JentleMemes Launcher готов к использованию
    </p>
  </div>

  <div
    class="ui-card max-w-[420px] w-full px-4 py-3"
    in:fly={{ y: 10, duration: 320, delay: 180, easing: quintOut, opacity: 0 }}
  >
    <span class="text-[11px] block" style:color="var(--text-secondary)">Установлено в:</span>
    <span class="text-[12px] break-all" style:color="var(--text)">{installPath}</span>
  </div>

  <div
    class="flex flex-col gap-2 text-left w-full max-w-[420px]"
    in:fly={{ y: 10, duration: 320, delay: 240, easing: quintOut, opacity: 0 }}
  >
    <label class="flex items-center gap-2.5 cursor-pointer group">
      <input type="checkbox" bind:checked={launchOnClose} class="sr-only peer" />
      <span
        class="w-4 h-4 rounded-[4px] border flex items-center justify-center transition-colors"
        style:border-color="var(--border-strong)"
        style:background={launchOnClose ? "var(--accent)" : "transparent"}
      >
        {#if launchOnClose}
          <svg width="10" height="10" viewBox="0 0 12 12">
            <path
              d="M2 6l3 3 5-5"
              stroke="#0a110a"
              stroke-width="2"
              fill="none"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        {/if}
      </span>
      <span class="text-[12.5px]" style:color="var(--text-secondary)">
        Запустить лаунчер после закрытия
      </span>
    </label>

    <label
      class="flex items-start gap-2.5 cursor-pointer group"
      class:opacity-50={!launchOnClose}
    >
      <input
        type="checkbox"
        bind:checked={runWizard}
        disabled={!launchOnClose}
        class="sr-only peer"
      />
      <span
        class="w-4 h-4 mt-0.5 rounded-[4px] border flex items-center justify-center transition-colors"
        style:border-color="var(--border-strong)"
        style:background={runWizard && launchOnClose ? "var(--accent)" : "transparent"}
      >
        {#if runWizard && launchOnClose}
          <svg width="10" height="10" viewBox="0 0 12 12">
            <path
              d="M2 6l3 3 5-5"
              stroke="#0a110a"
              stroke-width="2"
              fill="none"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        {/if}
      </span>
      <span class="text-[12.5px] leading-snug" style:color="var(--text-secondary)">
        <span class="inline-flex items-center gap-1">
          <Sparkles size={12} />
          Запустить мастер настройки внешнего вида
        </span>
        <br />
        <span class="text-[11px]">Поможет подобрать тему, навигацию и масштаб.</span>
      </span>
    </label>
  </div>

  <button
    type="button"
    on:click={handleClose}
    class="ui-btn ui-btn-primary h-10 px-6 mt-1"
    in:fly={{ y: 10, duration: 340, delay: 320, easing: quintOut, opacity: 0 }}
  >
    <Rocket size={14} />
    <span>Готово</span>
  </button>
</div>
