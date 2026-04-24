<script lang="ts">
  export let variant: "primary" | "subtle" | "ghost" | "danger" = "subtle";
  export let size: "md" | "sm" | "icon" = "md";
  export let type: "button" | "submit" | "reset" = "button";
  export let disabled: boolean = false;
  export let title: string | undefined = undefined;
  export let ariaLabel: string | undefined = undefined;
  let className: string = "";
  export { className as class };

  $: variantClass =
    variant === "primary"
      ? "ui-btn-primary"
      : variant === "ghost"
      ? "ui-btn-ghost"
      : variant === "danger"
      ? "ui-btn-danger"
      : "ui-btn-subtle";
  $: sizeClass = size === "sm" ? "ui-btn-sm" : size === "icon" ? "ui-btn-icon" : "";
</script>

<button
  {type}
  {disabled}
  {title}
  aria-label={ariaLabel}
  class="ui-btn {variantClass} {sizeClass} {className}"
  on:click
>
  <slot />
</button>
