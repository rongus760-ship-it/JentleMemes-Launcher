<script lang="ts">
  export let value = 0;
  export let max = 100;
  export let size = 48;
  export let stroke = 4;
  export let label: string | null = null;
  export let indeterminate = false;

  $: percent = Math.max(0, Math.min(100, (value / max) * 100));
  $: r = (size - stroke) / 2;
  $: circumference = 2 * Math.PI * r;
  $: dashOffset = circumference - (percent / 100) * circumference;
</script>

<div class="ui-progress-ring" style:width="{size}px" style:height="{size}px">
  <svg width={size} height={size} viewBox="0 0 {size} {size}" class:indeterminate>
    <circle
      cx={size / 2}
      cy={size / 2}
      r={r}
      fill="none"
      stroke="var(--border)"
      stroke-width={stroke}
    />
    <circle
      cx={size / 2}
      cy={size / 2}
      r={r}
      fill="none"
      stroke="var(--accent)"
      stroke-width={stroke}
      stroke-linecap="round"
      stroke-dasharray={circumference}
      stroke-dashoffset={indeterminate ? circumference * 0.7 : dashOffset}
      style:transform="rotate(-90deg)"
      style:transform-origin="center"
    />
  </svg>
  {#if label !== null}
    <div class="ui-progress-ring-label">{label}</div>
  {/if}
</div>

<style>
  .ui-progress-ring {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .ui-progress-ring circle {
    transition: stroke-dashoffset var(--dur-base, 200ms) var(--ease-standard, ease);
  }
  .ui-progress-ring svg.indeterminate {
    animation: ui-ring-spin 1.2s linear infinite;
  }
  @keyframes ui-ring-spin {
    to {
      transform: rotate(360deg);
    }
  }
  .ui-progress-ring-label {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    font-weight: 600;
    color: var(--text);
  }
</style>
