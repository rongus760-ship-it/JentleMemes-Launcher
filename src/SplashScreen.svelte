<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { fade } from "svelte/transition";

  export let message: string = "Инициализация…";
  /** 0..100 — если не передан, показывается индетерминантный индикатор. */
  export let progress: number | null = null;
  /**
   * Опционально: массив имён фаз для «чек-листа» под логотипом. Последняя
   * «активна». Если не передано — чек-лист не показывается.
   * Пример: ["Шрифты", "Тема", "Аккаунт", "Сборки"]
   */
  export let phases: string[] = [];
  /** Индекс текущей активной фазы (0..phases.length). Всё, что < idx — завершено. */
  export let phaseIndex: number = 0;

  let mounted = false;
  let tipIdx = 0;
  let tipTimer: ReturnType<typeof setInterval> | null = null;

  const letters = "JentleMemes".split("");
  const TIPS = [
    "Ctrl+K — глобальный поиск по всему лаунчеру.",
    "В Настройках → Оформление можно выбрать «Legacy», «Glass», «Modrinth».",
    "Оверлей включается горячей клавишей (Alt+` по умолчанию).",
    "FluxCore v3 кэширует classpath и снижает время запуска до ~13 с.",
    "Microsoft-токен проверяется локально — без лишних HTTP-запросов.",
    "Палитру акцента можно поменять в настройках, даже кастомный HEX.",
    "Сборки раскладываются по data_dir/instances, иконка — instance.png.",
    "Drag-n-drop виджетов в оверлее — просто возьми за рукоятку слева.",
  ];

  onMount(() => {
    mounted = true;
    tipTimer = setInterval(() => {
      tipIdx = (tipIdx + 1) % TIPS.length;
    }, 3600);
  });
  onDestroy(() => {
    if (tipTimer) clearInterval(tipTimer);
  });

  $: currentTip = TIPS[tipIdx];
</script>

<div
  class="fixed inset-0 z-[20000] flex flex-col items-center justify-center bg-[var(--bg)]"
  transition:fade={{ duration: 240 }}
