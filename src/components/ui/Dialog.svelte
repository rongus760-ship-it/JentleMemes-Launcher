<script lang="ts">
  import { createEventDispatcher, onMount, onDestroy } from "svelte";
  import { fade, scale } from "svelte/transition";
  import { X } from "lucide-svelte";

  export let open = false;
  export let title = "";
  export let description = "";
  export let size: "sm" | "md" | "lg" | "xl" | "full" = "md";
  export let variant: "center" | "sheet" = "center";
  export let closeOnBackdrop = true;
  export let closeOnEscape = true;
  export let showCloseButton = true;

  const dispatch = createEventDispatcher<{ close: void }>();

  function close() {
    if (!open) return;
    open = false;
    dispatch("close");
  }

  function handleKey(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === "Escape" && closeOnEscape) {
      e.stopPropagation();
      close();
    }
  }

  function handleBackdropClick() {
    if (closeOnBackdrop) close();
  }

  onMount(() => {
    window.addEventListener("keydown", handleKey, true);
  });
  onDestroy(() => {
    window.removeEventListener("keydown", handleKey, true);
  });

  $: sizeClass = `ui-dialog-${size}`;
</script>

{#if open}
  <div
    class="jm-modal-backdrop jm-preset-modal-backdrop {variant === 'center'
      ? 'jm-modal-backdrop-center'
      : 'jm-modal-backdrop-right'}"
    transition:fade={{ duration: 150 }}
    on:click|self={handleBackdropClick}
    role="presentation"
  >
    <div
      class="jm-modal-surface jm-preset-modal-surface ui-dialog {sizeClass} {variant === 'sheet'
        ? 'jm-modal-side'
        : ''}"
      role="dialog"
      aria-modal="true"
      aria-labelledby={title ? "ui-dialog-title" : undefined}
      transition:scale={{ duration: 180, start: 0.96, opacity: 0 }}
    >
      {#if title || showCloseButton || $$slots.header}
        <header class="jm-modal-header">
          <slot name="header">
            <div class="flex-1 min-w-0">
              {#if title}
                <div id="ui-dialog-title" class="jm-modal-title">{title}</div>
              {/if}
              {#if description}
                <div class="ui-hint mt-0.5">{description}</div>
              {/if}
            </div>
          </slot>
          {#if showCloseButton}
            <button
              type="button"
              class="ui-btn ui-btn-icon ui-btn-ghost"
              aria-label="Закрыть"
              on:click={close}
            >
              <X size={16} />
            </button>
          {/if}
        </header>
      {/if}
      <div class="jm-modal-body">
        <slot />
      </div>
      {#if $$slots.footer}
        <footer class="jm-modal-footer">
          <slot name="footer" />
        </footer>
      {/if}
    </div>
  </div>
{/if}

<style>
  .ui-dialog {
    width: 100%;
  }
  .ui-dialog-sm {
    max-width: 380px;
  }
  .ui-dialog-md {
    max-width: 560px;
  }
  .ui-dialog-lg {
    max-width: 760px;
  }
  .ui-dialog-xl {
    max-width: 980px;
  }
  .ui-dialog-full {
    max-width: 100%;
    max-height: 100%;
    height: 100%;
  }
</style>
