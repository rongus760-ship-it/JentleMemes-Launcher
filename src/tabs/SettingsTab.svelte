<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { slide } from "svelte/transition";
  import {
    Check,
    RefreshCw,
    Download,
    X,
    Wand2,
    Pencil,
    Plus,
    Sliders,
    Palette,
    Gamepad2,
    Settings as SettingsIcon,
    Terminal,
  } from "lucide-svelte";
  import { showToast } from "../lib/jmEvents";
  import {
    applyTheme,
    applyVisualPreset,
    normalizeVisualPreset,
    applyShellLayout,
    normalizeShellLayout,
    applyOverlayLayout,
    normalizeOverlayLayout,
    type VisualPreset,
    type ShellLayout,
    type OverlayLayout,
  } from "../lib/themeApply";
  import {
    invalidateBackgroundImageCache,
    resolveBackgroundImageSrc,
  } from "../lib/localImageUrl";
  import { patchSettings } from "../lib/settingsStore";
  import {
    builtinThemes,
    type ThemeDef,
    type CustomThemeDef,
    loadCustomThemes,
    saveCustomThemes,
  } from "../themes";
  import { LAUNCHER_VERSION } from "../version";
  import { openUrlInLauncher } from "../lib/jmOpenUrl";
  import ThemeEditorModal from "./settings/ThemeEditorModal.svelte";
  import Card from "../components/ui/Card.svelte";
  import SettingRow from "../components/ui/SettingRow.svelte";
  import SectionNav, { type NavItem } from "../components/ui/SectionNav.svelte";
  import Toggle from "../components/ui/Toggle.svelte";
  import Button from "../components/ui/Button.svelte";

  let settings: Record<string, any> = {
    ram_mb: 4096,
    jvm_args: "-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions",
    wrapper: "",
    close_on_launch: false,
    custom_java_path: "",
    show_news: true,
    download_dependencies: true,
    hybrid_provider_enabled: false,
    internal_file_browser: false,
    mod_provider: "modrinth",
    curseforge_api_key: "",
    theme: "jentle-dark",
    visual_preset: "blend",
    shell_layout: "classic",
    overlay_layout: "panel",
    background: "",
    /** @see LauncherSettings.background_dim_percent */
    background_dim_percent: 78,
    /** @see LauncherSettings.ui_panel_opacity_percent */
    ui_panel_opacity_percent: 96,
    discord_rich_presence: true,
    show_advanced_tab: false,
    linux_jvm_unlock_experimental: false,
    linux_lwjgl_openal_libname: false,
    linux_openal_lib_path: "/usr/lib/libopenal.so",
  };

  let maxRam = 8192;
  let backgrounds: string[] = [];
  let updateInfo: any = null;
  let checkingUpdate = false;
  let downloading = false;
  let customThemes: CustomThemeDef[] = [];
  let editorOpen = false;
  let editingTheme: CustomThemeDef | null = null;
  let runtimeOs = "";
  let dataDirCheckMsg = "";
  type SectionId = "general" | "appearance" | "game" | "linux" | "advanced";
  let activeSection: SectionId = "general";
  const selectSection = (id: string): void => {
    activeSection = id as SectionId;
  };

  // Под-секции внутри «Оформление» — разделяем длинный список карточек
  // на логические группы, чтобы вкладка не казалась перегруженной.
  type AppearanceTab = "style" | "layout" | "palette" | "wallpaper";
  let appearanceTab: AppearanceTab = "style";
  const appearanceTabs: { id: AppearanceTab; label: string }[] = [
    { id: "style", label: "Стиль" },
    { id: "layout", label: "Расположение" },
    { id: "palette", label: "Цвета" },
    { id: "wallpaper", label: "Обои" },
  ];

  $: currentTheme = settings.theme || "jentle-dark";
  $: currentPreset = normalizeVisualPreset(settings.visual_preset);

  type PresetMeta = {
    id: VisualPreset;
    name: string;
    desc: string;
    radius: number;
    density: number;
    hasGlow: boolean;
    hasBlur: boolean;
  };
  const presetsMeta: PresetMeta[] = [
    {
      id: "blend",
      name: "Blend",
      desc: "Гибрид — мягкие радиусы, тонкие тени, полу-прозрачные карточки. Рекомендуемый дефолт.",
      radius: 10,
      density: 1,
      hasGlow: false,
      hasBlur: false,
    },
    {
      id: "modrinth",
      name: "Modrinth",
      desc: "Просторный и воздушный: большие радиусы, больше отступов, минимум декора.",
      radius: 14,
      density: 1.08,
      hasGlow: false,
      hasBlur: false,
    },
    {
      id: "discord",
      name: "Discord",
      desc: "Плотный: малые радиусы, компактные списки, плоские поверхности. Идеально для «длинных» таблиц.",
      radius: 6,
      density: 0.88,
      hasGlow: false,
      hasBlur: false,
    },
    {
      id: "legacy",
      name: "Legacy 1.1.0",
      desc: "Портированный визуал старой версии лаунчера: pulse-glow, shimmer, gradient-рамки.",
      radius: 10,
      density: 1,
      hasGlow: true,
      hasBlur: false,
    },
    {
      id: "glass",
      name: "Glass",
      desc: "Acrylic — полупрозрачные карточки с backdrop-blur. Экспериментальный, дорогой по GPU.",
      radius: 12,
      density: 1.02,
      hasGlow: false,
      hasBlur: true,
    },
  ];

  async function quickSavePreset(preset: VisualPreset) {
    applyVisualPreset(preset);
    settings = { ...settings, visual_preset: preset };
    try {
      await patchSettings({ visual_preset: preset });
    } catch {
      /* ignore */
    }
  }

  type ShellMeta = {
    id: ShellLayout;
    name: string;
    desc: string;
    tag: string;
  };
  const shellsMeta: ShellMeta[] = [
    {
      id: "classic",
      name: "Classic",
      desc: "Титлбар сверху, сайдбар слева — всё как в 1.x. Надёжный дефолт.",
      tag: "safe",
    },
    {
      id: "dock-bottom",
      name: "Floating Dock",
      desc: "Плавающий центрированный док снизу. macOS / iPadOS-подобно, подпрыгивающие активные иконки.",
      tag: "wow",
    },
    {
      id: "split-rail",
      name: "Split Rail",
      desc: "Тонкий иконочный рельс слева с акцентной подсветкой + подкрашенный титлбар. Похоже на GitHub / Jetbrains.",
      tag: "pro",
    },
    {
      id: "command-only",
      name: "Command Center",
      desc: "Почти без дока. Рельс раскрывается по hover, основной ввод — Ctrl+K. Для тех, кто живёт в клавиатуре.",
      tag: "zen",
    },
    {
      id: "holo-arc",
      name: "Holo-Arc",
      desc: "Радиальный плавающий диск в правом нижнем углу. Сканирующее кольцо вокруг активной вкладки. Про отвал башки.",
      tag: "mind-blown",
    },
  ];

  async function quickSaveShell(layout: ShellLayout) {
    applyShellLayout(layout);
    settings = { ...settings, shell_layout: layout };
    try {
      await patchSettings({ shell_layout: layout });
    } catch {
      /* ignore */
    }
  }

  type OverlayMeta = {
    id: OverlayLayout;
    name: string;
    desc: string;
    tag: string;
  };
  const overlaysMeta: OverlayMeta[] = [
    {
      id: "panel",
      name: "Panel",
      desc: "Классика 2.0: виджеты-карточки, control panel, DnD. Безопасно в любой сцене.",
      tag: "default",
    },
    {
      id: "hud",
      name: "HUD",
      desc: "Компактная HUD-полоса, меньше отступов, приоритет игровой картинке.",
      tag: "compact",
    },
    {
      id: "radial",
      name: "Radial",
      desc: "Центральный крупный хаб и мелкие виджеты по периметру. Круговое внимание.",
      tag: "focus",
    },
    {
      id: "ticker",
      name: "Ticker",
      desc: "Узкая «бегущая» лента сверху — статы и чат проходят бегущей строкой.",
      tag: "twitch",
    },
    {
      id: "neon-grid",
      name: "Neon Grid",
      desc: "Киберпанк-сетка, sweep-подсветка, моно-шрифт, неон-свечение. Для катки с адреналином.",
      tag: "mind-blown",
    },
  ];

  async function quickSaveOverlay(layout: OverlayLayout) {
    applyOverlayLayout(layout);
    settings = { ...settings, overlay_layout: layout };
    try {
      await patchSettings({ overlay_layout: layout });
    } catch {
      /* ignore */
    }
  }

  $: currentShell = normalizeShellLayout(settings.shell_layout);
  $: currentOverlayLayout = normalizeOverlayLayout(settings.overlay_layout);
  $: navItems = [
    { id: "general", label: "Общие", icon: SettingsIcon },
    { id: "appearance", label: "Оформление", icon: Palette },
    { id: "game", label: "Игра", icon: Gamepad2 },
    ...(runtimeOs === "linux"
      ? [{ id: "linux", label: "Linux", icon: Terminal }]
      : []),
    { id: "advanced", label: "Расширенные", icon: Sliders },
  ] as NavItem[];

  function clampBgDim(v: unknown): number {
    const n = typeof v === "number" ? v : Number.parseInt(String(v ?? ""), 10);
    if (!Number.isFinite(n)) return 78;
    return Math.min(98, Math.max(12, Math.round(n)));
  }

  function clampPanelOpacity(v: unknown): number {
    const n = typeof v === "number" ? v : Number.parseInt(String(v ?? ""), 10);
    if (!Number.isFinite(n)) return 96;
    return Math.min(100, Math.max(78, Math.round(n)));
  }

  /** Сохраняем через patchSettings — атомарный merge на бэкенде, без гонки с другими вкладками. */
  async function saveWallpaperAppearance() {
    const d = clampBgDim(settings.background_dim_percent);
    const p = clampPanelOpacity(settings.ui_panel_opacity_percent);
    settings = { ...settings, background_dim_percent: d, ui_panel_opacity_percent: p };
    try {
      await patchSettings({ background_dim_percent: d, ui_panel_opacity_percent: p });
    } catch {
      /* ignore */
    }
  }

  async function verifyDataDirWritable() {
    dataDirCheckMsg = "";
    try {
      const path = await invoke<string>("verify_data_dir_writable");
      dataDirCheckMsg = path;
      showToast("Каталог данных доступен на запись");
    } catch (e: any) {
      showToast(String(e ?? "Ошибка проверки каталога"));
    }
  }

  import { listen } from "@tauri-apps/api/event";
  import { invalidateCustomThemesCache } from "../themes";
  import { onDestroy } from "svelte";
  let unlistenSettings: (() => void) | null = null;

  onMount(() => {
    invoke("load_settings").then((data: any) => {
      const merged = { ...settings, ...data };
      merged.background_dim_percent = clampBgDim(merged.background_dim_percent);
      merged.ui_panel_opacity_percent = clampPanelOpacity(merged.ui_panel_opacity_percent);
      settings = merged;
    });
    invoke("runtime_os")
      .then((os) => (runtimeOs = typeof os === "string" ? os : ""))
      .catch(() => (runtimeOs = ""));
    invoke("get_system_ram")
      .then((ram: any) => {
        if (ram && ram > 1024) maxRam = ram;
      })
      .catch(console.error);
    invoke("get_backgrounds").then((bgs: any) => (backgrounds = bgs || []));
    loadCustomThemes().then((t) => (customThemes = t));

    // Если настройки меняются из другого места (onboarding / overlay / другой
    // таб), подтягиваем снапшот и обновляем список кастомных тем. Без этого
    // после quickSavePreset/Shell/Overlay мог «ускользать» кастомный лист.
    void (async () => {
      try {
        unlistenSettings = await listen("settings_updated", async () => {
          try {
            const data: any = await invoke("load_settings");
            const merged = { ...settings, ...data };
            merged.background_dim_percent = clampBgDim(merged.background_dim_percent);
            merged.ui_panel_opacity_percent = clampPanelOpacity(merged.ui_panel_opacity_percent);
            settings = merged;
            invalidateCustomThemesCache();
            customThemes = await loadCustomThemes();
          } catch {
            /* ignore */
          }
        });
      } catch {
        /* ignore */
      }
    })();
  });

  onDestroy(() => {
    if (unlistenSettings) {
      try {
        unlistenSettings();
      } catch {
        /* ignore */
      }
      unlistenSettings = null;
    }
  });

  async function quickSaveThemeBg(theme: string, background: string) {
    const next = { ...settings, theme, background };
    settings = next;
    try {
      await patchSettings({ theme, background });
    } catch {
      /* ignore */
    }
  }

  /**
   * «Сохранить» теперь идёт через patch_settings с дельтой, а не через
   * save_settings с целым объектом. Раньше прямой save_settings перезаписывал
   * `custom_themes` старым снапшотом из локального state (его обновлял
   * `saveCustomThemes`, но в `settings` этот массив не синхронизировался) —
   * и пользователь терял только что созданные темы при любом клике «Сохранить».
   *
   * Перечень полей соответствует тому, что вкладка «Настройки» реально
   * управляет; остальное (custom_themes, visual_preset, shell/overlay_layout,
   * chrome_layout и т.д.) не затрагивается и остаётся на диске нетронутым.
   */
  async function handleSave() {
    try {
      const delta = {
        ram_mb: settings.ram_mb,
        jvm_args: settings.jvm_args,
        wrapper: settings.wrapper,
        close_on_launch: settings.close_on_launch,
        custom_java_path: settings.custom_java_path,
        show_news: settings.show_news,
        download_dependencies: settings.download_dependencies,
        hybrid_provider_enabled: settings.hybrid_provider_enabled,
        internal_file_browser: settings.internal_file_browser,
        mod_provider: settings.mod_provider,
        curseforge_api_key: settings.curseforge_api_key,
        theme: settings.theme,
        background: settings.background,
        background_dim_percent: settings.background_dim_percent,
        ui_panel_opacity_percent: settings.ui_panel_opacity_percent,
        discord_rich_presence: settings.discord_rich_presence,
        show_advanced_tab: settings.show_advanced_tab,
        linux_jvm_unlock_experimental: settings.linux_jvm_unlock_experimental,
        linux_lwjgl_openal_libname: settings.linux_lwjgl_openal_libname,
        linux_openal_lib_path: settings.linux_openal_lib_path,
      };
      await patchSettings(delta);
      await applyTheme(settings.theme, settings.background);
      showToast("Настройки сохранены!");
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  async function handleCheckUpdate() {
    checkingUpdate = true;
    try {
      const upd: any = await invoke("check_launcher_update");
      if (upd?.available) {
        updateInfo = upd;
      } else {
        updateInfo = null;
        showToast("Вы используете последнюю версию!");
      }
    } catch (e) {
      showToast(`Ошибка проверки: ${e}`);
    } finally {
      checkingUpdate = false;
    }
  }

  async function handleDownloadUpdate() {
    downloading = true;
    showToast("Загрузка обновления...");
    try {
      await invoke("download_and_apply_update");
    } catch (e) {
      showToast(`Ошибка: ${e}`);
      downloading = false;
    }
  }

  async function handleAddBackground() {
    try {
      const selected: string | null = await invoke("pick_image_file");
      if (selected) {
        const newPath: string = await invoke("copy_background", { sourcePath: selected });
        backgrounds = [...backgrounds, newPath];
        settings = { ...settings, background: newPath };
        await applyTheme(settings.theme, newPath);
        quickSaveThemeBg(settings.theme, newPath);
      }
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  function openCreateTheme() {
    editingTheme = null;
    editorOpen = true;
  }

  function openEditTheme(ct: CustomThemeDef) {
    editingTheme = ct;
    editorOpen = true;
  }

  async function handleEditorSave(theme: CustomThemeDef) {
    const et = editingTheme;
    const updated = et
      ? customThemes.map((t) => (t.id === et.id ? theme : t))
      : [...customThemes, theme];
    customThemes = updated;
    await saveCustomThemes(updated);
    // Синхронизируем локальный state с диском: без этого `handleSave`
    // (кнопка «Сохранить» в футере) пушил бы старый custom_themes и
    // стирал только что созданную тему.
    settings = { ...settings, theme: theme.id, custom_themes: updated };
    await patchSettings({ theme: theme.id });
    await applyTheme(theme.id, settings.background);
    editorOpen = false;
    editingTheme = null;
  }

  async function handleEditorDelete(id: string) {
    const updated = customThemes.filter((t) => t.id !== id);
    customThemes = updated;
    await saveCustomThemes(updated);
    if (settings.theme === id) {
      settings = { ...settings, theme: "jentle-dark", custom_themes: updated };
      await patchSettings({ theme: "jentle-dark" });
      await applyTheme("jentle-dark", settings.background);
    } else {
      settings = { ...settings, custom_themes: updated };
    }
    editorOpen = false;
    editingTheme = null;
  }

  async function handleEditorCancel() {
    await applyTheme(settings.theme, settings.background);
    editorOpen = false;
    editingTheme = null;
  }

  function deleteEditingTheme() {
    if (editingTheme) void handleEditorDelete(editingTheme.id);
  }

  function themeCardLabelColor(t: ThemeDef) {
    return t.isLight ? (t.preview.bg === "#f8f5ee" ? "#2a2518" : "#333") : "#ddd";
  }
</script>

<div class="jm-container flex h-full gap-6 pt-6">
  <!-- Sidebar nav -->
  <aside class="w-48 shrink-0 hidden md:flex flex-col gap-4">
    <div>
      <h2 class="text-lg font-semibold leading-tight">Настройки</h2>
      <p class="text-[11px]" style:color="var(--text-secondary)">v{LAUNCHER_VERSION}</p>
    </div>
    <SectionNav items={navItems} active={activeSection} onChange={selectSection} />
  </aside>

  <!-- Mobile nav -->
  <div class="md:hidden sticky top-0 z-10 w-full">
    <div class="ui-seg w-full overflow-x-auto flex">
      {#each navItems as item (item.id)}
        <button
          type="button"
          class="ui-seg-item"
          class:is-active={activeSection === item.id}
          on:click={() => selectSection(item.id)}
        >
          {item.label}
        </button>
      {/each}
    </div>
  </div>

  <!-- Content -->
  <div class="flex-1 min-w-0 overflow-y-auto custom-scrollbar pb-4 pr-1">
    {#if activeSection === "general"}
      <div class="flex flex-col gap-4">
        <Card title="Обновления" hint={updateInfo
          ? `Доступна v${updateInfo.latest} (текущая: v${updateInfo.current})`
          : "Вы используете последнюю версию"}>
          <svelte:fragment slot="action">
            <Button variant="subtle" size="sm" on:click={handleCheckUpdate} disabled={checkingUpdate}>
              <RefreshCw size={13} strokeWidth={2} class={checkingUpdate ? "animate-spin" : ""} />
              Проверить
            </Button>
            {#if updateInfo}
              <Button variant="primary" size="sm" on:click={handleDownloadUpdate} disabled={downloading}>
                <Download size={13} strokeWidth={2.2} />
                {downloading ? "Загрузка…" : "Обновить"}
              </Button>
            {/if}
          </svelte:fragment>
          {#if updateInfo?.changelog}
            <div
              transition:slide={{ duration: 200 }}
              class="p-3 rounded-[var(--radius)] border border-[var(--border)] text-xs whitespace-pre-wrap mt-0"
              style:background="var(--surface-1)"
              style:color="var(--text-secondary)"
            >
              {updateInfo.changelog}
            </div>
          {/if}
        </Card>

        <Card padding="p-0">
          <SettingRow label="Новости на главной" hint="Показывать блок новостей на домашней вкладке">
            <Toggle bind:checked={settings.show_news} ariaLabel="Новости" />
          </SettingRow>
          <SettingRow
            label="Скачивать зависимости автоматически"
            hint="Авто-установка требуемых библиотек/модов при запуске"
          >
            <Toggle bind:checked={settings.download_dependencies} ariaLabel="Автозагрузка" />
          </SettingRow>
          <SettingRow
            label="Discord Rich Presence"
            hint="Статус в Discord на время игры (требуется Application ID, см. README)"
          >
            <Toggle bind:checked={settings.discord_rich_presence} ariaLabel="Discord" />
          </SettingRow>
        </Card>
      </div>
    {:else if activeSection === "appearance"}
      <div class="flex flex-col gap-4">
        <!-- Внутренний табулятор «Оформление» — вместо одного длинного списка -->
        <div
          class="flex gap-1 p-1 rounded-[var(--radius)] border shrink-0 self-start"
          style:background="var(--surface-1)"
          style:border-color="var(--border)"
          role="tablist"
          aria-label="Разделы оформления"
        >
          {#each appearanceTabs as t (t.id)}
            <button
              type="button"
              role="tab"
              aria-selected={appearanceTab === t.id}
              on:click={() => (appearanceTab = t.id)}
              class="px-3 py-1.5 rounded-[var(--radius-sm)] text-[12px] font-medium transition-colors {appearanceTab ===
              t.id
                ? ''
                : 'hover:bg-[var(--surface-hover)]'}"
              style:background={appearanceTab === t.id ? "var(--surface-2)" : "transparent"}
              style:color={appearanceTab === t.id ? "var(--text)" : "var(--text-secondary)"}
            >
              {t.label}
            </button>
          {/each}
        </div>

      {#if appearanceTab === "style"}
        <Card
          title="Визуальный стиль"
          hint="Структурный пресет — задаёт радиусы, плотность, тени и анимации. Палитру выбирайте во вкладке «Цвета»."
        >
          <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5 gap-3">
            {#each presetsMeta as p (p.id)}
              {@const isActive = currentPreset === p.id}
              <button
                type="button"
                on:click={() => quickSavePreset(p.id)}
                class="relative group text-left rounded-[var(--radius)] border p-3 transition-all {isActive
                  ? 'border-[var(--accent)] ring-1 ring-[var(--accent-soft)]'
                  : 'border-[var(--border)] hover:border-[var(--border-strong)]'}"
                style:background="var(--surface-1)"
                aria-pressed={isActive}
              >
                <!-- Preview «fake card» that uses preset-specific visual metrics -->
                <div
                  class="relative w-full h-[72px] rounded-md overflow-hidden mb-3"
                  style:background="linear-gradient(135deg, color-mix(in srgb, var(--accent) 18%, var(--bg)) 0%, var(--bg) 100%)"
                  style:border-radius="{p.radius}px"
                >
                  <div
                    class="absolute inset-2 flex items-center gap-2"
                    style:border-radius="{Math.max(p.radius - 2, 4)}px"
                    style:background={p.hasBlur ? "color-mix(in srgb, var(--card) 55%, transparent)" : "var(--card)"}
                    style:backdrop-filter={p.hasBlur ? "blur(10px) saturate(1.3)" : ""}
                    style:border="1px solid color-mix(in srgb, var(--accent) 25%, var(--border))"
                    style:box-shadow={p.hasGlow
                      ? "0 0 14px rgba(var(--accent-rgb),0.25)"
                      : "var(--shadow-card)"}
                  >
                    <div
                      class="w-6 h-6 rounded-full shrink-0"
                      style:background="var(--accent)"
                      style:border-radius="{p.radius >= 10 ? '999px' : '4px'}"
                    ></div>
                    <div class="flex-1 flex flex-col gap-1 min-w-0">
                      <div
                        class="h-1.5 rounded-full"
                        style:width="70%"
                        style:background="color-mix(in srgb, var(--text) 35%, transparent)"
                      ></div>
                      <div
                        class="h-1 rounded-full"
                        style:width="40%"
                        style:background="color-mix(in srgb, var(--text) 20%, transparent)"
                      ></div>
                    </div>
                  </div>
                </div>
                <div class="flex items-center justify-between gap-2">
                  <div class="flex flex-col min-w-0">
                    <span class="font-semibold text-[13px] truncate" style:color="var(--text)">{p.name}</span>
                    <span class="text-[10px] font-mono uppercase tracking-wide" style:color="var(--text-secondary)">
                      r{p.radius} · d{p.density.toFixed(2)}{p.hasGlow ? " · glow" : ""}{p.hasBlur
                        ? " · blur"
                        : ""}
                    </span>
                  </div>
                  {#if isActive}
                    <div
                      class="w-5 h-5 rounded-full flex items-center justify-center shrink-0"
                      style:background="var(--accent)"
                    >
                      <Check size={12} class="text-black" strokeWidth={3} />
                    </div>
                  {/if}
                </div>
                <p class="text-[11px] mt-2 leading-snug" style:color="var(--text-secondary)">
                  {p.desc}
                </p>
              </button>
            {/each}
          </div>
        </Card>
      {/if}

      {#if appearanceTab === "layout"}
        <Card
          title="Схема интерфейса"
          hint="Радикально меняет расположение и оформление дока. Эти темы не трогают палитру — только структуру. Переключается мгновенно, без перезапуска."
        >
          <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5 gap-3">
            {#each shellsMeta as s (s.id)}
              {@const isActive = currentShell === s.id}
              <button
                type="button"
                on:click={() => quickSaveShell(s.id)}
                class="relative text-left rounded-[var(--radius)] border p-3 transition-all {isActive
                  ? 'border-[var(--accent)] ring-1 ring-[var(--accent-soft)]'
                  : 'border-[var(--border)] hover:border-[var(--border-strong)]'}"
                style:background="var(--surface-1)"
                aria-pressed={isActive}
              >
                <!-- Схематическое превью shell-layout'а -->
                <div
                  class="relative w-full h-[72px] rounded-md overflow-hidden mb-3 border"
                  style:background="color-mix(in srgb, var(--bg) 92%, transparent)"
                  style:border-color="var(--border)"
                >
                  {#if s.id === "classic"}
                    <div class="absolute inset-x-0 top-0 h-2" style:background="var(--surface-2)"></div>
                    <div class="absolute left-0 top-2 bottom-0 w-5" style:background="var(--surface-1)"></div>
                    <div class="absolute left-1 top-4 w-3 h-1.5 rounded" style:background="var(--accent)"></div>
                  {:else if s.id === "dock-bottom"}
                    <div class="absolute inset-x-3 bottom-2 h-3 rounded-full flex items-center justify-center gap-1"
                         style:background="color-mix(in srgb, var(--card) 88%, transparent)"
                         style:box-shadow="0 4px 14px -6px color-mix(in srgb, var(--accent) 60%, transparent)">
                      <span class="w-1.5 h-1.5 rounded-full" style:background="var(--accent)"></span>
                      <span class="w-1.5 h-1.5 rounded-full" style:background="var(--text-secondary)" style:opacity="0.5"></span>
                      <span class="w-1.5 h-1.5 rounded-full" style:background="var(--text-secondary)" style:opacity="0.5"></span>
                    </div>
                  {:else if s.id === "split-rail"}
                    <div class="absolute left-0 top-0 bottom-0 w-2.5" style:background="var(--surface-2)"></div>
                    <div class="absolute left-2.5 top-0 right-0 h-2" style:background="linear-gradient(90deg, transparent, color-mix(in srgb, var(--accent) 35%, transparent), transparent)"></div>
                    <div class="absolute left-0.5 top-4 w-1.5 h-4 rounded-sm" style:background="var(--accent)"></div>
                  {:else if s.id === "command-only"}
                    <div class="absolute left-0 top-0 bottom-0 w-1.5" style:background="color-mix(in srgb, var(--text-secondary) 25%, transparent)"></div>
                    <div class="absolute inset-x-0 top-2 flex justify-center">
                      <span class="text-[9px] font-mono px-2 py-0.5 rounded-md" style:background="var(--card)" style:color="var(--accent)" style:border="1px solid var(--accent)">Ctrl+K</span>
                    </div>
                  {:else if s.id === "holo-arc"}
                    <div class="absolute right-2 bottom-2 w-8 h-8 rounded-full"
                         style:background="radial-gradient(circle at 30% 30%, color-mix(in srgb, var(--accent) 28%, var(--card)) 0%, var(--card) 70%)"
                         style:box-shadow="0 0 0 2px color-mix(in srgb, var(--accent) 15%, transparent), 0 0 14px color-mix(in srgb, var(--accent) 55%, transparent)"
                         style:border="1px solid color-mix(in srgb, var(--accent) 55%, transparent)"></div>
                  {/if}
                </div>
                <div class="flex items-center justify-between gap-2">
                  <div class="flex flex-col min-w-0">
                    <span class="font-semibold text-[13px] truncate" style:color="var(--text)">{s.name}</span>
                    <span class="text-[10px] font-mono uppercase tracking-wide" style:color="var(--text-secondary)">{s.tag}</span>
                  </div>
                  {#if isActive}
                    <div class="w-5 h-5 rounded-full flex items-center justify-center shrink-0" style:background="var(--accent)">
                      <Check size={12} class="text-black" strokeWidth={3} />
                    </div>
                  {/if}
                </div>
                <p class="text-[11px] mt-2 leading-snug" style:color="var(--text-secondary)">{s.desc}</p>
              </button>
            {/each}
          </div>
        </Card>

        <Card
          title="Стиль оверлея"
          hint="Радикальные темы in-game оверлея. Применяются на лету, оверлей подхватит после следующего открытия."
        >
          <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5 gap-3">
            {#each overlaysMeta as o (o.id)}
              {@const isActive = currentOverlayLayout === o.id}
              <button
                type="button"
                on:click={() => quickSaveOverlay(o.id)}
                class="relative text-left rounded-[var(--radius)] border p-3 transition-all {isActive
                  ? 'border-[var(--accent)] ring-1 ring-[var(--accent-soft)]'
                  : 'border-[var(--border)] hover:border-[var(--border-strong)]'}"
                style:background="var(--surface-1)"
                aria-pressed={isActive}
              >
                <div
                  class="relative w-full h-[72px] rounded-md overflow-hidden mb-3 border"
                  style:background="color-mix(in srgb, #0a0c14 86%, transparent)"
                  style:border-color="var(--border)"
                >
                  {#if o.id === "panel"}
                    <div class="absolute inset-2 grid grid-cols-3 gap-1">
                      {#each Array(6) as _, i}
                        <div class="rounded" style:background="color-mix(in srgb, var(--accent) {i === 0 ? 55 : 18}%, transparent)"></div>
                      {/each}
                    </div>
                  {:else if o.id === "hud"}
                    <div class="absolute inset-x-2 top-2 h-3 rounded" style:background="color-mix(in srgb, var(--accent) 25%, transparent)"></div>
                    <div class="absolute left-2 bottom-2 w-8 h-3 rounded" style:background="color-mix(in srgb, var(--accent) 15%, transparent)"></div>
                    <div class="absolute right-2 bottom-2 w-8 h-3 rounded" style:background="color-mix(in srgb, var(--accent) 15%, transparent)"></div>
                  {:else if o.id === "radial"}
                    <div class="absolute inset-0 flex items-center justify-center">
                      <div class="w-10 h-10 rounded-full" style:background="radial-gradient(circle, color-mix(in srgb, var(--accent) 40%, transparent) 0%, transparent 70%)" style:border="1px solid var(--accent)"></div>
                    </div>
                    <div class="absolute left-1 top-1 w-3 h-3 rounded" style:background="color-mix(in srgb, var(--accent) 30%, transparent)"></div>
                    <div class="absolute right-1 bottom-1 w-3 h-3 rounded" style:background="color-mix(in srgb, var(--accent) 30%, transparent)"></div>
                  {:else if o.id === "ticker"}
                    <div class="absolute inset-x-0 top-1.5 h-1.5" style:background="linear-gradient(90deg, transparent, var(--accent), transparent)"></div>
                    <div class="absolute inset-x-3 top-3 h-2 flex gap-1">
                      <div class="w-6 h-full rounded" style:background="color-mix(in srgb, var(--accent) 30%, transparent)"></div>
                      <div class="w-10 h-full rounded" style:background="color-mix(in srgb, var(--accent) 20%, transparent)"></div>
                      <div class="w-5 h-full rounded" style:background="color-mix(in srgb, var(--accent) 40%, transparent)"></div>
                    </div>
                  {:else if o.id === "neon-grid"}
                    <div class="absolute inset-0"
                         style:background="repeating-linear-gradient(0deg, transparent 0 12px, color-mix(in srgb, var(--accent) 18%, transparent) 12px 13px), repeating-linear-gradient(90deg, transparent 0 12px, color-mix(in srgb, var(--accent) 18%, transparent) 12px 13px)"></div>
                    <div class="absolute inset-2 rounded" style:border="1px solid var(--accent)" style:box-shadow="inset 0 0 8px color-mix(in srgb, var(--accent) 40%, transparent)"></div>
                  {/if}
                </div>
                <div class="flex items-center justify-between gap-2">
                  <div class="flex flex-col min-w-0">
                    <span class="font-semibold text-[13px] truncate" style:color="var(--text)">{o.name}</span>
                    <span class="text-[10px] font-mono uppercase tracking-wide" style:color="var(--text-secondary)">{o.tag}</span>
                  </div>
                  {#if isActive}
                    <div class="w-5 h-5 rounded-full flex items-center justify-center shrink-0" style:background="var(--accent)">
                      <Check size={12} class="text-black" strokeWidth={3} />
                    </div>
                  {/if}
                </div>
                <p class="text-[11px] mt-2 leading-snug" style:color="var(--text-secondary)">{o.desc}</p>
              </button>
            {/each}
          </div>
        </Card>
      {/if}

      {#if appearanceTab === "palette"}
        <Card title="Тема" hint="Акцентный цвет и базовая палитра интерфейса">
          <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
            {#each builtinThemes as t (t.id)}
              <button
                type="button"
                on:click={() => {
                  settings = { ...settings, theme: t.id };
                  applyTheme(t.id, settings.background);
                  quickSaveThemeBg(t.id, settings.background);
                }}
                class="relative p-3 rounded-[var(--radius)] border transition-colors overflow-hidden text-left {currentTheme === t.id
                  ? 'border-[var(--accent)]'
                  : 'border-[var(--border)] hover:border-[var(--border-strong)]'}"
                style:background={t.preview.bg}
              >
                <div class="flex flex-col gap-1.5">
                  <div class="w-full h-2 rounded-full" style:background={t.preview.accent}></div>
                  <div class="flex gap-1">
                    <div class="flex-1 h-6 rounded-md" style:background={t.preview.card}></div>
                    <div class="flex-1 h-6 rounded-md" style:background={t.preview.card}></div>
                  </div>
                  <div class="h-3 rounded-md opacity-40" style:background={t.preview.card}></div>
                </div>
                <p
                  class="text-[10px] font-medium mt-2 text-center truncate"
                  style:color={themeCardLabelColor(t)}
                >
                  {t.name}
                </p>
                {#if currentTheme === t.id}
                  <div
                    class="absolute top-1.5 right-1.5 w-4 h-4 rounded-full flex items-center justify-center"
                    style:background={t.preview.accent}
                  >
                    <Check size={10} class="text-white" strokeWidth={3} />
                  </div>
                {/if}
              </button>
            {/each}

            <button
              type="button"
              on:click={() => {
                if (!settings.background) {
                  showToast("Сначала выберите фоновое изображение");
                  return;
                }
                settings = { ...settings, theme: "auto-bg" };
                applyTheme("auto-bg", settings.background);
                quickSaveThemeBg("auto-bg", settings.background);
              }}
              disabled={!settings.background}
              class="relative p-3 rounded-[var(--radius)] border transition-colors overflow-hidden min-h-[88px] {currentTheme === 'auto-bg'
                ? 'border-[var(--accent)]'
                : 'border-[var(--border)] hover:border-[var(--border-strong)]'} {!settings.background
                ? 'opacity-40 cursor-not-allowed'
                : ''}"
              style:background="linear-gradient(135deg, #ff6b6b 0%, #feca57 25%, #48dbfb 50%, #ff9ff3 75%, #54a0ff 100%)"
            >
              <div class="flex flex-col items-center justify-center h-full gap-1.5">
                <Wand2 size={18} class="text-white" />
                <p class="text-[10px] font-medium text-white text-center">Авто (по фону)</p>
              </div>
              {#if currentTheme === "auto-bg"}
                <div class="absolute top-1.5 right-1.5 w-4 h-4 rounded-full bg-white flex items-center justify-center">
                  <Check size={10} class="text-black" strokeWidth={3} />
                </div>
              {/if}
            </button>

            {#each customThemes as ct (ct.id)}
              <div class="relative group">
                <button
                  type="button"
                  on:click={() => {
                    settings = { ...settings, theme: ct.id };
                    applyTheme(ct.id, settings.background);
                    quickSaveThemeBg(ct.id, settings.background);
                  }}
                  class="relative w-full p-3 rounded-[var(--radius)] border transition-colors overflow-hidden text-left {currentTheme === ct.id
                    ? 'border-[var(--accent)]'
                    : 'border-[var(--border)] hover:border-[var(--border-strong)]'}"
                  style:background={ct.colors.bg}
                >
                  <div class="flex flex-col gap-1.5">
                    <div class="w-full h-2 rounded-full" style:background={ct.colors.accent}></div>
                    <div class="flex gap-1">
                      <div class="flex-1 h-6 rounded-md" style:background={ct.colors.card}></div>
                      <div class="flex-1 h-6 rounded-md" style:background={ct.colors.card}></div>
                    </div>
                    <div class="h-3 rounded-md opacity-40" style:background={ct.colors.card}></div>
                  </div>
                  <p
                    class="text-[10px] font-medium mt-2 text-center truncate"
                    style:color={ct.isLight ? "#333" : "#ddd"}
                  >
                    {ct.name}
                  </p>
                  {#if currentTheme === ct.id}
                    <div
                      class="absolute top-1.5 right-1.5 w-4 h-4 rounded-full flex items-center justify-center"
                      style:background={ct.colors.accent}
                    >
                      <Check size={10} class="text-white" strokeWidth={3} />
                    </div>
                  {/if}
                </button>
                <button
                  type="button"
                  on:click|stopPropagation={() => openEditTheme(ct)}
                  class="absolute top-1.5 left-1.5 w-5 h-5 rounded-full bg-black/60 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity z-10"
                  aria-label="Редактировать тему"
                >
                  <Pencil size={9} class="text-white" />
                </button>
              </div>
            {/each}

            <button
              type="button"
              on:click={openCreateTheme}
              class="relative p-3 rounded-[var(--radius)] border border-dashed border-[var(--border)] hover:border-[var(--border-strong)] transition-colors flex flex-col items-center justify-center gap-2 min-h-[88px]"
              style:background="var(--surface-1)"
            >
              <Plus size={20} strokeWidth={1.8} style="color: var(--text-secondary)" />
              <p class="text-[10px] font-medium" style:color="var(--text-secondary)">
                Создать тему
              </p>
            </button>
          </div>
        </Card>
      {/if}

      {#if appearanceTab === "wallpaper"}
        <Card title="Фоновое изображение" hint="Используется для темы «Авто (по фону)» и визуального оформления">
          <div class="flex flex-wrap gap-3">
            <button
              type="button"
              on:click={() => {
                settings = { ...settings, background: "" };
                applyTheme(settings.theme, "");
                quickSaveThemeBg(settings.theme, "");
              }}
              class="w-24 h-16 rounded-[var(--radius)] border flex items-center justify-center text-xs font-medium transition-colors {!settings.background
                ? 'border-[var(--accent)]'
                : 'border-[var(--border)] hover:border-[var(--border-strong)]'}"
              style:background="var(--surface-1)"
              style:color={!settings.background ? "var(--accent-light)" : "var(--text-secondary)"}
            >
              {#if !settings.background}<Check size={13} class="mr-1" />{/if}
              Без фона
            </button>
            {#each backgrounds as bgPath (bgPath)}
              <div class="relative group">
                <button
                  type="button"
                  on:click={() => {
                    settings = { ...settings, background: bgPath };
                    applyTheme(settings.theme, bgPath);
                    quickSaveThemeBg(settings.theme, bgPath);
                  }}
                  class="w-24 h-16 rounded-[var(--radius)] border overflow-hidden relative transition-colors {settings.background === bgPath
                    ? 'border-[var(--accent)]'
                    : 'border-[var(--border)] hover:border-[var(--border-strong)]'}"
                >
                  {#await resolveBackgroundImageSrc(bgPath)}
                    <div class="w-full h-full bg-[var(--surface-1)] animate-pulse" aria-hidden="true"></div>
                  {:then thumbSrc}
                    <img src={thumbSrc} alt="" class="w-full h-full object-cover" />
                  {:catch}
                    <div class="w-full h-full bg-[var(--surface-2)] flex items-center justify-center text-[10px] text-[var(--text-secondary)]">
                      ?
                    </div>
                  {/await}
                  {#if settings.background === bgPath}
                    <div
                      class="absolute inset-0 flex items-center justify-center"
                      style:background="var(--accent-soft)"
                    >
                      <Check size={14} class="text-white drop-shadow-lg" />
                    </div>
                  {/if}
                </button>
                <button
                  type="button"
                  on:click|stopPropagation={async () => {
                    invalidateBackgroundImageCache(bgPath);
                    await invoke("delete_background", { path: bgPath });
                    backgrounds = backgrounds.filter((b) => b !== bgPath);
                    if (settings.background === bgPath) {
                      settings = { ...settings, background: "" };
                      await applyTheme(settings.theme, "");
                      quickSaveThemeBg(settings.theme, "");
                    }
                  }}
                  class="absolute -top-1.5 -right-1.5 w-5 h-5 rounded-full flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity z-10"
                  style:background="#ef4444"
                  aria-label="Удалить фон"
                >
                  <X size={10} class="text-white" />
                </button>
              </div>
            {/each}
            <button
              type="button"
              on:click={handleAddBackground}
              class="w-24 h-16 rounded-[var(--radius)] border border-dashed border-[var(--border)] hover:border-[var(--border-strong)] flex items-center justify-center transition-colors"
              style:color="var(--text-secondary)"
              aria-label="Добавить фон"
            >
              <Plus size={18} />
            </button>
          </div>
          {#if settings.background}
            <div
              class="mt-4 pt-4 border-t border-[var(--border)] space-y-2 max-w-md"
              role="group"
              aria-label="Затемнение поверх фона"
            >
              <div class="flex items-center justify-between gap-3">
                <span class="text-sm font-medium" style:color="var(--accent-light)">
                  Затемнение поверх фона
                </span>
                <span class="text-xs tabular-nums font-medium" style:color="var(--text-secondary)">
                  {clampBgDim(settings.background_dim_percent)}%
                </span>
              </div>
              <input
                type="range"
                min="12"
                max="98"
                step="1"
                bind:value={settings.background_dim_percent}
                on:input={() => saveWallpaperAppearance()}
                class="w-full accent-jm-accent cursor-pointer"
              />
              <p class="text-[11px] leading-snug" style:color="var(--text-secondary)">
                Меньше — фон заметнее; больше — сплошнее «занавес» темы и удобнее читать текст.
              </p>
              <div class="flex items-center justify-between gap-3 pt-2">
                <span class="text-sm font-medium" style:color="var(--accent-light)">
                  Интерфейс: непрозрачность панелей
                </span>
                <span class="text-xs tabular-nums font-medium" style:color="var(--text-secondary)">
                  {clampPanelOpacity(settings.ui_panel_opacity_percent)}%
                </span>
              </div>
              <input
                type="range"
                min="78"
                max="100"
                step="1"
                bind:value={settings.ui_panel_opacity_percent}
                on:input={() => saveWallpaperAppearance()}
                class="w-full accent-jm-accent cursor-pointer"
              />
              <p class="text-[11px] leading-snug" style:color="var(--text-secondary)">
                Ниже по шкале — сильнее прозрачность: сквозь карточки, боковую панель и шапку немного просвечивает фон. Ниже 78 % плитки сливаются с фоном и теряют читаемость.
              </p>
            </div>
          {/if}
        </Card>
      {/if}
      </div>
    {:else if activeSection === "game"}
      <div class="flex flex-col gap-4">
        <Card title="Память" hint="Количество оперативной памяти, выделяемой для Minecraft">
          <div class="px-1">
            <div class="flex items-center justify-between mb-2">
              <span class="text-sm font-medium" style:color="var(--accent-light)">
                {settings.ram_mb} MB
              </span>
              <span class="text-xs" style:color="var(--text-secondary)">
                1 GB — {Math.round(maxRam / 1024)} GB
              </span>
            </div>
            <input
              type="range"
              min="1024"
              max={maxRam}
              step="512"
              bind:value={settings.ram_mb}
              class="w-full accent-jm-accent cursor-pointer"
            />
          </div>
        </Card>

        <Card padding="p-0">
          <SettingRow
            label="Путь к Java"
            hint="Оставьте пустым для автопоиска подходящей версии"
            stacked
          >
            <input
              type="text"
              placeholder="/usr/lib/jvm/java-21/bin/java"
              bind:value={settings.custom_java_path}
              class="ui-input"
            />
          </SettingRow>
        </Card>

        <Card title="Расширенные параметры запуска" hint="JVM-аргументы и враппер — только если знаете, что делаете">
          <details class="group">
            <summary
              class="cursor-pointer text-sm font-medium list-none flex items-center gap-2 py-2"
              style:color="var(--text-secondary)"
            >
              <span class="inline-block transition-transform group-open:rotate-90">▸</span>
              Показать JVM / Wrapper
            </summary>
            <div class="flex flex-col gap-4 mt-3">
              <div>
                <label
                  class="block text-xs font-medium mb-1"
                  style:color="var(--text-secondary)"
                >
                  Аргументы JVM
                </label>
                <input
                  type="text"
                  bind:value={settings.jvm_args}
                  class="ui-input font-mono"
                />
              </div>
              <div>
                <label
                  class="block text-xs font-medium mb-1"
                  style:color="var(--text-secondary)"
                >
                  Wrapper (Linux/macOS)
                </label>
                <input
                  type="text"
                  placeholder="mangohud"
                  bind:value={settings.wrapper}
                  class="ui-input font-mono"
                />
                <p class="ui-hint mt-1">
                  Префикс для <code class="font-mono">/bin/sh -c</code> перед java. Пример:
                  <code class="font-mono">GLFW_PLATFORM=x11 mangohud</code>
                </p>
              </div>
            </div>
          </details>
        </Card>
      </div>
    {:else if activeSection === "linux" && runtimeOs === "linux"}
      <div class="flex flex-col gap-4">
        <Card title="OpenAL / LWJGL" hint="Лечит крэши со звуком на некоторых дистрибутивах">
          <div class="flex flex-col gap-3">
            <label
              class="flex items-start gap-3 cursor-pointer p-3 rounded-[var(--radius)] border border-[var(--border)]"
              style:background="var(--surface-1)"
            >
              <Toggle
                bind:checked={settings.linux_jvm_unlock_experimental}
                ariaLabel="Unlock experimental VM options"
              />
              <div class="flex flex-col gap-0.5 min-w-0">
                <span class="text-sm font-medium">
                  <code class="font-mono text-xs">-XX:+UnlockExperimentalVMOptions</code> в начало JVM
                </span>
                <span class="ui-hint">
                  Ставится сразу после authlib, до <code class="font-mono">-Xmx</code>.
                </span>
              </div>
            </label>
            <label
              class="flex items-start gap-3 cursor-pointer p-3 rounded-[var(--radius)] border border-[var(--border)]"
              style:background="var(--surface-1)"
            >
              <Toggle
                bind:checked={settings.linux_lwjgl_openal_libname}
                ariaLabel="OpenAL libname"
              />
              <div class="flex flex-col gap-1 min-w-0 flex-1">
                <span class="text-sm font-medium">Указать системный OpenAL для LWJGL</span>
                <span class="ui-hint">
                  В конец JVM добавляется
                  <code class="font-mono">-Dorg.lwjgl.openal.libname=…</code>
                </span>
                <input
                  type="text"
                  placeholder="/usr/lib/libopenal.so"
                  bind:value={settings.linux_openal_lib_path}
                  disabled={!settings.linux_lwjgl_openal_libname}
                  class="ui-input font-mono text-xs mt-1 {!settings.linux_lwjgl_openal_libname
                    ? 'opacity-50'
                    : ''}"
                />
              </div>
            </label>
          </div>
        </Card>

        <Card title="Установка и обновления" hint="Разница между AppImage и системным пакетом">
          <p class="ui-hint mb-2">
            <strong style:color="var(--text)">AppImage</strong> — один файл без установки; автообновление
            к такому формату не применимо (обновляйте вручную).
          </p>
          <p class="ui-hint mb-3">
            <strong style:color="var(--text)">.deb / .rpm</strong> с сайта: бинарь в
            <code class="font-mono">/usr/bin</code>, данные — в домашнем каталоге, обновления как у
            обычных программ.
          </p>
          <div class="flex flex-wrap gap-2">
            <Button
              variant="subtle"
              size="sm"
              on:click={() => void openUrlInLauncher("https://jentlememes.ru/")}
            >
              Сайт и загрузки
            </Button>
            <Button variant="subtle" size="sm" on:click={() => void verifyDataDirWritable()}>
              Проверить запись в каталог данных
            </Button>
          </div>
          {#if dataDirCheckMsg}
            <p class="text-xs font-mono break-all mt-2" style:color="var(--text-secondary)">
              {dataDirCheckMsg}
            </p>
          {/if}
          <p class="ui-hint mt-3">
            Не запускайте лаунчер от root. Если были ошибки записи — выставьте владельца:
            <code class="font-mono text-[10px]">chown -R $USER ~/.jentlememes_data</code>
          </p>
        </Card>
      </div>
    {:else if activeSection === "advanced"}
      <div class="flex flex-col gap-4">
        <Card padding="p-0">
          <SettingRow
            label="Показать вкладку «Расширенные настройки»"
            hint="Токены, Java, производительность и эксперименты. Появится в шапке."
          >
            <Toggle bind:checked={settings.show_advanced_tab} ariaLabel="Advanced tab" />
          </SettingRow>
        </Card>

        <Card title="О лаунчере">
          <div class="text-sm space-y-1">
            <div class="flex justify-between">
              <span style:color="var(--text-secondary)">Версия</span>
              <span class="font-mono">v{LAUNCHER_VERSION}</span>
            </div>
            <div class="flex justify-between">
              <span style:color="var(--text-secondary)">Платформа</span>
              <span class="font-mono">{runtimeOs || "unknown"}</span>
            </div>
          </div>
        </Card>
      </div>
    {/if}

    <div class="ui-sticky-footer">
      <Button variant="primary" on:click={handleSave}>
        <Check size={14} strokeWidth={2.2} />
        Сохранить
      </Button>
    </div>
  </div>
</div>

{#if editorOpen}
  <ThemeEditorModal
    initial={editingTheme}
    onSave={handleEditorSave}
    onDelete={editingTheme ? deleteEditingTheme : undefined}
    onCancel={handleEditorCancel}
  />
{/if}
