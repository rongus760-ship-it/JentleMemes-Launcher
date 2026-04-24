<script lang="ts">
  import { fade } from "svelte/transition";
  import { X, ExternalLink } from "lucide-svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { portal } from "../lib/portalAction";
  import { internalBrowserUrl, closeInternalBrowser } from "../lib/internalBrowser";

  function openExternal() {
    const u = $internalBrowserUrl;
    if (u) void openUrl(u);
  }
</script>

{#if $internalBrowserUrl}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    use:portal
    class="fixed inset-0 z-[10070] jm-preset-modal-backdrop flex flex-col p-3 sm:p-5"
    transition:fade={{ duration: 180 }}
    on:click={closeInternalBrowser}
    role="presentation"
  >
    <div
      class="jm-preset-modal-surface flex flex-col flex-1 min-h-0 max-w-[min(96vw,1280px)] w-full mx-auto rounded-2xl border border-jm-accent/40 bg-jm-card shadow-[0_0_60px_rgba(var(--accent-rgb),0.15)] overflow-hidden"
      on:click|stopPropagation
      role="dialog"
      aria-modal="true"
    >
      <div
        class="flex items-center gap-2 px-3 py-2.5 border-b border-[var(--border)] bg-black/30 shrink-0"
      >
        <p class="flex-1 min-w-0 text-xs font-mono text-[var(--text-secondary)] truncate">
          {$internalBrowserUrl}
        </p>
        <button
          type="button"
          on:click={openExternal}
          class="shrink-0 p-2 rounded-lg hover:bg-white/10 text-jm-accent"
          title="Во внешнем браузере"
        >
          <ExternalLink size={18} />
        </button>
        <button
          type="button"
          on:click={closeInternalBrowser}
          class="shrink-0 p-2 rounded-lg hover:bg-white/10 text-white"
          title="Закрыть"
        >
          <X size={20} />
        </button>
      </div>
      <iframe
        class="flex-1 w-full min-h-[50vh] border-0 bg-[#111]"
        title="Просмотр страницы"
        src={$internalBrowserUrl}
        sandbox="allow-same-origin allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox allow-downloads"
      ></iframe>
      <p class="text-[10px] px-3 py-2 text-[var(--text-secondary)] border-t border-[var(--border)]">
        Часть сайтов запрещает отображение во фрейме — тогда откройте ссылку во внешнем браузере.
      </p>
    </div>
  </div>
{/if}
