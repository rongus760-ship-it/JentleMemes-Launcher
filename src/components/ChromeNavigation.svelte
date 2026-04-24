<script lang="ts">
  import type { ChromeLayout } from "../lib/chromeLayout";
  import SkinHeadAvatar from "./SkinHeadAvatar.svelte";
  import { PanelLeftClose, PanelLeftOpen } from "lucide-svelte";

  type TabItem = { id: string; label: string; Icon: any };

  export let layout: ChromeLayout;
  export let activeTab: string;
  export let primaryTabs: TabItem[];
  export let systemTabs: TabItem[];
  export let activeAccount: { username?: string; acc_type?: string } | null;
  export let activeAvatar: string;
  export let activeAvatarHeadFromTexture: boolean;
  export let onTab: (id: string) => void;
  export let onToggleDensity: () => void;

  $: horiz = layout === "top_tabs" || layout === "bottom_tabs";
  $: sideLeft = layout.startsWith("sidebar_left");
  $: sideRight = layout.startsWith("sidebar_right");
  $: compact =
    layout === "sidebar_left_compact" ||
    layout === "sidebar_right_compact" ||
    horiz;
  $: showSide = sideLeft || sideRight;

  function tabBtnClass(isActive: boolean, horizontal: boolean): string {
    const base =
      "relative flex items-center transition-colors duration-150 rounded-[var(--radius-sm)] text-left";
    if (horizontal) {
      return `${base} shrink-0 gap-1.5 px-2.5 py-2 text-[12px] ${
        isActive
          ? "bg-[var(--surface-1)] text-[var(--text)] font-semibold"
          : "text-[var(--text-secondary)] hover:text-[var(--text)] hover:bg-[var(--surface-hover)]"
      }`;
    }
    return `${base} ${compact ? "justify-center h-10 w-full" : "gap-2.5 pl-3 pr-2 py-2 text-[13px]"} ${
      isActive
        ? "bg-[var(--surface-1)] text-[var(--text)] font-semibold"
        : "text-[var(--text-secondary)] hover:text-[var(--text)] hover:bg-[var(--surface-hover)]"
    }`;
  }
</script>

