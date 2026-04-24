<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Newspaper, Pin } from "lucide-svelte";
  import { fade, fly } from "svelte/transition";
  import { quintOut, elasticOut } from "svelte/easing";
  import { portal } from "../lib/portalAction";

  interface NewsItem {
    id: string;
    title: string;
    body: string;
    image?: string | null;
    date: string;
    tag?: string;
    pinned?: boolean;
  }

  let news: NewsItem[] = [];
  let loading = true;
  let selected: NewsItem | null = null;

  const tagColors: Record<string, string> = {
    update: "bg-blue-500/20 text-blue-400",
    feature: "bg-green-500/20 text-green-400",
    announcement: "bg-jm-accent/20 text-jm-accent",
    event: "bg-purple-500/20 text-purple-400",
    bugfix: "bg-orange-500/20 text-orange-400",
  };

  onMount(() => {
    invoke("fetch_launcher_news")
      .then((items: unknown) => {
        news = (items as NewsItem[]) || [];
      })
      .catch(console.error)
      .finally(() => {
        loading = false;
      });
  });

</script>

<div class="jm-container-narrow flex flex-col h-full gap-4">
  {#if loading}
    <div class="flex items-center justify-center h-full">
      <div
        class="w-8 h-8 border-2 border-jm-accent border-t-transparent rounded-full animate-spin"
      ></div>
    </div>
  {:else}
    <div class="flex items-center gap-3 mb-2">
      <Newspaper size={24} class="text-jm-accent" />
      <h2 class="text-xl md:text-2xl font-bold text-jm-accent-light">Новости</h2>
    </div>

    {#if news.length === 0}
      <div class="text-center py-20" style:color="var(--text-secondary)">
        <Newspaper size={48} class="mx-auto mb-4 opacity-30" />
        <p class="text-lg font-bold">Пока нет новостей</p>
      </div>
    {:else}
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        {#each news as item, ni (item.id)}
          <button
            type="button"
            class="bg-jm-card rounded-2xl border border-[var(--border)] overflow-hidden cursor-pointer group text-left w-full card-hover-subtle hover:border-jm-accent/35 jm-tap-scale relative"
            style="animation-delay: {Math.min(ni * 40, 200)}ms"
            in:fly={{ y: 16, duration: 380, delay: Math.min(ni * 40, 200), easing: quintOut }}
            on:click={() => (selected = item)}
          >
            <div
              class="pointer-events-none absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-500 bg-gradient-to-tr from-jm-accent/[0.07] via-transparent to-transparent rounded-2xl"
            ></div>
            {#if item.image}
              <div class="h-40 overflow-hidden">
                <img
                  src={item.image}
                  alt=""
                  class="w-full h-full object-cover group-hover:scale-105 transition-transform duration-500"
                />
              </div>
            {/if}
            <div class="p-4">
              <div class="flex items-center gap-2 mb-2 flex-wrap">
                {#if item.pinned}
                  <span
                    class="flex items-center gap-1 text-[10px] px-2 py-0.5 bg-jm-accent/20 text-jm-accent rounded-full font-bold"
                  >
                    <Pin size={10} /> Закреплено
                  </span>
                {/if}
                {#if item.tag}
                  <span
                    class="text-[10px] px-2 py-0.5 rounded-full font-bold {tagColors[item.tag] ||
                      tagColors.announcement}"
                  >
                    {item.tag}
                  </span>
                {/if}
                <span class="text-[11px] ml-auto" style:color="var(--text-secondary)">
                  {item.date ? new Date(item.date).toLocaleDateString("ru") : ""}
                </span>
              </div>
              <h3 class="font-bold text-sm mb-1">{item.title}</h3>
              <p class="text-xs line-clamp-3" style:color="var(--text-secondary)">{item.body}</p>
            </div>
          </button>
        {/each}
      </div>
    {/if}
  {/if}

  {#if selected}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      use:portal
      class="fixed inset-0 z-[10055] bg-black/70 backdrop-blur-md flex items-center justify-center p-4"
      transition:fade={{ duration: 220 }}
      on:click={() => (selected = null)}
    >
      <div
        class="jm-gradient-border rounded-2xl max-w-lg w-full max-h-[80vh] shadow-2xl jm-ring-pulse"
        transition:fly={{ y: 28, duration: 420, easing: elasticOut }}
        on:click|stopPropagation
        role="dialog"
        aria-modal="true"
      >
        <div
          class="jm-gradient-border-inner rounded-2xl max-h-[80vh] overflow-y-auto custom-scrollbar border border-white/[0.06]"
        >
          {#if selected.image}
            <img src={selected.image} alt="" class="w-full h-48 object-cover rounded-t-2xl" />
          {/if}
          <div class="p-6">
            <div class="flex items-center gap-2 mb-3">
              {#if selected.tag}
                <span
                  class="text-xs px-2 py-1 rounded-full font-bold {tagColors[selected.tag] ||
                    tagColors.announcement}"
                >
                  {selected.tag}
                </span>
              {/if}
              <span class="text-xs" style:color="var(--text-secondary)">
                {selected.date ? new Date(selected.date).toLocaleDateString("ru") : ""}
              </span>
            </div>
            <h2 class="text-xl font-bold mb-3">{selected.title}</h2>
            <p class="text-sm leading-relaxed whitespace-pre-wrap" style:color="var(--text-secondary)">
              {selected.body}
            </p>
            <button
              type="button"
              on:click={() => (selected = null)}
              class="mt-4 bg-jm-accent/15 text-jm-accent px-4 py-2.5 rounded-xl font-bold text-sm hover:bg-jm-accent hover:text-black transition-all duration-300 jm-tap-scale border border-jm-accent/30"
            >
              Закрыть
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>