>
  <div class="absolute inset-0 pointer-events-none overflow-hidden">
    <div
      class="jm-splash-orb absolute -top-32 left-1/2 -translate-x-1/2 w-[540px] h-[240px] rounded-full"
      style:background="radial-gradient(ellipse at center, color-mix(in srgb, var(--accent) 18%, transparent) 0%, transparent 70%)"
    ></div>
    <div
      class="jm-splash-orb-2 absolute bottom-[-120px] left-[-80px] w-[380px] h-[260px] rounded-full"
      style:background="radial-gradient(ellipse at center, color-mix(in srgb, var(--accent-light, var(--accent)) 12%, transparent) 0%, transparent 75%)"
    ></div>
    <div
      class="jm-splash-orb-3 absolute top-[30%] right-[-120px] w-[320px] h-[320px] rounded-full"
      style:background="radial-gradient(ellipse at center, color-mix(in srgb, var(--accent) 10%, transparent) 0%, transparent 80%)"
    ></div>
    <div
      class="absolute inset-x-0 bottom-0 h-[160px]"
      style:background="linear-gradient(to top, color-mix(in srgb, var(--accent) 8%, transparent), transparent)"
    ></div>
    <div class="jm-splash-stars absolute inset-0"></div>
  </div>

  <div class="relative z-10 flex flex-col items-center gap-6 px-8">
    <!-- Brand mark -->
    <div class="jm-splash-mark relative flex items-center justify-center">
      <div
        class="jm-splash-mark-ring absolute inset-[-14px] rounded-[calc(var(--radius-lg)+14px)] border"
        style:border-color="color-mix(in srgb, var(--accent) 35%, transparent)"
      ></div>
      <div
        class="w-20 h-20 rounded-[var(--radius-lg)] border border-[var(--accent)]/40 flex items-center justify-center relative"
        style:background="color-mix(in srgb, var(--accent) 12%, var(--surface-1))"
      >
        <svg viewBox="0 0 32 32" width="38" height="38" class="jm-splash-mark-logo">
          <path
            d="M6 8 L16 4 L26 8 L26 22 L16 28 L6 22 Z"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            stroke-linejoin="round"
            style:color="var(--accent-light)"
          />
          <path
            d="M11 12 L16 10 L21 12 L21 19 L16 22 L11 19 Z"
            fill="currentColor"
            style:color="var(--accent)"
            opacity="0.75"
          />
        </svg>
      </div>
    </div>

    <!-- Wordmark -->
    <div class="flex flex-col items-center gap-1 select-none">
      <div class="flex items-baseline gap-[1px]">
        {#each letters as ch, i (i + ch)}
          <span
            class="jm-splash-letter text-[22px] font-semibold tracking-tight"
            style:color="var(--text)"
            style="animation-delay: {mounted ? i * 45 : 0}ms"
          >{ch}</span>
        {/each}
      </div>
      <span
        class="text-[11px] uppercase tracking-[0.22em]"
        style:color="var(--text-secondary)"
      >Launcher 2.0</span>
    </div>

    <!-- Progress -->
    <div class="flex flex-col items-center gap-2 w-[280px]">
      {#if progress == null}
        <div
          class="h-[3px] w-full rounded-full overflow-hidden relative"
          style:background="color-mix(in srgb, var(--text) 8%, transparent)"
        >
          <span
            class="jm-splash-indet absolute top-0 bottom-0 w-1/3 rounded-full"
            style:background="linear-gradient(90deg, transparent, var(--accent), transparent)"
          ></span>
        </div>
      {:else}
        <div
          class="h-[3px] w-full rounded-full overflow-hidden"
          style:background="color-mix(in srgb, var(--text) 8%, transparent)"
        >
          <span
            class="block h-full rounded-full transition-[width] duration-300 ease-out"
            style:width="{Math.max(0, Math.min(100, progress))}%"
            style:background="var(--accent)"
          ></span>
        </div>
      {/if}
      <span class="text-[11px] tabular-nums" style:color="var(--text-secondary)">{message}</span>
    </div>

    {#if phases.length}
      <div class="flex flex-wrap items-center justify-center gap-x-3 gap-y-1 max-w-[420px]">
        {#each phases as p, i}
          {@const done = i < phaseIndex}
          {@const active = i === phaseIndex}
          <span
            class="inline-flex items-center gap-1.5 text-[10px] uppercase tracking-[0.14em]"
            style:color={done
              ? "var(--accent-light, var(--accent))"
              : active
                ? "var(--text)"
                : "var(--text-secondary)"}
            style:opacity={done ? 0.75 : active ? 1 : 0.5}
          >
            <span
              class="jm-splash-dot inline-block w-1.5 h-1.5 rounded-full"
              class:jm-splash-dot-active={active}
              style:background={done
                ? "var(--accent-light, var(--accent))"
                : active
                  ? "var(--accent)"
                  : "color-mix(in srgb, var(--text) 25%, transparent)"}
            ></span>
            {p}
          </span>
        {/each}
      </div>
    {/if}

    <!-- Rotating tip -->
    <div
      class="h-6 flex items-center justify-center text-center px-4 text-[11px] leading-snug max-w-[360px]"
      style:color="var(--text-secondary)"
    >
      {#key tipIdx}
        <span class="jm-splash-tip">{currentTip}</span>
      {/key}
    </div>
  </div>
</div>

<style>
  @keyframes jm-splash-letter-in {
    0% { opacity: 0; transform: translateY(6px); letter-spacing: -0.02em; }
    100% { opacity: 1; transform: translateY(0); letter-spacing: 0; }
  }
  .jm-splash-letter {
    display: inline-block;
    opacity: 0;
    animation: jm-splash-letter-in 420ms cubic-bezier(0.22, 1, 0.36, 1) forwards;
  }

  @keyframes jm-splash-mark-in {
    0% { opacity: 0; transform: scale(0.82); }
    100% { opacity: 1; transform: scale(1); }
  }
  .jm-splash-mark {
    animation: jm-splash-mark-in 520ms cubic-bezier(0.22, 1, 0.36, 1) both;
  }
  .jm-splash-mark-logo {
    animation: jm-splash-mark-spin 14s linear infinite;
    transform-origin: 50% 50%;
  }
  @keyframes jm-splash-mark-spin {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
  }

  @keyframes jm-splash-mark-ring {
    0%, 100% { transform: scale(1); opacity: 0.6; }
    50% { transform: scale(1.06); opacity: 0.9; }
  }
  .jm-splash-mark-ring {
    animation: jm-splash-mark-ring 2.4s ease-in-out infinite;
  }

  @keyframes jm-splash-indet {
    0% { left: -35%; }
    100% { left: 100%; }
  }
  .jm-splash-indet {
    animation: jm-splash-indet 1.35s cubic-bezier(0.45, 0, 0.15, 1) infinite;
  }

  @keyframes jm-splash-orb-drift {
    0%, 100% { transform: translate(-50%, 0) scale(1); opacity: 0.7; }
    50% { transform: translate(-50%, -4%) scale(1.04); opacity: 1; }
  }
  .jm-splash-orb {
    animation: jm-splash-orb-drift 7s ease-in-out infinite;
  }
  @keyframes jm-splash-orb-drift-2 {
    0%, 100% { transform: translate(0, 0) scale(1); opacity: 0.7; }
    50% { transform: translate(30px, -12px) scale(1.06); opacity: 1; }
  }
  .jm-splash-orb-2 { animation: jm-splash-orb-drift-2 9s ease-in-out infinite; }
  @keyframes jm-splash-orb-drift-3 {
    0%, 100% { transform: translate(0, 0) scale(1); opacity: 0.55; }
    50% { transform: translate(-24px, 18px) scale(1.08); opacity: 0.95; }
  }
  .jm-splash-orb-3 { animation: jm-splash-orb-drift-3 11s ease-in-out infinite; }

  .jm-splash-stars {
    background-image:
      radial-gradient(1px 1px at 10% 20%, rgba(255,255,255,0.28), transparent 60%),
      radial-gradient(1px 1px at 80% 15%, rgba(255,255,255,0.22), transparent 60%),
      radial-gradient(1px 1px at 40% 80%, rgba(255,255,255,0.18), transparent 60%),
      radial-gradient(1px 1px at 65% 55%, rgba(255,255,255,0.2), transparent 60%),
      radial-gradient(1px 1px at 25% 50%, rgba(255,255,255,0.14), transparent 60%),
      radial-gradient(1px 1px at 90% 75%, rgba(255,255,255,0.2), transparent 60%);
    opacity: 0.6;
    animation: jm-splash-stars-fade 5s ease-in-out infinite;
  }
  @keyframes jm-splash-stars-fade {
    0%, 100% { opacity: 0.4; }
    50% { opacity: 0.75; }
  }

  @keyframes jm-splash-tip-in {
    0% { opacity: 0; transform: translateY(4px); }
    100% { opacity: 1; transform: translateY(0); }
  }
  .jm-splash-tip {
    display: inline-block;
    animation: jm-splash-tip-in 340ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  @keyframes jm-splash-dot-pulse {
    0%, 100% { transform: scale(1); box-shadow: 0 0 0 0 color-mix(in srgb, var(--accent) 45%, transparent); }
    50% { transform: scale(1.25); box-shadow: 0 0 0 6px transparent; }
  }
  .jm-splash-dot-active {
    animation: jm-splash-dot-pulse 1.4s ease-in-out infinite;
  }

  :global(.jm-reduce-motion) .jm-splash-mark-logo,
  :global(.jm-reduce-motion) .jm-splash-mark-ring,
  :global(.jm-reduce-motion) .jm-splash-orb,
  :global(.jm-reduce-motion) .jm-splash-orb-2,
  :global(.jm-reduce-motion) .jm-splash-orb-3,
  :global(.jm-reduce-motion) .jm-splash-stars,
  :global(.jm-reduce-motion) .jm-splash-letter,
  :global(.jm-reduce-motion) .jm-splash-dot-active,
  :global(.jm-reduce-motion) .jm-splash-tip,
  :global(.jm-reduce-motion) .jm-splash-indet {
    animation: none !important;
  }
  :global(.jm-reduce-motion) .jm-splash-letter {
    opacity: 1;
  }
</style>
