<script lang="ts">
  import { onMount, afterUpdate } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { fade, fly, scale } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import Titlebar from "./Titlebar.svelte";
  import HomeTab from "./tabs/HomeTab.svelte";
  import NewsTab from "./tabs/NewsTab.svelte";
  import AccountTab from "./tabs/AccountTab.svelte";
  import SkinsTab from "./tabs/SkinsTab.svelte";
  import SettingsTab from "./tabs/SettingsTab.svelte";
  import DiscoverTab from "./tabs/DiscoverTab.svelte";
  import LibraryTab from "./tabs/LibraryTab.svelte";
  import SkinHeadAvatar from "./components/SkinHeadAvatar.svelte";
  import { applyTheme } from "./lib/themeApply";
  import type { LibraryTabProps } from "./lib/libraryTabTypes";
  import {
    Home,
    Library,
    Compass,
    Settings,
    Shirt,
    LoaderCircle,
    Info,
    Newspaper,
  } from "lucide-svelte";

  const tabDefs = [
    { id: "home", label: "Главная", Icon: Home },
    { id: "news", label: "Новости", Icon: Newspaper },
    { id: "library", label: "Сборки", Icon: Library },
    { id: "skins", label: "Скины", Icon: Shirt },
    { id: "discover", label: "Браузер", Icon: Compass },
    { id: "settings", label: "Настройки", Icon: Settings },
  ] as const;

  let activeTab = "home";
  let pendingInstanceId: string | undefined = undefined;
  let pendingServerIp: string | undefined = undefined;
  let pendingWorldName: string | undefined = undefined;
  let activeAccount: any = null;
  let activeAvatar = "https://minotar.net/helm/Steve/32.png";
  /** Полная текстура (локальный скин / сессия) — рисуем только голову в SkinHeadAvatar */
  let activeAvatarHeadFromTexture = false;

  function inferAccType(acc: any): string {
    if (!acc) return "offline";
    const t = String(acc.acc_type || "").trim();
    if (t) return t;
    const id = String(acc.id || "");
    if (id.startsWith("ms-")) return "microsoft";
    if (id.startsWith("elyby-")) return "elyby";
    if (id.startsWith("offline-")) return "offline";
    return "offline";
  }
  let progress: {
    task_name: string;
    downloaded: number;
    total: number;
    instance_id?: string;
  } = { task_name: "", downloaded: 0, total: 0 };
  let busyInstanceId: string | null = null;
  let isHoveringDL = false;
  let toasts: { id: number; msg: string }[] = [];
  let bgPath = "";
  let ready = false;
  let updateInfo: any = null;
  let launcherUpdateDownloading = false;
  let launcherUpdateProgress = 0;
  let launcherUpdateProgressTimer: ReturnType<typeof setInterval> | null = null;

  let navEl: HTMLElement | undefined;

  let indicatorLeft = 0;
  let indicatorWidth = 0;

  function syncIndicator() {
    if (!navEl) return;
    if (activeTab === "account") {
      indicatorLeft = 0;
      indicatorWidth = 0;
      return;
    }
    const btn = navEl.querySelector(`[data-tab="${activeTab}"]`) as HTMLElement | null;
    if (btn) {
      indicatorLeft = btn.offsetLeft;
      indicatorWidth = btn.offsetWidth;
    }
  }

  function startLauncherUpdateProgressAnim() {
    launcherUpdateProgress = 6;
    if (launcherUpdateProgressTimer) clearInterval(launcherUpdateProgressTimer);
    launcherUpdateProgressTimer = setInterval(() => {
      launcherUpdateProgress = Math.min(
        94,
        launcherUpdateProgress + (2.5 + Math.random() * 5.5),
      );
    }, 320);
  }

  function stopLauncherUpdateProgressAnim() {
    if (launcherUpdateProgressTimer) {
      clearInterval(launcherUpdateProgressTimer);
      launcherUpdateProgressTimer = null;
    }
    launcherUpdateProgress = 0;
  }

  async function runLauncherUpdate() {
    if (launcherUpdateDownloading) return;
    launcherUpdateDownloading = true;
    startLauncherUpdateProgressAnim();
    try {
      await invoke("download_and_apply_update");
      launcherUpdateProgress = 100;
    } catch (e) {
      pushToast(`Ошибка: ${e}`);
    } finally {
      stopLauncherUpdateProgressAnim();
      launcherUpdateDownloading = false;
    }
  }

  afterUpdate(() => {
    syncIndicator();
  });

  async function loadSettings() {
    try {
      const s: any = await invoke("load_settings");
      const t = s.theme || "jentle-dark";
      bgPath = s.background || "";
      await applyTheme(t, s.background || "");
    } catch {
      /* ignore */
    }
    void invoke("check_launcher_update")
      .then((upd: any) => {
        if (upd?.available) updateInfo = upd;
      })
      .catch(() => {});
  }

  async function loadActiveAccount() {
    activeAvatarHeadFromTexture = false;
    try {
      const data: any = await invoke("load_profiles");
      const active = data.accounts.find((a: any) => a.id === data.active_account_id);
      activeAccount = active || null;
      if (active) {
        const uname = encodeURIComponent(String(active.username || "Steve").trim() || "Steve");
        let avatarUrl = `https://minotar.net/helm/${uname}/32.png`;
        if (active.active_skin_id) {
          const skin = data.skin_presets.find((p: any) => p.id === active.active_skin_id);
          if (skin?.skin_type === "local" && skin.skin_data) {
            activeAvatar = skin.skin_data;
            activeAvatarHeadFromTexture = true;
            return;
          }
        }
        const uuid = String(active.uuid || "").replace(/-/g, "");
        const t = inferAccType(active);
        if ((t === "elyby" || t === "microsoft") && uuid.length === 32) {
          try {
            const raw: any = await invoke("resolve_session_skin", {
              uuid: active.uuid,
              accountType: t,
              username: String(active.username || "").trim(),
            });
            if (raw?.url) {
              avatarUrl = String(raw.url);
              activeAvatarHeadFromTexture = true;
            }
          } catch {
            /* helm ниже */
          }
        }
        activeAvatar = avatarUrl;
      } else {
        activeAvatar = "https://minotar.net/helm/Steve/32.png";
      }
    } catch {
      /* ignore */
    }
  }

  function pushToast(msg: string) {
    const id = Date.now();
    toasts = [...toasts, { id, msg }];
    setTimeout(() => {
      toasts = toasts.filter((t) => t.id !== id);
    }, 3000);
  }

  $: libraryProps = {
    initialInstanceId: pendingInstanceId,
    initialServerIp: pendingServerIp,
    initialWorldName: pendingWorldName,
    onInstanceOpened: () => {
      pendingInstanceId = undefined;
    },
    onServerLaunchConsumed: () => {
      pendingServerIp = undefined;
    },
    onWorldLaunchConsumed: () => {
      pendingWorldName = undefined;
    },
    busyInstanceId,
    progress,
  } satisfies LibraryTabProps;

  $: percent =
    progress.total > 0 ? Math.round((progress.downloaded / progress.total) * 100) : 0;
  $: showDownload = progress.total > 0 && progress.downloaded < progress.total;

  onMount(() => {
    void (async () => {
      await loadSettings();
      try {
        await invoke("refresh_microsoft_sessions_startup");
      } catch {
        /* ignore */
      }
      await loadActiveAccount();
    })();
    setTimeout(() => (ready = true), 100);

    const unsubs: Array<() => void> = [];
    listen("profiles_updated", () => loadActiveAccount()).then((u) => unsubs.push(u));
    listen("settings_updated", () => loadSettings()).then((u) => unsubs.push(u));

    const onJmTheme = (e: Event) => {
      const d = (e as CustomEvent<{ theme?: string; bg?: string }>).detail;
      if (d && "bg" in d) bgPath = d.bg || "";
    };
    window.addEventListener("jm_theme", onJmTheme);
    listen<any>("download_progress", (e) => {
      const p = e.payload;
      if (p.silent) return;
      progress = p;
      if (p.instance_id) busyInstanceId = p.instance_id;
      if (p.total > 0 && p.downloaded >= p.total) {
        setTimeout(() => (busyInstanceId = null), 500);
      }
    }).then((u) => unsubs.push(u));

    const handleToast = (e: Event) => pushToast((e as CustomEvent).detail);
    window.addEventListener("jm_toast", handleToast);

    return () => {
      unsubs.forEach((f) => f());
      window.removeEventListener("jm_toast", handleToast);
      window.removeEventListener("jm_theme", onJmTheme);
    };
  });
