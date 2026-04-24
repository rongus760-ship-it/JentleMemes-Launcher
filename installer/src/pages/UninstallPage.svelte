<script lang="ts">
  import { onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { fly, fade } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import { Trash2, CheckCircle, AlertTriangle } from "lucide-svelte";

  type Phase = "confirm" | "progress" | "done";

  let phase: Phase = "confirm";
  let percent = 0;
  let status = "";
  let error: string | null = null;

  const unsubs: Array<() => void> = [];

  onDestroy(() => {
    unsubs.forEach((u) => u());
  });

  async function startUninstall() {
    phase = "progress";
    try {
      unsubs.push(
        await listen<{ percent: number; status: string }>("install-progress", (ev) => {
          percent = ev.payload.percent;
          status = ev.payload.status;
          if (ev.payload.percent >= 100) {
            setTimeout(() => (phase = "done"), 500);
          }
        }),
      );
      unsubs.push(
        await listen<string>("install-error", (ev) => {
          error = ev.payload;
        }),
      );
      await invoke("run_uninstall");
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="flex-1 flex flex-col items-center justify-center px-8 gap-5 text-center">
  {#key phase}
    {#if phase === "confirm"}
      <div
        class="flex flex-col items-center gap-4"
        in:fly={{ y: 10, duration: 320, easing: quintOut, opacity: 0 }}
        out:fade={{ duration: 160 }}
      >
        <div
          class="w-16 h-16 rounded-[var(--radius-lg)] flex items-center justify-center border"
          style:background="color-mix(in srgb, var(--danger) 14%, transparent)"
          style:border-color="color-mix(in srgb, var(--danger) 35%, transparent)"
          style:color="var(--danger)"
        >
          <AlertTriangle size={28} />
        </div>
        <div>
          <h2 class="text-[16px] font-semibold" style:color="var(--text)">
            Удалить JentleMemes?
          </h2>
          <p class="text-[12.5px] mt-1.5 max-w-[360px]" style:color="var(--text-secondary)">
            Лаунчер и все его файлы будут удалены с компьютера. Данные игр (миры,
            настройки) останутся.
          </p>
        </div>
        <div class="flex items-center gap-2">
          <button
            type="button"
            on:click={() => invoke("exit_app")}
            class="ui-btn ui-btn-subtle h-9 px-4"
          >
            Отмена
          </button>
          <button
            type="button"
            on:click={startUninstall}
            class="ui-btn ui-btn-danger h-9 px-4"
          >
            <Trash2 size={14} />
            <span>Удалить</span>
          </button>
        </div>
      </div>
    {:else if phase === "progress"}
      <div
        class="flex flex-col items-center gap-4 w-full"
        in:fly={{ y: 10, duration: 280, easing: quintOut, opacity: 0 }}
        out:fade={{ duration: 160 }}
      >
        <div
          class="w-14 h-14 rounded-[var(--radius-lg)] flex items-center justify-center border"
          style:background="color-mix(in srgb, var(--danger) 10%, transparent)"
          style:border-color="color-mix(in srgb, var(--danger) 30%, transparent)"
          style:color="var(--danger)"
        >
          <div class="animate-spin" style="animation-duration: 1.6s;">
            <Trash2 size={22} />
          </div>
        </div>
        <h2 class="text-[15px] font-semibold" style:color="var(--text)">Удаление…</h2>
        <div class="w-full max-w-[400px] flex flex-col gap-2">
          <div
            class="relative h-2.5 rounded-full overflow-hidden border"
            style:background="color-mix(in srgb, var(--text) 6%, transparent)"
            style:border-color="var(--border)"
          >
            <div
              class="absolute inset-y-0 left-0 rounded-full transition-[width] duration-300"
              style:width="{percent}%"
              style:background="var(--danger)"
            ></div>
          </div>
          <div class="flex justify-between text-[11px] gap-2">
            <span class="truncate" style:color="var(--text-secondary)">{status}</span>
            <span class="font-medium tabular-nums" style:color="var(--danger)">
              {Math.round(percent)}%
            </span>
          </div>
        </div>
        {#if error}
          <p class="text-[12px]" style:color="var(--danger)" in:fade>{error}</p>
        {/if}
      </div>
    {:else}
      <div
        class="flex flex-col items-center gap-4"
        in:fly={{ y: 10, duration: 320, easing: quintOut, opacity: 0 }}
      >
        <div
          class="w-16 h-16 rounded-[var(--radius-lg)] flex items-center justify-center border jm-tick-in"
          style:background="color-mix(in srgb, var(--accent) 10%, var(--surface-1))"
          style:border-color="color-mix(in srgb, var(--accent) 40%, transparent)"
          style:color="var(--accent-light)"
        >
          <CheckCircle size={28} />
        </div>
        <div>
          <h2 class="text-[16px] font-semibold" style:color="var(--text)">
            Удаление завершено
          </h2>
          <p class="text-[12.5px] mt-1.5" style:color="var(--text-secondary)">
            JentleMemes Launcher был успешно удалён.
          </p>
        </div>
        <button
          type="button"
          on:click={() => invoke("exit_app")}
          class="ui-btn ui-btn-subtle h-9 px-5"
        >
          Закрыть
        </button>
      </div>
    {/if}
  {/key}
</div>
