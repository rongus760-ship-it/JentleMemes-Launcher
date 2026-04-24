<script lang="ts">
  export let title: string | undefined = undefined;
  export let hint: string | undefined = undefined;
  export let padding: string = "p-5";
  let className: string = "";
  export { className as class };
</script>

<section class="ui-card {padding} {className}">
  {#if title || $$slots.header || $$slots.action}
    <header class="flex items-start justify-between gap-3 mb-4">
      <div class="min-w-0">
        {#if title}
          <h3 class="ui-heading">{title}</h3>
        {/if}
        {#if hint}
          <p class="ui-hint mt-0.5">{hint}</p>
        {/if}
        <slot name="header" />
      </div>
      {#if $$slots.action}
        <div class="flex items-center gap-2 shrink-0">
          <slot name="action" />
        </div>
      {/if}
    </header>
  {/if}
  <slot />
</section>
