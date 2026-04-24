<script lang="ts">
  import { fly } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import { Download, ChevronRight } from "lucide-svelte";
  import pkg from "../../package.json";

  export let onNext: () => void;
</script>

<div class="flex-1 flex flex-col items-center justify-center px-8 gap-5 text-center">
  <div
    in:fly={{ y: 12, duration: 360, easing: quintOut, opacity: 0 }}
    class="w-20 h-20 rounded-[var(--radius-lg)] flex items-center justify-center border"
    style:background="color-mix(in srgb, var(--accent) 10%, var(--surface-1))"
    style:border-color="color-mix(in srgb, var(--accent) 35%, transparent)"
    style:color="var(--accent-light)"
  >
    <Download size={34} />
  </div>

  <div
    class="flex flex-col items-center gap-1"
    in:fly={{ y: 12, duration: 360, delay: 80, easing: quintOut, opacity: 0 }}
  >
    <h1 class="text-[22px] font-semibold tracking-tight" style:color="var(--text)">
      JentleMemes Launcher
    </h1>
    <p class="text-[12px] uppercase tracking-[0.22em]" style:color="var(--text-secondary)">
      Установщик
    </p>
  </div>

  <p
    class="max-w-[380px] text-[13px] leading-relaxed"
    style:color="var(--text-secondary)"
    in:fly={{ y: 10, duration: 360, delay: 140, easing: quintOut, opacity: 0 }}
  >
    Мы установим лаунчер на ваш компьютер. Это займёт меньше минуты. При первом запуске
    появится пошаговый мастер настройки внешнего вида.
  </p>

  <button
    type="button"
    on:click={onNext}
    class="ui-btn ui-btn-primary h-10 px-6 text-[13px] mt-2"
    in:fly={{ y: 10, duration: 340, delay: 220, easing: quintOut, opacity: 0 }}
  >
    <span>Начать установку</span>
    <ChevronRight size={14} />
  </button>

  <span class="text-[11px]" style:color="color-mix(in srgb, var(--text-secondary) 75%, transparent)">
    Версия {pkg.version}
  </span>
</div>