</script>

<div
  class="jm-app-shell flex flex-col h-screen bg-jm-bg overflow-hidden font-sans rounded-xl border border-white/[0.08] shadow-2xl relative"
  style:color="var(--text)"
>
  <Titlebar />

  {#if bgPath}
    <div class="absolute inset-0 z-0">
      <img src={convertFileSrc(bgPath)} alt="" class="w-full h-full object-cover" />
      <div class="absolute inset-0 bg-jm-bg/70 backdrop-blur-sm"></div>
    </div>
  {/if}

  <div class="absolute inset-0 pointer-events-none overflow-hidden z-0 jm-ambient">
    <div
      class="jm-ambient-orb absolute -top-40 -left-40 w-[560px] h-[560px] bg-jm-accent/[0.075] rounded-full blur-[110px] spin-slow jm-breathe"
    ></div>
    <div
      class="jm-ambient-orb-delayed absolute top-1/3 -right-32 w-[420px] h-[420px] bg-jm-accent-light/[0.055] rounded-full blur-[100px] spin-slow"
      style:animation-duration="26s"
    ></div>
    <div
      class="jm-ambient-orb-slow absolute -bottom-60 -right-40 w-[680px] h-[680px] bg-jm-accent/[0.045] rounded-full blur-[150px] spin-slow"
      style:animation-direction="reverse"
      style:animation-duration="32s"
    ></div>
    <div
      class="absolute inset-0 opacity-[0.04] bg-[radial-gradient(ellipse_80%_50%_at_50%_-10%,rgba(var(--accent-rgb),0.5),transparent)]"
    ></div>
  </div>

  <header
    in:fly={{ y: -12, duration: 400 }}
    class="flex items-center justify-between px-3 md:px-6 py-2 glass border-b border-[var(--border)] shadow-lg z-[10050] shrink-0 gap-2 min-h-0 relative"
  >
    <div
      class="text-lg md:text-xl font-bold text-jm-accent-light tracking-wide shrink-0 hidden sm:flex items-baseline"
    >
      JentleMemes
    </div>

    <nav
      bind:this={navEl}
      class="jm-nav-pill relative flex bg-black/35 p-1 rounded-full border border-[var(--border)] shrink min-w-0 overflow-x-auto [&::-webkit-scrollbar]:hidden shadow-inner backdrop-blur-md"
    >
      <div
        class="absolute top-1 bottom-1 bg-gradient-to-r from-jm-accent/25 to-jm-accent/10 border border-jm-accent/35 rounded-full z-0 transition-[left,width,opacity] duration-500 cubic-bezier(0.22,1,0.36,1) shadow-[0_0_24px_rgba(var(--accent-rgb),0.15)] {activeTab ===
        'account'
          ? 'opacity-0 pointer-events-none'
          : 'opacity-100'}"
        style:left="{indicatorLeft}px"
        style:width="{indicatorWidth}px"
      ></div>
      {#each tabDefs as item (item.id)}
        <button
          type="button"
          data-tab={item.id}
          on:click={() => (activeTab = item.id)}
          class="relative z-10 flex items-center gap-1.5 px-3 py-1.5 rounded-full transition-all duration-300 whitespace-nowrap text-xs md:text-sm shrink-0 jm-tap-scale {activeTab === item.id
            ? 'text-jm-accent-light font-bold drop-shadow-[0_0_12px_rgba(var(--accent-rgb),0.35)]'
            : 'hover:text-jm-accent-light'}"
          style:color={activeTab === item.id ? undefined : "var(--text-secondary)"}
        >
          <svelte:component this={item.Icon} size={16} />
          <span class="hidden lg:inline">{item.label}</span>
        </button>
      {/each}
    </nav>

    <button
      type="button"
      on:click={() => (activeTab = "account")}
      class="flex items-center gap-2 px-2 py-1 pr-3 rounded-full border transition-all duration-300 shrink-0 jm-tap-scale backdrop-blur-sm {activeTab ===
      'account'
        ? 'border-jm-accent bg-jm-accent/15 shadow-[0_0_20px_rgba(var(--accent-rgb),0.2)]'
        : 'border-[var(--border)] bg-black/35 hover:border-jm-accent/45 hover:bg-jm-accent/5'}"
    >
      {#if activeAvatarHeadFromTexture}
        <SkinHeadAvatar
          src={activeAvatar}
          size={28}
          alt="Аватар"
          wrapperClass="rounded-full ring-2 ring-white/10 ring-offset-2 ring-offset-transparent"
        />
      {:else}
        <img
          src={activeAvatar}
          alt="Avatar"
          class="w-7 h-7 shrink-0 rounded-full object-cover ring-2 ring-white/10 ring-offset-2 ring-offset-transparent"
          style:image-rendering="pixelated"
        />
      {/if}
      <div class="hidden sm:flex flex-col items-start overflow-hidden">
        <span class="text-xs font-bold leading-tight truncate max-w-[100px] text-left"
          >{activeAccount ? activeAccount.username : "Offline"}</span
        >
        <span class="text-[9px] leading-tight uppercase" style:color="var(--text-secondary)"
          >{activeAccount ? activeAccount.acc_type : "..."}</span
        >
      </div>
    </button>
  </header>

  {#if updateInfo}
    <div class="mx-3 mt-1 overflow-hidden z-[10040] relative" transition:fade>
      <div
        class="p-3 rounded-xl border border-jm-accent/40 bg-jm-accent/10 backdrop-blur-md shadow-[0_8px_32px_rgba(0,0,0,0.25)] flex flex-col gap-2"
      >
        <div class="flex items-center gap-3 flex-wrap">
          <span class="text-sm font-bold flex-1 min-w-[10rem]"
            >Доступно обновление v{updateInfo.latest}</span
          >
          <button
            type="button"
            disabled={launcherUpdateDownloading}
            on:click={() => {
              pushToast("Загрузка обновления...");
              void runLauncherUpdate();
            }}
            class="bg-jm-accent text-black px-4 py-1.5 rounded-lg font-bold text-sm disabled:opacity-50 shrink-0 transition-transform hover:scale-[1.02] active:scale-[0.98]"
          >
            {launcherUpdateDownloading ? "Загрузка…" : "Обновить"}
          </button>
          <button
            type="button"
            disabled={launcherUpdateDownloading}
            on:click={() => (updateInfo = null)}
            class="text-jm-accent hover:text-jm-accent-light text-sm disabled:opacity-40">✕</button
          >
        </div>
        {#if launcherUpdateDownloading}
          <div class="w-full">
            <div
              class="h-2 rounded-full bg-black/30 border border-white/10 overflow-hidden jm-progress-indeterminate"
            >
              <div
                class="h-full rounded-full bg-gradient-to-r from-jm-accent to-jm-accent-light transition-[width] duration-300 ease-out"
                style:width="{Math.round(launcherUpdateProgress)}%"
              ></div>
            </div>
            <p class="text-[10px] mt-1 font-medium" style:color="var(--text-secondary)">
              Скачивание и применение обновления…
            </p>
          </div>
        {/if}
      </div>
    </div>
  {/if}

  <main
    class="flex-grow relative overflow-hidden"
    style:background="radial-gradient(ellipse at top, rgba(var(--accent-rgb),0.05) 0%, transparent 80%)"
  >
    {#key activeTab}
      <div
        in:scale={{ duration: 280, start: 0.985, opacity: 0.75, easing: quintOut }}
        class="absolute inset-0 overflow-hidden"
      >
        {#if activeTab === "news"}
          <NewsTab />
        {:else if activeTab === "home"}
          <HomeTab
            setActiveTab={(t) => (activeTab = t)}
            openInstance={(id) => {
              pendingInstanceId = id;
              pendingServerIp = undefined;
              pendingWorldName = undefined;
              activeTab = "library";
            }}
            onLaunchWithServer={(id, ip) => {
              pendingInstanceId = id;
              pendingServerIp = ip;
              pendingWorldName = undefined;
              activeTab = "library";
            }}
            onLaunchWorld={(id, worldName) => {
              pendingInstanceId = id;
              pendingServerIp = undefined;
              pendingWorldName = worldName;
              activeTab = "library";
            }}
          />
        {:else if activeTab === "library"}
          <LibraryTab {...libraryProps} />
        {:else if activeTab === "skins"}
          <SkinsTab />
        {:else if activeTab === "discover"}
          <DiscoverTab />
        {:else if activeTab === "settings"}
          <SettingsTab />
        {:else if activeTab === "account"}
          <AccountTab />
        {/if}
      </div>
    {/key}
  </main>

  {#if showDownload}
    <div class="absolute inset-0 pointer-events-none z-[10058]" aria-hidden="true">
      <div
        class="absolute bottom-6 left-6 pointer-events-auto glass border border-jm-accent shadow-[0_10px_30px_rgba(var(--accent-rgb),0.2)] rounded-full flex items-center transition-all duration-300 overflow-hidden {isHoveringDL
          ? 'w-80 p-3 rounded-2xl'
          : 'w-14 h-14 justify-center cursor-pointer glow-pulse'}"
        on:mouseenter={() => (isHoveringDL = true)}
        on:mouseleave={() => (isHoveringDL = false)}
        role="presentation"
      >
        {#if isHoveringDL}
          <div class="flex flex-col w-full px-2">
            <div class="flex justify-between items-center mb-2">
              <span class="text-xs font-bold text-white truncate pr-2"
                >{progress.task_name || "Загрузка..."}</span
              >
              <span class="text-xs text-jm-accent font-bold">{percent}%</span>
            </div>
            <div class="w-full bg-white/10 rounded-full h-2 overflow-hidden">
              <div
                class="bg-gradient-to-r from-jm-accent to-jm-accent-light h-full rounded-full progress-striped transition-all duration-300"
                style:width="{percent}%"
              ></div>
            </div>
            <div class="text-[10px] mt-1 text-right" style:color="var(--text-secondary)">
              {progress.downloaded} / {progress.total} файлов
            </div>
          </div>
        {:else}
          <div class="relative flex items-center justify-center w-full h-full">
            <svg
              class="w-full h-full transform -rotate-90 absolute inset-0 text-jm-accent/20"
              viewBox="0 0 36 36"
              ><circle cx="18" cy="18" r="16" fill="none" stroke-width="3" stroke="currentColor"
              ></circle></svg
            >
            <svg
              class="w-full h-full transform -rotate-90 absolute inset-0 text-jm-accent transition-all duration-200"
              viewBox="0 0 36 36"
              ><circle
                cx="18"
                cy="18"
                r="16"
                fill="none"
                stroke-width="3"
                stroke-dasharray="100"
                stroke-dashoffset={100 - percent}
                stroke-linecap="round"
                stroke="currentColor"
              ></circle></svg
            >
            <LoaderCircle size={16} class="text-jm-accent animate-spin" />
          </div>
        {/if}
      </div>
    </div>
  {/if}

  <div class="absolute bottom-6 right-6 z-[10058] flex flex-col gap-2">
    {#each toasts as t (t.id)}
      <div
        in:fly={{ x: 40, duration: 280, opacity: 0 }}
        out:fly={{ x: 48, duration: 220, opacity: 0 }}
        class="glass border-l-4 border-jm-accent p-4 rounded-xl shadow-2xl flex items-center gap-3 jm-toast-glow backdrop-blur-xl"
      >
        <Info size={18} class="text-jm-accent" />
        <span class="text-sm font-bold text-white">{t.msg}</span>
      </div>
    {/each}
  </div>

  {#if !ready}
    <div
      class="absolute inset-0 z-[99999] bg-jm-bg/95 backdrop-blur-md flex flex-col items-center justify-center gap-4"
      out:fade={{ duration: 450 }}
    >
      <div
        class="text-3xl font-black text-transparent bg-clip-text bg-gradient-to-r from-jm-accent-light via-white to-jm-accent animate-pulse tracking-tight"
      >
        JentleMemes
      </div>
      <div class="h-1 w-32 rounded-full bg-jm-accent/30 overflow-hidden">
        <div class="h-full w-1/2 rounded-full bg-jm-accent shimmer"></div>
      </div>
    </div>
  {/if}
</div>
