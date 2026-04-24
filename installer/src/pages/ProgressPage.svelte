<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { fade } from "svelte/transition";
  import { AlertTriangle, Loader2 } from "lucide-svelte";

  export let installPath: string;
  export let onDone: () => void;

  let percent = 0;
  let status = "Запуск установки…";
  let error: string | null = null;

  onMount(() => {
    let cancelled = false;
    const unsubs: Array<() => void> = [];

    void (async () => {
      try {
        unsubs.push(
          await listen<{ percent: number; status: string }>("install-progress", (ev) => {
            if (cancelled) return;
            percent = ev.payload.percent;
            status = ev.payload.status;
            if (ev.payload.percent >= 100) {
              setTimeout(onDone, 600);
            }
          }),
        );
        unsubs.push(
          await listen<string>("install-error", (ev) => {
            if (cancelled) return;
            error = ev.payload;
          }),
        );
        await invoke("run_install", { installPath });
      } catch (e) {
        if (!cancelled) error = String(e);
      }
    })();

    return () => {
      cancelled = true;
      unsubs.forEach((u) => u());
    };
  });
</script>

<div class="flex-1 flex flex-col items-center justify-center px-8 gap-5 text-center">
  {#if error}
    <div
      class="w-16 h-16 rounded-[var(--radius-lg)] flex items-center justify-center border"
      style:background="color-mix(in srgb, var(--danger) 14%, transparent)"
      style:border-color="color-mix(in srgb, var(--danger) 40%, transparent)"
      style:color="var(--danger)"
    >
      <AlertTriangle size={28} />
    </div>
    <div>
      <h3 class="text-[15px] font-semibold" style:color="var(--danger)">Ошибка установки</h3>
      <p class="text-[12px] mt-1.5 max-w-[420px] break-words" style:color="var(--text-secondary)">
        {error}
      </p>
    </div>
  {:else}
    <div
      class="w-16 h-16 rounded-[var(--radius-lg)] flex items-center justify-center border"
      style:background="color-mix(in srgb, var(--accent) 10%, var(--surface-1))"
      style:border-color="color-mix(in srgb, var(--accent) 35%, transparent)"
      style:color="var(--accent-light)"
    >
      <div class="animate-spin" style="animation-duration: 1.6s;">
        <Loader2 size={28} />
      </div>
    </div>

    <div in:fade={{ duration: 300 }}>
      <h2 class="text-[16px] font-semibold" style:color="var(--text)">Установка</h2>
      <p class="text-[12px] mt-1" style:color="var(--text-secondary)">Пожалуйста, подождите…</p>
    </div>

    <div class="w-full max-w-[400px] flex flex-col gap-2">
      <div
        class="relative h-2.5 rounded-full overflow-hidden border"
        style:background="color-mix(in srgb, var(--text) 6%, transparent)"
        style:border-color="var(--border)"
      >
        <div
          class="absolute inset-y-0 left-0 progress-striped transition-[width] duration-300 ease-out"
          style:width="{percent}%"
          style:background="var(--accent)"
        ></div>
      </div>
      <div class="flex items-center justify-between gap-2 text-[11px]">
        <span class="truncate" style:color="var(--text-secondary)">{status}</span>
        <span class="font-medium tabular-nums" style:color="var(--accent-light)">
          {Math.round(percent)}%
        </span>
      </div>
    </div>
  {/if}
</div>
