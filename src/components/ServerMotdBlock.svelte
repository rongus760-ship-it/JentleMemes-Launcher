<script lang="ts">
  import McChatNode from "./McChatNode.svelte";
  import ServerPingFaces from "./ServerPingFaces.svelte";

  /** Корень поля description из SLP (объект / строка / массив). */
  export let motdChat: unknown = null;
  /** Плоские строки с бэка (fallback). */
  export let motdLines: string[] = [];
  export let samples: { name?: string; skin_url?: string | null }[] = [];
  /** Узкая карточка на главной — меньше отступы и clamp строк. */
  export let compact = false;
  export let showFaces = true;
  /** `jm` — акцент лаунчера; `neutral` — для тёмных встроенных панелей (чат). */
  export let tone: "jm" | "neutral" = "jm";

  $: hasChat = motdChat !== null && motdChat !== undefined && motdChat !== "";
  $: plainFallback = (motdLines && motdLines.length > 0 ? motdLines.join("\n") : "").trim();
</script>

<div
  class="jm-motd-shell relative overflow-hidden rounded-xl border transition-all duration-300 {compact
    ? 'p-2.5'
    : 'p-4'}"
  style={tone === "neutral"
    ? "border-color: rgba(255,255,255,0.08); background: linear-gradient(145deg, rgba(63,63,70,0.55) 0%, rgba(24,24,27,0.96) 55%, rgba(9,9,11,0.98) 100%); box-shadow: inset 0 1px 0 rgba(255,255,255,0.05), 0 10px 32px rgba(0,0,0,0.45);"
    : "border-color: rgba(255,255,255,0.12); background: linear-gradient(135deg, rgba(20, 184, 166, 0.07) 0%, rgba(15, 23, 42, 0.92) 42%, rgba(0, 0, 0, 0.55) 100%); box-shadow: inset 0 1px 0 rgba(255,255,255,0.06), 0 12px 40px rgba(0,0,0,0.35);"}
>
  <div
    class="pointer-events-none absolute inset-0 opacity-[0.04]"
    style="background-image: repeating-linear-gradient(0deg, transparent, transparent 2px, rgba(255,255,255,0.9) 2px, rgba(255,255,255,0.9) 3px);"
    aria-hidden="true"
  ></div>
  {#if tone === "jm"}
    <div class="absolute -top-8 -right-8 h-24 w-24 rounded-full bg-teal-400/15 blur-2xl" aria-hidden="true"></div>
  {:else}
    <div class="absolute -top-6 -right-6 h-20 w-20 rounded-full bg-violet-500/10 blur-2xl" aria-hidden="true"></div>
  {/if}

  <div class="relative flex flex-col gap-2 min-w-0">
    <div class="flex items-center justify-between gap-2 min-w-0">
      <span
        class="text-[10px] font-bold uppercase tracking-[0.2em] shrink-0 {tone === 'neutral'
          ? 'text-zinc-400'
          : 'text-teal-300/90'}"
        style="font-family: ui-monospace, monospace;">MOTD</span
      >
      {#if showFaces && samples?.length}
        <ServerPingFaces players={samples} size={compact ? 20 : 26} max={8} />
      {/if}
    </div>

    <div
      class="font-mono text-sm leading-snug min-w-0 {compact ? 'line-clamp-2' : 'line-clamp-4'}"
      style="--jm-motd-fg: rgba(226, 232, 240, 0.95); color: var(--jm-motd-fg);"
    >
      {#if hasChat}
        <McChatNode node={motdChat} />
      {:else if plainFallback}
        <McChatNode node={plainFallback} />
      {:else}
        <span class="text-[var(--text-secondary)] italic text-xs">Нет описания</span>
      {/if}
    </div>
  </div>
</div>
