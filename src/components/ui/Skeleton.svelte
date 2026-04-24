<script lang="ts">
  export let width: string | number = "100%";
  export let height: string | number = 16;
  export let rounded: "sm" | "md" | "lg" | "full" = "md";

  $: w = typeof width === "number" ? `${width}px` : width;
  $: h = typeof height === "number" ? `${height}px` : height;

  $: radiusVar =
    rounded === "full"
      ? "999px"
      : rounded === "sm"
        ? "var(--radius-sm)"
        : rounded === "lg"
          ? "var(--radius-lg)"
          : "var(--radius)";
</script>

<span
  class="ui-skeleton"
  aria-busy="true"
  aria-hidden="true"
  style:width={w}
  style:height={h}
  style:border-radius={radiusVar}
></span>

<style>
  .ui-skeleton {
    display: inline-block;
    background: linear-gradient(
      90deg,
      color-mix(in srgb, var(--surface-1) 90%, var(--text) 4%) 0%,
      color-mix(in srgb, var(--surface-1) 70%, var(--text) 8%) 50%,
      color-mix(in srgb, var(--surface-1) 90%, var(--text) 4%) 100%
    );
    background-size: 200% 100%;
    animation: ui-skeleton-shimmer 1.4s linear infinite;
  }
  @keyframes ui-skeleton-shimmer {
    0% {
      background-position: 200% 0;
    }
    100% {
      background-position: -200% 0;
    }
  }
  :global(.jm-reduce-motion) .ui-skeleton {
    animation: none;
    background: var(--surface-1);
  }
</style>
