<script lang="ts">
  export let checked: boolean = false;
  export let disabled: boolean = false;
  export let id: string | undefined = undefined;
  export let ariaLabel: string | undefined = undefined;
</script>

<input
  type="checkbox"
  class="ui-toggle"
  bind:checked
  {disabled}
  {id}
  aria-label={ariaLabel}
  on:change
/>
