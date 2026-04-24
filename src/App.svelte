<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { fade, fly } from "svelte/transition";
  import Titlebar from "./Titlebar.svelte";
  import HomeTab from "./tabs/HomeTab.svelte";
  import NewsTab from "./tabs/NewsTab.svelte";
  import AccountTab from "./tabs/AccountTab.svelte";
  import SkinsTab from "./tabs/SkinsTab.svelte";
  import SettingsTab from "./tabs/SettingsTab.svelte";
  import AdvancedSettingsTab from "./tabs/AdvancedSettingsTab.svelte";
  import DiscoverTab from "./tabs/DiscoverTab.svelte";
  import LibraryTab from "./tabs/LibraryTab.svelte";
  import ChatTab from "./tabs/ChatTab.svelte";
  import InternalBrowserModal from "./components/InternalBrowserModal.svelte";
  import SplashScreen from "./SplashScreen.svelte";
  import OnboardingWizard from "./OnboardingWizard.svelte";
  import ChromeNavigation from "./components/ChromeNavigation.svelte";
  import CommandPalette from "./components/CommandPalette.svelte";
  import { registerCommands } from "./lib/commandRegistry";
  import { openInternalBrowser } from "./lib/internalBrowser";
  import {
    applyTheme,
    applyVisualPreset,
    normalizeVisualPreset,
    applyShellLayout,
    normalizeShellLayout,
  } from "./lib/themeApply";
  import { resolveBackgroundImageSrc } from "./lib/localImageUrl";
  import { initSettingsStore } from "./lib/settingsStore";
  import { registerIngameOverlayHotkey } from "./lib/ingameOverlayHotkey";
  import type { LibraryTabProps } from "./lib/libraryTabTypes";
  import {
    migrateChromeLayout,
    applyChromeDocumentAttrs,
    toggleSidebarDensity,
    sidebarStyleFromLayout,
    type ChromeLayout,
    type ModalPreset,
    type DownloadCorner,
  } from "./lib/chromeLayout";
  import {
    Home,
    Library,
    Compass,
    Settings,
    Shirt,
    LoaderCircle,
    Info,
    Newspaper,
    Sliders,
    MessageCircle,
    User,
    Palette,
    Package,
    ExternalLink,
    RefreshCw,
    FolderOpen,
    Monitor,
  } from "lucide-svelte";

  let showAdvancedTab = false;
  let showFriendsChatTab = false;
  let jentlememesApiBaseUrl = "https://jentlememes.ru";
  let chatProfileMcServer = false;
  let reduceMotion = false;
  let ingameOverlayEnabled = false;
  let ingameOverlayHotkey = "Alt+Backquote";
  let uiScale = 1.05;
  let chromeLayout: ChromeLayout = "sidebar_left_expanded";
  let modalPreset: ModalPreset = "minimal";
  let downloadCorner: DownloadCorner = "bl";

  $: dlCornerClass =
    downloadCorner === "bl"
      ? "bottom-6 left-6"
      : downloadCorner === "br"
        ? "bottom-6 right-6"
        : downloadCorner === "tl"
          ? "jm-dl-corner-tl"
          : downloadCorner === "tr"
            ? "jm-dl-corner-tr"
            : "hidden";

  /** Уведомления — тот же угол, что и виджет загрузки (если виджет скрыт — левый нижний). */
  $: toastCornerClass =
    downloadCorner === "hidden" ? "bottom-6 left-6" : dlCornerClass === "hidden" ? "bottom-6 left-6" : dlCornerClass;

  $: primaryTabs = [
    { id: "home" as const, label: "Главная", Icon: Home },
    { id: "news" as const, label: "Новости", Icon: Newspaper },
    { id: "library" as const, label: "Сборки", Icon: Library },
    { id: "skins" as const, label: "Скины", Icon: Shirt },
    { id: "discover" as const, label: "Браузер", Icon: Compass },
    ...(showFriendsChatTab
      ? [{ id: "chat" as const, label: "Чат", Icon: MessageCircle }]
      : []),
  ];

  $: systemTabs = [
    ...(showAdvancedTab
      ? [{ id: "advanced" as const, label: "Расширенные", Icon: Sliders }]
      : []),
    { id: "settings" as const, label: "Настройки", Icon: Settings },
  ];

  $: if (!showAdvancedTab && activeTab === "advanced") activeTab = "settings";
  $: if (!showFriendsChatTab && activeTab === "chat") activeTab = "settings";

  let activeTab = "home";
  let pendingInstanceId: string | undefined = undefined;
  let pendingServerIp: string | undefined = undefined;
  let pendingWorldName: string | undefined = undefined;
  let activeAccount: any = null;
  let activeAvatar = "https://minotar.net/helm/Steve/32.png";
  /** Полная текстура (локальный скин / сессия) — рисуем только голову в SkinHeadAvatar */
  let activeAvatarHeadFromTexture = false;
  /** Сбрасывает кеш текстуры после смены скина / профиля */
  let avatarCacheBust = 0;

  function withAvatarCacheBust(url: string): string {
    if (!url || !/^https?:\/\//i.test(url)) return url;
    const sep = url.includes("?") ? "&" : "?";
    return `${url}${sep}_jmav=${avatarCacheBust}`;
  }

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
  let bgSrc = "";
  let bgResolveSeq = 0;
  /** 12…98: непрозрачность слоя темы поверх картинки (меньше — фон ярче). */
  let bgDimPercent = 78;
  /** 78…100: непрозрачность карточек/панелей при фоне.
   *
   * Эволюция: 40 → 60 → 78. При 60 % на ярких обоях плитки всё ещё просвечивали
   * настолько, что текст читался плохо, а интерфейс выглядел «пустым». При 78 %
   * плитки остаются ощутимо полупрозрачными (видно нижнюю картинку как лёгкое
   * свечение), но сохраняют читаемость и каркас. Если нужен более сильный
   * «стеклянный» эффект — это делается через `.jm-glass` (backdrop-filter),
   * а не через alpha-канал. */
  let uiPanelOpacityPercent = 96;
  let ready = false;
  let splashMessage = "Загрузка лаунчера…";
  const splashPhases = ["Настройки", "Onboarding", "Сессии", "Профиль"];
  let splashPhaseIndex = 0;
  let needsOnboarding = false;
  let onboardingResolved = false;
  let updateInfo: any = null;
  let launcherUpdateDownloading = false;
  let launcherUpdateProgress = 0;
  let launcherUpdateProgressTimer: ReturnType<typeof setInterval> | null = null;

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

  async function applyIngameOverlayHotkey() {
    await registerIngameOverlayHotkey(ingameOverlayEnabled, ingameOverlayHotkey);
  }

  function applyUiScale(scale: number) {
    try {
      document.documentElement.style.setProperty("--ui-scale", String(scale));
    } catch {
      /* ignore */
    }
  }

  async function persistChromeSettings() {
    try {
      const current: any = await invoke("load_settings");
      await invoke("save_settings", {
        settings: {
          ...current,
          chrome_layout: chromeLayout,
          sidebar_style: sidebarStyleFromLayout(chromeLayout),
        },
      });
    } catch {
      /* ignore */
    }
  }

  async function toggleSidebarStyle() {
    const next = toggleSidebarDensity(chromeLayout);
    if (next !== chromeLayout) {
      chromeLayout = next;
      void persistChromeSettings();
    }
  }

  function setActiveTab(id: string) {
    activeTab = id;
  }

  $: applyChromeDocumentAttrs(chromeLayout, modalPreset);

  function syncWallpaperUiChrome(path: string, panelOp: number) {
    try {
      const root = document.documentElement;
      if (!path.trim()) {
        root.classList.remove("jm-wallpaper-ui");
        root.style.removeProperty("--jm-panel-opacity");
        return;
      }
      const op = Math.min(100, Math.max(78, panelOp)) / 100;
      root.style.setProperty("--jm-panel-opacity", String(op));
      root.classList.add("jm-wallpaper-ui");
    } catch {
      /* ignore */
    }
  }

  $: syncWallpaperUiChrome(bgPath, uiPanelOpacityPercent);

  $: {
    const seq = ++bgResolveSeq;
    const p = bgPath;
    if (!p) {
      bgSrc = "";
    } else {
      resolveBackgroundImageSrc(p).then((s) => {
        if (seq === bgResolveSeq) bgSrc = s;
      });
    }
  }

  async function loadSettings() {
    try {
      const s: any = await invoke("load_settings");
      showAdvancedTab = !!s.show_advanced_tab;
      showFriendsChatTab = !!s.show_friends_chat_tab;
      jentlememesApiBaseUrl = String(s.jentlememes_api_base_url || "https://jentlememes.ru").replace(
        /\/$/,
        "",
      );
      chatProfileMcServer = !!s.chat_profile_mc_server;
      reduceMotion = !!s.reduce_motion;
      ingameOverlayEnabled = !!s.ingame_overlay_enabled;
      ingameOverlayHotkey =
        String(s.ingame_overlay_hotkey || "Alt+Backquote").trim() || "Alt+Backquote";
      const rawScale = typeof s.ui_scale === "number" ? s.ui_scale : parseFloat(s.ui_scale || "1.05");
      uiScale = Number.isFinite(rawScale) ? Math.min(1.6, Math.max(0.85, rawScale)) : 1.05;
      chromeLayout = migrateChromeLayout(s.chrome_layout, s.sidebar_style);
      const mp = String(s.modal_preset || "minimal");
      modalPreset = (["minimal", "glass", "dense", "sheet"].includes(mp) ? mp : "minimal") as ModalPreset;
      const dc = String(s.download_corner || "bl");
      downloadCorner = (["bl", "br", "tl", "tr", "hidden"].includes(dc) ? dc : "bl") as DownloadCorner;
      applyUiScale(uiScale);
      applyVisualPreset(normalizeVisualPreset(s.visual_preset));
      applyShellLayout(normalizeShellLayout(s.shell_layout));
      const t = s.theme || "jentle-dark";
      bgPath = s.background || "";
      const rawDim =
        typeof s.background_dim_percent === "number"
          ? s.background_dim_percent
          : Number.parseInt(String(s.background_dim_percent ?? ""), 10);
      bgDimPercent = Number.isFinite(rawDim)
        ? Math.min(98, Math.max(12, Math.round(rawDim)))
        : 78;
      const rawPanel =
        typeof s.ui_panel_opacity_percent === "number"
          ? s.ui_panel_opacity_percent
          : Number.parseInt(String(s.ui_panel_opacity_percent ?? ""), 10);
      // Одноразовая миграция: старые дефолты были 60 / 82 / 92, а при них плитки на
      // обоях реально просвечивают. Любое значение <88 трактуем как «из старого
      // дефолта» и поднимаем до нового 96. Это не ломает пользователя, который
      // намеренно поставил, например, 95 — мы не трогаем значения ≥88.
      const panelResolved = Number.isFinite(rawPanel) ? Math.round(rawPanel) : 96;
      const panelMigrated = panelResolved < 88 ? 96 : panelResolved;
      uiPanelOpacityPercent = Math.min(100, Math.max(78, panelMigrated));
      if (panelMigrated !== panelResolved) {
        // Сохраняем миграцию в бэкенд, чтобы при следующем старте не переделывать.
        invoke("patch_settings", {
          delta: { ui_panel_opacity_percent: uiPanelOpacityPercent },
        }).catch(() => {});
      }
      await applyTheme(t, s.background || "");
      await applyIngameOverlayHotkey();
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
          if (skin && skin.skin_type !== "local") {
            const nick = String(skin.skin_data || skin.username || "").trim();
            if (nick) {
              try {
                const raw: any = await invoke("resolve_skin_texture_by_username", {
                  username: nick,
                });
                if (raw?.url) {
                  activeAvatar = withAvatarCacheBust(String(raw.url));
                  activeAvatarHeadFromTexture = true;
                  return;
                }
              } catch {
                /* ниже — сессия / helm */
              }
            }
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
              avatarUrl = withAvatarCacheBust(String(raw.url));
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
    const next = [{ id, msg }, ...toasts];
    toasts = next.length > 3 ? next.slice(0, 3) : next;
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

  function registerAppCommands() {
    return registerCommands([
      {
        id: "goto.home",
        title: "Главная",
        group: "Навигация",
        icon: Home,
        keywords: ["home", "dashboard", "главная"],
        run: () => (activeTab = "home"),
      },
      {
        id: "goto.library",
        title: "Сборки",
        group: "Навигация",
        icon: Library,
        keywords: ["library", "instances", "сборки"],
        run: () => (activeTab = "library"),
      },
      {
        id: "goto.discover",
        title: "Обзор модпаков и модов",
        group: "Навигация",
        icon: Compass,
        keywords: ["discover", "browse", "modrinth", "curseforge"],
        run: () => (activeTab = "discover"),
      },
      {
        id: "goto.skins",
        title: "Скины",
        group: "Навигация",
        icon: Shirt,
        run: () => (activeTab = "skins"),
      },
      {
        id: "goto.news",
        title: "Новости",
        group: "Навигация",
        icon: Newspaper,
        run: () => (activeTab = "news"),
      },
      {
        id: "goto.account",
        title: "Аккаунт",
        group: "Навигация",
        icon: User,
        run: () => (activeTab = "account"),
      },
      {
        id: "goto.settings",
        title: "Настройки",
        group: "Навигация",
        icon: Settings,
        shortcut: "Ctrl+,",
        run: () => (activeTab = "settings"),
      },
      {
        id: "goto.appearance",
        title: "Настройки оформления",
        description: "Пресеты, тема, обои",
        group: "Навигация",
        icon: Palette,
        run: () => {
          activeTab = "settings";
          setTimeout(() => {
            const btn = document.querySelector<HTMLButtonElement>(
              "[data-section='appearance']",
            );
            btn?.click();
          }, 60);
        },
      },
      {
        id: "action.refresh-ms",
        title: "Обновить сессии Microsoft",
        group: "Действия",
        icon: RefreshCw,
        run: async () => {
          try {
            await invoke("refresh_microsoft_sessions_startup");
            pushToast("Microsoft-сессии обновлены");
          } catch (e) {
            pushToast(`Ошибка: ${e}`);
          }
        },
      },
      {
        id: "action.check-update",
        title: "Проверить обновление лаунчера",
        group: "Действия",
        icon: Package,
        run: async () => {
          try {
            const upd: any = await invoke("check_launcher_update");
            if (upd?.available) {
              updateInfo = upd;
              pushToast(`Доступно обновление ${upd.latest}`);
            } else {
              pushToast("Вы используете актуальную версию");
            }
          } catch (e) {
            pushToast(`Ошибка: ${e}`);
          }
        },
      },
      {
        id: "action.open-data-dir",
        title: "Открыть папку данных",
        group: "Действия",
        icon: FolderOpen,
        run: async () => {
          try {
            await invoke("open_launcher_data_folder");
          } catch (e) {
            pushToast(`Ошибка: ${e}`);
          }
        },
      },
      {
        id: "action.open-overlay",
        title: "Открыть игровой оверлей",
        group: "Действия",
        icon: Monitor,
        run: async () => {
          if (!ingameOverlayEnabled) {
            pushToast("Включите оверлей в Настройках → Расширенные");
            return;
          }
          const { toggleIngameOverlay } = await import("./lib/ingameOverlayToggle");
          await toggleIngameOverlay();
        },
      },
      {
        id: "action.open-site",
        title: "Открыть сайт jentlememes.ru",
        group: "Действия",
        icon: ExternalLink,
        run: async () => {
          openInternalBrowser("https://jentlememes.ru/");
        },
      },
    ]);
  }

  onMount(() => {
    const splashStart = performance.now();
    void initSettingsStore();
    const unregisterCommands = registerAppCommands();
    void (async () => {
      splashPhaseIndex = 0;
      splashMessage = "Применяем настройки…";
      await loadSettings();
      try {
        splashPhaseIndex = 1;
        splashMessage = "Проверяем первый запуск…";
        const pending = await invoke<boolean>("is_onboarding_pending");
        needsOnboarding = !!pending;
      } catch {
        needsOnboarding = false;
      }
      onboardingResolved = true;
      splashPhaseIndex = 2;
      splashMessage = "Обновляем сессии…";
      try {
        await invoke("refresh_microsoft_sessions_startup");
      } catch {
        /* ignore */
      }
      splashPhaseIndex = 3;
      splashMessage = "Загружаем профиль…";
      await loadActiveAccount();
      splashPhaseIndex = 4;
      const elapsed = performance.now() - splashStart;
      const minSplashMs = reduceMotion ? 0 : 650;
      const wait = Math.max(0, minSplashMs - elapsed);
      setTimeout(() => (ready = true), wait);
    })();

    const unsubs: Array<() => void> = [];
    listen("profiles_updated", () => {
      avatarCacheBust++;
      void loadActiveAccount();
    }).then((u) => unsubs.push(u));
    listen("settings_updated", () => void loadSettings()).then((u) => unsubs.push(u));

    let activeMsTimer: ReturnType<typeof setInterval> | null = setInterval(() => {
      if (document.visibilityState !== "visible") return;
      try {
        if (!document.hasFocus()) return;
      } catch {
        /* some platforms */
      }
      void invoke("add_launcher_active_ms", { deltaMs: 60_000 }).catch(() => {});
    }, 60_000);

    const onJmTheme = (e: Event) => {
      const d = (e as CustomEvent<{ theme?: string; bg?: string }>).detail;
      if (d && "bg" in d) bgPath = d.bg || "";
    };
    window.addEventListener("jm_theme", onJmTheme);
    listen<any>("download_progress", (e) => {
      const p = e.payload;
      if (p.silent) {
        if (!p.total && !p.downloaded) {
          progress = { task_name: "", downloaded: 0, total: 0 };
          busyInstanceId = null;
        }
        return;
      }
      progress = p;
      if (p.instance_id) busyInstanceId = p.instance_id;
      if (p.total > 0 && p.downloaded >= p.total) {
        setTimeout(() => (busyInstanceId = null), 500);
      }
    }).then((u) => unsubs.push(u));

    const handleToast = (e: Event) => pushToast((e as CustomEvent).detail);
    window.addEventListener("jm_toast", handleToast);

    const handleOpenUrl = (e: Event) => {
      const d = (e as CustomEvent<{ url?: string }>).detail;
      const u = d?.url?.trim();
      if (u) openInternalBrowser(u);
    };
    window.addEventListener("jm_open_url", handleOpenUrl);

    return () => {
      import("@tauri-apps/plugin-global-shortcut")
        .then((m) => m.unregisterAll())
        .catch(() => {});
      unsubs.forEach((f) => f());
      window.removeEventListener("jm_open_url", handleOpenUrl);
      window.removeEventListener("jm_toast", handleToast);
      window.removeEventListener("jm_theme", onJmTheme);
      unregisterCommands();
      if (activeMsTimer) clearInterval(activeMsTimer);
    };
  });
</script>

<div
  class="jm-app-shell flex flex-col h-screen overflow-hidden font-sans rounded-xl border border-[var(--border)] shadow-sm relative {reduceMotion
    ? 'jm-reduce-motion'
    : ''}"
  class:bg-jm-bg={!bgPath}
  style:color="var(--text)"
>
  <Titlebar ingameOverlayEnabled={ingameOverlayEnabled} />

  {#if bgPath}
    <div class="absolute inset-0 z-0">
      <img src={bgSrc} alt="" class="w-full h-full object-cover" />
      <div
        class="absolute inset-0 bg-jm-bg pointer-events-none"
        style:opacity={bgDimPercent / 100}
        aria-hidden="true"
      ></div>
    </div>
  {/if}

  <div class="flex flex-col flex-1 min-h-0 relative z-[1]">
    {#if chromeLayout === "top_tabs"}
      <ChromeNavigation
        layout={chromeLayout}
        activeTab={activeTab}
        primaryTabs={primaryTabs}
        systemTabs={systemTabs}
        activeAccount={activeAccount}
        activeAvatar={activeAvatar}
        activeAvatarHeadFromTexture={activeAvatarHeadFromTexture}
        onTab={setActiveTab}
        onToggleDensity={toggleSidebarStyle}
      />
    {/if}

    <div class="flex flex-1 min-h-0 min-w-0">
      {#if chromeLayout.startsWith("sidebar")}
        <ChromeNavigation
          layout={chromeLayout}
          activeTab={activeTab}
          primaryTabs={primaryTabs}
          systemTabs={systemTabs}
          activeAccount={activeAccount}
          activeAvatar={activeAvatar}
          activeAvatarHeadFromTexture={activeAvatarHeadFromTexture}
          onTab={setActiveTab}
          onToggleDensity={toggleSidebarStyle}
        />
      {/if}


    <!-- ═══ Right column: update banner + content ═══ -->
    <div class="flex flex-col flex-1 min-w-0 min-h-0">
      {#if updateInfo}
        <div class="mx-3 mt-2 shrink-0 z-[10040] relative" transition:fade>
          <div
            class="p-3 rounded-lg border border-jm-accent/40 bg-jm-accent/10 flex flex-col gap-2"
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

      <main class="flex-grow relative overflow-hidden min-h-0" class:bg-jm-bg={!bgPath}>
    {#key activeTab}
      <div
        in:fade={{ duration: reduceMotion ? 0 : 100 }}
        class="absolute inset-0 overflow-hidden jm-tab-panel"
        class:hidden={showFriendsChatTab && activeTab === "chat"}
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
        {:else if activeTab === "advanced"}
          <AdvancedSettingsTab />
        {:else if activeTab === "account"}
          <AccountTab />
        {/if}
      </div>
    {/key}
    {#if showFriendsChatTab}
      <div
        class="absolute inset-0 flex flex-col min-h-0 overflow-hidden z-[2] {activeTab === 'chat'
          ? 'pointer-events-auto'
          : 'pointer-events-none'}"
        class:hidden={activeTab !== "chat"}
        aria-hidden={activeTab !== "chat"}
      >
        <ChatTab
          apiBaseUrl={jentlememesApiBaseUrl}
          chatProfileMcServer={chatProfileMcServer}
          chatChromeVisible={activeTab === "chat"}
          onNavigateLibraryWithServer={(instanceId, serverIp) => {
            pendingInstanceId = instanceId;
            pendingServerIp = serverIp;
            pendingWorldName = undefined;
            activeTab = "library";
          }}
          onOpenLibrary={() => {
            activeTab = "library";
          }}
        />
      </div>
    {/if}
      </main>
    </div>
  </div>
    {#if chromeLayout === "bottom_tabs"}
      <ChromeNavigation
        layout={chromeLayout}
        activeTab={activeTab}
        primaryTabs={primaryTabs}
        systemTabs={systemTabs}
        activeAccount={activeAccount}
        activeAvatar={activeAvatar}
        activeAvatarHeadFromTexture={activeAvatarHeadFromTexture}
        onTab={setActiveTab}
        onToggleDensity={toggleSidebarStyle}
      />
    {/if}
  </div>

  {#if showDownload && downloadCorner !== "hidden"}
    <div class="absolute inset-0 pointer-events-none z-[10058]" aria-hidden="true">
      <div
        class="absolute pointer-events-auto border border-jm-accent/50 bg-[var(--card)] rounded-full flex items-center transition-all duration-200 overflow-hidden {dlCornerClass} {isHoveringDL
          ? 'w-80 p-3 rounded-xl'
          : 'w-14 h-14 justify-center cursor-pointer'}"
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

  <div
    class="absolute z-[10058] flex flex-col-reverse gap-2 max-h-[min(40vh,14rem)] overflow-y-auto overflow-x-hidden pointer-events-auto custom-scrollbar {toastCornerClass}"
  >
    {#each toasts as t (t.id)}
      <div
        in:fly={{ x: -40, duration: 280, opacity: 0 }}
        out:fly={{ x: -48, duration: 220, opacity: 0 }}
        class="border border-[var(--border)] border-l-4 border-l-jm-accent p-3 rounded-lg flex items-center gap-3 bg-[var(--card)] jm-toast-glow"
      >
        <Info size={18} class="text-jm-accent" />
        <span class="text-sm font-medium" style:color="var(--text)">{t.msg}</span>
      </div>
    {/each}
  </div>

  <InternalBrowserModal />

  <CommandPalette />

  {#if !ready}
    <SplashScreen
      message={splashMessage}
      phases={splashPhases}
      phaseIndex={splashPhaseIndex}
    />
  {/if}

  {#if ready && onboardingResolved && needsOnboarding}
    <OnboardingWizard
      on:done={() => {
        needsOnboarding = false;
        void loadSettings();
      }}
    />
  {/if}
</div>