{#if horiz}
  <nav
    class="jm-chrome-nav flex items-center gap-0.5 px-3 py-1.5 border-[var(--border)] bg-[var(--bg)] z-[12000] overflow-x-auto custom-scrollbar shrink-0 {layout ===
    'top_tabs'
      ? 'border-b'
      : 'border-t'}"
    data-role="shell-nav"
    data-orientation="horizontal"
    aria-label="Навигация"
    style="min-height: 2.75rem;"
  >
    {#each primaryTabs as item (item.id)}
      {@const isActive = activeTab === item.id}
      <button
        type="button"
        data-tab={item.id}
        aria-current={isActive ? "page" : undefined}
        on:click={() => onTab(item.id)}
        title={item.label}
        class={tabBtnClass(isActive, true)}
      >
        {#if isActive}
          <span
            class="absolute bottom-1 left-1/2 -translate-x-1/2 w-8 h-[3px] rounded-full pointer-events-none"
            style:background="var(--accent)"
          ></span>
        {/if}
        <svelte:component this={item.Icon} size={15} strokeWidth={2} />
        <span class="truncate max-w-[7rem]">{item.label}</span>
      </button>
    {/each}
    <div class="w-px h-5 shrink-0 mx-0.5 self-center" style:background="var(--border)" role="separator"></div>
    {#each systemTabs as item (item.id)}
      {@const isActive = activeTab === item.id}
      <button
        type="button"
        data-tab={item.id}
        aria-current={isActive ? "page" : undefined}
        on:click={() => onTab(item.id)}
        title={item.label}
        class={tabBtnClass(isActive, true)}
      >
        {#if isActive}
          <span
            class="absolute bottom-1 left-1/2 -translate-x-1/2 w-8 h-[3px] rounded-full pointer-events-none"
            style:background="var(--accent)"
          ></span>
        {/if}
        <svelte:component this={item.Icon} size={15} strokeWidth={2} />
        <span class="truncate max-w-[7rem]">{item.label}</span>
      </button>
    {/each}
    <div class="flex-1 min-w-2"></div>
    <button
      type="button"
      on:click={() => onTab("account")}
      aria-current={activeTab === "account" ? "page" : undefined}
      title={activeAccount ? activeAccount.username : "Аккаунт"}
      class="flex items-center gap-2 rounded-[var(--radius-sm)] px-2 py-1.5 shrink-0 {activeTab === 'account'
        ? 'bg-[var(--surface-1)]'
        : 'hover:bg-[var(--surface-hover)]'}"
    >
      {#key activeAvatar + String(activeAvatarHeadFromTexture)}
        {#if activeAvatarHeadFromTexture}
          <SkinHeadAvatar src={activeAvatar} size={22} alt="" wrapperClass="rounded-[4px] shrink-0" />
        {:else}
          <img
            src={activeAvatar}
            alt=""
            class="shrink-0 rounded-[4px] object-cover w-5 h-5"
            style:image-rendering="pixelated"
          />
        {/if}
      {/key}
      <span class="text-[12px] font-medium truncate max-w-[6rem]" style:color="var(--text)">
        {activeAccount ? activeAccount.username : "Offline"}
      </span>
    </button>
  </nav>
{:else if showSide}
  <aside
    class="jm-chrome-nav flex flex-col shrink-0 border-[var(--border)] bg-[var(--bg)] z-[12000] transition-[width] duration-200 ease-out {sideRight
      ? 'border-l order-last'
      : 'border-r'} {compact ? 'w-[56px]' : 'w-[208px]'}"
    data-role="shell-nav"
    data-orientation={sideRight ? "vertical-right" : "vertical-left"}
    aria-label="Навигация"
  >
    {#if !compact}
      <div class="px-4 pt-3 pb-2 flex items-baseline gap-2 select-none">
        <span class="text-[14px] font-semibold tracking-tight" style:color="var(--accent-light)"
          >JentleMemes</span
        >
      </div>
    {:else}
      <div class="h-[38px] shrink-0" aria-hidden="true"></div>
    {/if}

    <nav
      class="flex flex-col gap-0.5 overflow-y-auto custom-scrollbar flex-1 min-h-0 {compact
        ? 'px-1.5 pt-1'
        : 'px-2 pt-2'}"
    >
      {#each primaryTabs as item (item.id)}
        {@const isActive = activeTab === item.id}
        <button
          type="button"
          data-tab={item.id}
          aria-current={isActive ? "page" : undefined}
          on:click={() => onTab(item.id)}
          title={compact ? item.label : undefined}
          class={tabBtnClass(isActive, false)}
        >
          {#if isActive}
            <span
              class="absolute {sideRight ? 'right-0 rounded-l-[var(--radius-sm)]' : 'left-0 rounded-r-[var(--radius-sm)]'} top-1.5 bottom-1.5 w-[3px]"
              style:background="var(--accent)"
            ></span>
          {/if}
          <svelte:component this={item.Icon} size={compact ? 17 : 15} strokeWidth={2} />
          {#if !compact}
            <span class="truncate">{item.label}</span>
          {/if}
        </button>
      {/each}

      <div class="my-2 border-t border-[var(--border)]" role="separator"></div>

      {#each systemTabs as item (item.id)}
        {@const isActive = activeTab === item.id}
        <button
          type="button"
          data-tab={item.id}
          aria-current={isActive ? "page" : undefined}
          on:click={() => onTab(item.id)}
          title={compact ? item.label : undefined}
          class={tabBtnClass(isActive, false)}
        >
          {#if isActive}
            <span
              class="absolute {sideRight ? 'right-0 rounded-l-[var(--radius-sm)]' : 'left-0 rounded-r-[var(--radius-sm)]'} top-1.5 bottom-1.5 w-[3px]"
              style:background="var(--accent)"
            ></span>
          {/if}
          <svelte:component this={item.Icon} size={compact ? 17 : 15} strokeWidth={2} />
          {#if !compact}
            <span class="truncate">{item.label}</span>
          {/if}
        </button>
      {/each}
    </nav>

    <div
      class="mt-auto border-t border-[var(--border)] flex flex-col gap-1 {compact ? 'p-1.5' : 'p-2'}"
    >
      <button
        type="button"
        on:click={() => onTab("account")}
        aria-current={activeTab === "account" ? "page" : undefined}
        title={compact ? (activeAccount ? activeAccount.username : "Войти в аккаунт") : undefined}
        class="relative w-full flex items-center rounded-[var(--radius-sm)] transition-colors duration-150 {compact
          ? 'justify-center h-10'
          : 'gap-2.5 pl-2 pr-2 py-2 text-left'} {activeTab === 'account'
          ? 'bg-[var(--surface-1)]'
          : 'hover:bg-[var(--surface-hover)]'}"
      >
        {#if activeTab === "account"}
          <span
            class="absolute {sideRight ? 'right-0 rounded-l-[var(--radius-sm)]' : 'left-0 rounded-r-[var(--radius-sm)]'} top-1.5 bottom-1.5 w-[3px]"
            style:background="var(--accent)"
          ></span>
        {/if}
        {#key activeAvatar + String(activeAvatarHeadFromTexture)}
          {#if activeAvatarHeadFromTexture}
            <SkinHeadAvatar
              src={activeAvatar}
              size={compact ? 24 : 28}
              alt="Аватар"
              wrapperClass="rounded-[4px] shrink-0"
            />
          {:else}
            <img
              src={activeAvatar}
              alt="Avatar"
              class="shrink-0 rounded-[4px] object-cover {compact ? 'w-6 h-6' : 'w-7 h-7'}"
              style:image-rendering="pixelated"
            />
          {/if}
        {/key}
        {#if !compact}
          <div class="flex flex-col items-start min-w-0 flex-1">
            <span class="text-[13px] font-semibold leading-tight truncate w-full" style:color="var(--text)"
              >{activeAccount ? activeAccount.username : "Offline"}</span
            >
            <span
              class="text-[10px] leading-tight uppercase tracking-wide truncate w-full"
              style:color="var(--text-secondary)"
              >{activeAccount ? activeAccount.acc_type : "не в сети"}</span
            >
          </div>
        {/if}
      </button>

      <button
        type="button"
        on:click={onToggleDensity}
        title={compact ? "Развернуть подписи" : "Компактнее"}
        aria-label={compact ? "Развернуть подписи" : "Компактнее"}
        class="w-full flex items-center rounded-[var(--radius-sm)] transition-colors duration-150 text-[var(--text-secondary)] hover:text-[var(--text)] hover:bg-[var(--surface-hover)] {compact
          ? 'justify-center h-9'
          : 'gap-2 pl-3 pr-2 py-1.5 text-[12px]'}"
      >
        {#if layout.startsWith("sidebar_right")}
          {#if compact}
            <span class="inline-flex scale-x-[-1]"><PanelLeftOpen size={16} strokeWidth={2} /></span>
          {:else}
            <span class="inline-flex scale-x-[-1]"><PanelLeftClose size={14} strokeWidth={2} /></span>
            <span class="truncate">Ширина</span>
          {/if}
        {:else}
          {#if compact}
            <PanelLeftOpen size={16} strokeWidth={2} />
          {:else}
            <PanelLeftClose size={14} strokeWidth={2} />
            <span>Плотнее</span>
          {/if}
        {/if}
      </button>
    </div>
  </aside>
{/if}
