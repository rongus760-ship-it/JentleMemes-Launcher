<script lang="ts">
  import SkinHeadAvatar from "./SkinHeadAvatar.svelte";

  export let players: { name?: string; skin_url?: string | null }[] = [];
  /** CSS px */
  export let size = 24;
  export let max = 8;
</script>

{#if players && players.length > 0}
  <div class="flex items-center pl-1" aria-label="Игроки на сервере">
    {#each players.slice(0, max) as p, i (p.name + "-" + i + "-" + (p.skin_url || ""))}
      <div
        class="rounded-md ring-2 ring-black/60 shadow-lg bg-zinc-900 overflow-hidden shrink-0 relative"
        style:width="{size}px"
        style:height="{size}px"
        style:margin-left={i === 0 ? "0" : "-10px"}
        style:z-index={String(i)}
        title={p.name || ""}
      >
        {#if p.skin_url}
          <SkinHeadAvatar src={p.skin_url} {size} alt={p.name || "?"} wrapperClass="rounded-md" />
        {:else}
          <div
            class="w-full h-full flex items-center justify-center font-bold text-white/65 uppercase bg-gradient-to-br from-zinc-700 to-zinc-900"
            style:font-size="{Math.max(8, Math.floor(size * 0.38))}px"
          >
            {(p.name || "?").charAt(0)}
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}
