<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { fly } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import { X, Minus } from "lucide-svelte";
  import WelcomePage from "./pages/WelcomePage.svelte";
  import PathPage from "./pages/PathPage.svelte";
  import ProgressPage from "./pages/ProgressPage.svelte";
  import FinishPage from "./pages/FinishPage.svelte";
  import UninstallPage from "./pages/UninstallPage.svelte";

  type Page = "welcome" | "path" | "progress" | "finish" | "uninstall";

  let page: Page = "welcome";
  let installPath = "";
  let mode: "install" | "uninstall" = "install";

  $: steps =
    mode === "uninstall"
      ? [
          { id: "uninstall" as const, label: "Удаление" },
        ]
      : [
          { id: "welcome" as const, label: "Старт" },
          { id: "path" as const, label: "Путь" },
          { id: "progress" as const, label: "Установка" },
          { id: "finish" as const, label: "Готово" },
        ];

  $: stepIndex = steps.findIndex((s) => s.id === page);

  onMount(() => {
    invoke<string>("get_mode").then((m) => {
      if (m === "uninstall") {
        mode = "uninstall";
        page = "uninstall";
      }
    });
    invoke<string>("get_default_path").then((p) => (installPath = p));
  });

  function handleDrag(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("button")) return;
    invoke("drag_window");
  }
</script>

<div
  class="relative flex flex-col h-screen overflow-hidden rounded-[var(--radius-lg)] border"
  style:background="var(--bg)"
  style:border-color="var(--border)"
>
  <!-- Ambient: тихое свечение от акцента -->
  <div class="absolute inset-0 pointer-events-none overflow-hidden">
    <div
      class="jm-orb absolute -top-24 left-1/2 w-[480px] h-[220px] rounded-full"
      style:background="radial-gradient(ellipse at center, color-mix(in srgb, var(--accent) 14%, transparent), transparent 70%)"
    ></div>
    <div
      class="absolute inset-x-0 bottom-0 h-[120px]"
      style:background="linear-gradient(to top, color-mix(in srgb, var(--accent) 6%, transparent), transparent)"
    ></div>
  </div>

  <!-- Titlebar -->
  <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
  <div
    role="banner"
    on:mousedown={handleDrag}
    class="relative z-20 flex items-center justify-between h-9 px-3 border-b select-none cursor-default"
    style:background="color-mix(in srgb, var(--bg) 95%, transparent)"
    style:border-color="var(--border)"
  >
    <span
      class="text-[11px] font-medium tracking-wide pointer-events-none"
      style:color="var(--text-secondary)"
    >
      {mode === "uninstall" ? "JentleMemes — удаление" : "JentleMemes — установка"}
    </span>
    <div class="flex items-center gap-0.5">
      <button
        type="button"
        class="jm-titlebar-btn"
        on:click={() => invoke("minimize_window")}
      >
        <Minus size={13} />
      </button>
      <button
        type="button"
        class="jm-titlebar-btn jm-titlebar-btn--close"
        on:click={() => invoke("close_window")}
      >
        <X size={13} />
      </button>
    </div>
  </div>

  <!-- Stepper (только для install) -->
  {#if mode === "install"}
    <div
      class="relative z-10 flex items-center gap-2 px-5 pt-3 pb-2"
    >
      {#each steps as s, i (s.id)}
        <div
          class="flex-1 h-1 rounded-full transition-colors"
          style:background={i <= stepIndex
            ? "var(--accent)"
            : "color-mix(in srgb, var(--text) 10%, transparent)"}
        ></div>
      {/each}
    </div>
  {/if}

  <div class="relative flex-1 overflow-hidden z-10">
    {#key page}
      <div
        class="absolute inset-0 flex flex-col"
        in:fly={{ x: 32, duration: 280, opacity: 0, easing: quintOut }}
        out:fly={{ x: -28, duration: 200, opacity: 0, easing: quintOut }}
      >
        {#if page === "welcome"}
          <WelcomePage onNext={() => (page = "path")} />
        {:else if page === "path"}
          <PathPage
            bind:installPath
            onBack={() => (page = "welcome")}
            onNext={() => (page = "progress")}
          />
        {:else if page === "progress"}
          <ProgressPage {installPath} onDone={() => (page = "finish")} />
        {:else if page === "finish"}
          <FinishPage {installPath} />
        {:else if page === "uninstall"}
          <UninstallPage />
        {/if}
      </div>
    {/key}
  </div>
</div>
