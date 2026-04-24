<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { openUrlInLauncher } from "../lib/jmOpenUrl";
  import { MessageCircle, ExternalLink } from "lucide-svelte";

  let baseUrl = "";
  let token = "";

  onMount(async () => {
    try {
      const s: any = await invoke("load_settings");
      baseUrl = String(s.social_api_base_url || "").trim().replace(/\/$/, "");
      token = String(s.social_api_token || "").trim();
    } catch {
      /* ignore */
    }
  });

  function chatFrameSrc(): string {
    if (!baseUrl) return "";
    const path = "/v1/launcher/embed";
    const q = token ? `?token=${encodeURIComponent(token)}` : "";
    return `${baseUrl}${path}${q}`;
  }
</script>

<div
  class="jm-container flex flex-col items-center h-full gap-4 pb-8 overflow-hidden"
>
  <div class="w-full shrink-0 pt-2">
    <h2 class="text-xl md:text-2xl font-bold text-jm-accent-light flex items-center gap-2">
      <MessageCircle size={26} class="text-jm-accent" /> Чат и друзья
    </h2>
    <p class="text-xs text-[var(--text-secondary)] mt-1 max-w-2xl">
      Вкладка работает с бэкендом Jentle Social API. Укажите базовый URL в «Расширенные настройки». Токен
      лаунчера (если задан) передаётся во фрейм для входа по API.
    </p>
  </div>

  {#if !baseUrl}
    <div
      class="w-full flex-1 min-h-[200px] rounded-2xl border border-amber-500/30 bg-amber-500/10 p-6 text-center"
    >
      <p class="text-sm text-white font-bold mb-2">URL сервера не настроен</p>
      <p class="text-xs text-[var(--text-secondary)] mb-4">
        Откройте «Расширенные настройки» → блок «Чат и Social API» и вставьте адрес бэкенда (например
        <code class="font-mono">http://127.0.0.1:3847</code>).
      </p>
    </div>
  {:else}
    <div class="w-full flex flex-wrap items-center gap-2 shrink-0">
      <button
        type="button"
        on:click={() => openUrlInLauncher(baseUrl)}
        class="text-xs font-bold px-3 py-2 rounded-xl border border-jm-accent/40 text-jm-accent hover:bg-jm-accent/15 flex items-center gap-1.5"
      >
        <ExternalLink size={14} /> Открыть сайт во встроенном браузере
      </button>
      <a
        href={baseUrl}
        target="_blank"
        rel="noopener noreferrer"
        class="text-xs font-bold px-3 py-2 rounded-xl border border-white/15 text-[var(--text-secondary)] hover:text-white"
      >
        В системном браузере
      </a>
    </div>
    <div class="w-full flex-1 min-h-0 rounded-2xl border border-[var(--border)] overflow-hidden bg-black/40">
      {#if chatFrameSrc()}
        <iframe
          title="Jentle Social"
          src={chatFrameSrc()}
          class="w-full h-full min-h-[480px] border-0"
          sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox"
        />
      {:else}
        <div class="p-8 text-center text-sm text-[var(--text-secondary)]">Некорректный URL</div>
      {/if}
    </div>
  {/if}
</div>
