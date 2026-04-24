<script lang="ts">
  export let loader = "";
  export let size = 20;
  export let className = "";

  const s = size;
  const cn = `shrink-0 ${className}`.trim();
  $: L = (loader || "").toLowerCase();
</script>

{#if L === "vanilla"}
  <svg
    width={s}
    height={s}
    class={cn}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <rect x="4" y="4" width="16" height="16" rx="2" />
    <path d="M9 12l3 3 3-3" />
  </svg>
{:else if L === "fabric"}
  <svg
    width={s}
    height={s}
    class={cn}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <path d="M4 4h6v6H4z" />
    <path d="M14 4h6v6h-6z" />
    <path d="M4 14h6v6H4z" />
    <path d="M14 14h6v6h-6z" />
  </svg>
{:else if L === "quilt"}
  <svg
    width={s}
    height={s}
    class={cn}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <rect x="2" y="2" width="20" height="20" rx="2" />
    <path d="M12 2v20" />
    <path d="M2 12h20" />
    <path d="M4 6l8 6 8-6" />
  </svg>
{:else if L === "forge"}
  <svg
    width={s}
    height={s}
    class={cn}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <path d="M12 3l-2 4h4l-2 4" />
    <rect x="4" y="11" width="16" height="8" rx="2" />
    <path d="M8 15h8" />
  </svg>
{:else if L === "neoforge"}
  <svg
    width={s}
    height={s}
    class={cn}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <path d="M12 2l-2 5 2 5 2-5-2-5z" />
    <rect x="4" y="12" width="16" height="8" rx="2" />
    <path d="M8 16h8" />
  </svg>
{:else if L === "liteloader" || L === "modloader"}
  <svg
    width={s}
    height={s}
    class={cn}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <path d="M12 2v4M12 18v4M4 12h4M16 12h4" />
    <circle cx="12" cy="12" r="3" />
  </svg>
{:else}
  <svg
    width={s}
    height={s}
    class={cn}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    stroke-linejoin="round"
  >
    <rect x="4" y="4" width="16" height="16" rx="2" />
    <path d="M9 12l3 3 3-3" />
  </svg>
{/if}
