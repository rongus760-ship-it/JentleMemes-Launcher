<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { showToast } from "../lib/jmEvents";
  import { applyTheme } from "../lib/themeApply";
  import { keyboardEventToTauriShortcut } from "../lib/tauriShortcutFromKeyboard";
  import { registerIngameOverlayHotkey } from "../lib/ingameOverlayHotkey";
  import {
    Save,
    HardDrive,
    RefreshCw,
    Coffee,
    FlaskConical,
    Layers,
    Keyboard,
    FolderOpen,
    FolderSearch,
    RotateCcw,
    Check,
    Trash2,
    Download as DownloadIcon,
    Monitor,
    LayoutGrid,
  } from "lucide-svelte";
  import {
    applyChromeDocumentAttrs,
    migrateChromeLayout,
    sidebarStyleFromLayout,
    type ChromeLayout,
    type ModalPreset,
    type DownloadCorner,
  } from "../lib/chromeLayout";
  import Card from "../components/ui/Card.svelte";
  import SettingRow from "../components/ui/SettingRow.svelte";
  import SectionNav, { type NavItem } from "../components/ui/SectionNav.svelte";
  import Toggle from "../components/ui/Toggle.svelte";
  import Button from "../components/ui/Button.svelte";

  let settings: Record<string, any> = {
    token_refresh_active_hours: 22,
    token_refresh_on_instance_launch: false,
    java_download_provider: "adoptium",
    java_major_default_subdir: {} as Record<string, string>,
    reduce_motion: false,
    hybrid_provider_enabled: false,
    internal_file_browser: false,
    enable_alpha_loaders: false,
    show_mc_snapshot_versions: false,
    show_mc_alpha_beta_versions: false,
    curseforge_api_key: "",
    download_proxy_url: "",
    show_friends_chat_tab: false,
    chat_profile_mc_server: false,
    ingame_overlay_enabled: false,
    ingame_overlay_hotkey: "Alt+Backquote",
    jentlememes_api_base_url: "https://jentlememes.ru",
    theme: "jentle-dark",
    background: "",
    ui_scale: 1.05,
    sidebar_style: "expanded",
    chrome_layout: "sidebar_left_expanded" as ChromeLayout,
    modal_preset: "minimal" as ModalPreset,
    download_corner: "bl" as DownloadCorner,
  };

  function applyUiScalePreview(scale: number) {
    try {
      document.documentElement.style.setProperty("--ui-scale", String(scale));
    } catch {
      /* ignore */
    }
  }

  $: applyUiScalePreview(
    typeof settings.ui_scale === "number"
      ? Math.min(1.6, Math.max(0.85, settings.ui_scale))
      : 1.05,
  );

  $: (() => {
    const cl = migrateChromeLayout(settings.chrome_layout, settings.sidebar_style);
    const mp = (["minimal", "glass", "dense", "sheet"].includes(String(settings.modal_preset))
      ? String(settings.modal_preset)
      : "minimal") as ModalPreset;
    applyChromeDocumentAttrs(cl, mp);
  })();

  let effectiveDataDir = "";
  let defaultDataDir = "";
  let overridePath: string | null = null;

  const modalOptions: { id: ModalPreset; label: string; sub: string }[] = [
    { id: "minimal", label: "Минимум", sub: "Лёгкая тень, без blur" },
    { id: "glass", label: "Стекло", sub: "Blur + полупрозрачность" },
    { id: "dense", label: "Плотный", sub: "Темнее фон, мелкий радиус" },
    { id: "sheet", label: "Шит", sub: "Жёстче контраст" },
  ];

  const downloadOptions: { id: DownloadCorner; label: string }[] = [
    { id: "bl", label: "Низ · слева" },
    { id: "br", label: "Низ · справа" },
    { id: "tl", label: "Верх · слева" },
    { id: "tr", label: "Верх · справа" },
    { id: "hidden", label: "Скрыть виджет" },
  ];

  const chromeLayouts: { id: ChromeLayout; title: string; hint: string }[] = [
    { id: "sidebar_left_expanded", title: "Слева, полный", hint: "Стиль по умолчанию" },
    { id: "sidebar_left_compact", title: "Слева, компакт", hint: "Только иконки" },
    { id: "sidebar_right_expanded", title: "Справа, полный", hint: "Панель справа" },
    { id: "sidebar_right_compact", title: "Справа, компакт", hint: "Иконки справа" },
    { id: "top_tabs", title: "Вкладки сверху", hint: "Строка под заголовком" },
    { id: "bottom_tabs", title: "Вкладки снизу", hint: "Навигация внизу окна" },
  ];

  const javaMajorChoices = [8, 11, 17, 21, 22, 25];
  let javaWizardMajor = 17;
  type JavaBuildRow = { id: string; label: string; download_url: string; archive_name: string };
  let javaBuildList: JavaBuildRow[] = [];
  let javaBuildLoading = false;
  let javaSelectedBuild: JavaBuildRow | null = null;
  let javaDownloadBusy = false;

  type SectionId =
    | "interface"
    | "chrome"
    | "session"
    | "java"
    | "storage"
    | "mc-versions"
    | "experiments"
    | "overlay";
  let activeSection: SectionId = "interface";
  const selectSection = (id: string): void => {
    activeSection = id as SectionId;
  };

  const navItems: NavItem[] = [
    { id: "interface", label: "Интерфейс", icon: Monitor },
    { id: "chrome", label: "Оформление", icon: LayoutGrid },
    { id: "session", label: "Сессия", icon: RefreshCw },
    { id: "java", label: "Java", icon: Coffee },
    { id: "storage", label: "Каталог данных", icon: HardDrive },
    { id: "mc-versions", label: "Версии MC", icon: Layers },
    { id: "overlay", label: "Оверлей", icon: Keyboard },
    { id: "experiments", label: "Эксперименты", icon: FlaskConical },
  ];

  onMount(async () => {
    try {
      const data: any = await invoke("load_settings");
      const cl = migrateChromeLayout(data.chrome_layout, data.sidebar_style);
      const mp = (["minimal", "glass", "dense", "sheet"].includes(String(data.modal_preset))
        ? data.modal_preset
        : "minimal") as ModalPreset;
      const dc = (["bl", "br", "tl", "tr", "hidden"].includes(String(data.download_corner))
        ? data.download_corner
        : "bl") as DownloadCorner;
      settings = { ...settings, ...data, chrome_layout: cl, modal_preset: mp, download_corner: dc };
      applyChromeDocumentAttrs(cl, mp);
      effectiveDataDir = await invoke("get_data_dir");
      defaultDataDir = await invoke("get_default_data_dir_path");
      overridePath = await invoke("get_data_root_override_path_json");
    } catch (e) {
      console.error(e);
    }
  });

  function setJavaProvider(p: string) {
    settings = { ...settings, java_download_provider: p };
  }

  async function loadJavaBuildList() {
    javaBuildLoading = true;
    javaBuildList = [];
    javaSelectedBuild = null;
    try {
      const prov = String(settings.java_download_provider || "adoptium");
      const rows = await invoke<JavaBuildRow[]>("list_java_build_options", {
        provider: prov,
        major: javaWizardMajor,
      });
      javaBuildList = rows || [];
      if (!javaBuildList.length) {
        showToast("Список сборок пуст (попробуйте другого поставщика или версию)");
      }
    } catch (e) {
      showToast(`Список JRE: ${e}`);
    } finally {
      javaBuildLoading = false;
    }
  }

  async function downloadSelectedJavaBuild() {
    if (!javaSelectedBuild) {
      showToast("Выберите сборку в списке");
      return;
    }
    javaDownloadBusy = true;
    try {
      await invoke("download_java_build", {
        buildId: javaSelectedBuild.id,
        downloadUrl: javaSelectedBuild.download_url,
        archiveName: javaSelectedBuild.archive_name,
      });
      showToast("JRE скачана в каталог java/runtimes/");
    } catch (e) {
      showToast(`Скачивание: ${e}`);
    } finally {
      javaDownloadBusy = false;
    }
  }

  async function setDefaultJavaForWizardMajor() {
    if (!javaSelectedBuild) {
      showToast("Выберите сборку");
      return;
    }
    const rel = `runtimes/${javaSelectedBuild.id}`;
    try {
      await invoke("set_java_default_for_major", {
        major: javaWizardMajor,
        runtimeSubdirRel: rel,
      });
      settings = {
        ...settings,
        java_major_default_subdir: {
          ...((settings.java_major_default_subdir as Record<string, string>) || {}),
          [String(javaWizardMajor)]: rel,
        },
      };
      showToast(`По умолчанию для Java ${javaWizardMajor}: ${rel}`);
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  async function clearDefaultJavaForWizardMajor() {
    try {
      await invoke("set_java_default_for_major", {
        major: javaWizardMajor,
        runtimeSubdirRel: null,
      });
      const next = { ...((settings.java_major_default_subdir as Record<string, string>) || {}) };
      delete next[String(javaWizardMajor)];
      settings = { ...settings, java_major_default_subdir: next };
      showToast(`Сброшен дефолт для Java ${javaWizardMajor}`);
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  async function handleSave() {
    try {
      // Атомарно патчим только поля расширенных настроек. Прежняя схема
      // load_settings → merge → save_settings устраивала race condition:
      // если другая вкладка между load и save писала custom_themes или
      // visual_preset, эти значения затирались «свежим» merged-снапшотом.
      const cl = migrateChromeLayout(settings.chrome_layout, settings.sidebar_style);
      const delta = {
        ...settings,
        chrome_layout: cl,
        sidebar_style: sidebarStyleFromLayout(cl),
      };
      const { patchSettings } = await import("../lib/settingsStore");
      await patchSettings(delta);
      await applyTheme(settings.theme, settings.background);
      showToast("Сохранено");
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  async function pickDataRoot() {
    try {
      const p: string | null = await invoke("pick_data_root_folder");
      if (!p) return;
      await invoke("apply_data_root_override", { path: p });
      showToast("Путь сохранён. Перезапустите лаунчер.");
      overridePath = p;
      effectiveDataDir = p;
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  async function clearDataRoot() {
    try {
      await invoke("clear_data_root_override");
      overridePath = null;
      effectiveDataDir = await invoke("get_data_dir");
      showToast("Сброшено. Перезапустите лаунчер.");
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  async function openDataDir() {
    try {
      await openPath(effectiveDataDir || (await invoke("get_data_dir")));
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  let recordingOverlayHotkey = false;

  function onOverlayHotkeyRecordKeydown(e: KeyboardEvent) {
    if (!recordingOverlayHotkey) return;
    e.preventDefault();
    e.stopPropagation();
    e.stopImmediatePropagation();
    if (e.key === "Escape") {
      recordingOverlayHotkey = false;
      showToast("Запись комбинации отменена");
      return;
    }
    const s = keyboardEventToTauriShortcut(e);
    if (!s) return;
    settings = { ...settings, ingame_overlay_hotkey: s };
    recordingOverlayHotkey = false;
    void registerIngameOverlayHotkey(!!settings.ingame_overlay_enabled, s);
    showToast(`Комбинация: ${s}. Сохраните настройки, чтобы записать в файл.`);
  }

  const javaProviders = [
    { id: "adoptium", label: "Eclipse Temurin" },
    { id: "zulu", label: "Azul Zulu" },
    { id: "microsoft", label: "Microsoft (fallback Temurin)" },
  ];
</script>

<svelte:window on:keydown|capture={onOverlayHotkeyRecordKeydown} />

<div class="jm-container flex h-full gap-6 pt-6">
  <!-- Sidebar nav -->
  <aside class="w-48 shrink-0 hidden md:flex flex-col gap-4">
    <div>
      <h2 class="text-lg font-semibold leading-tight">Расширенные</h2>
      <p class="text-[11px]" style:color="var(--text-secondary)">Тонкие настройки</p>
    </div>
    <SectionNav items={navItems} active={activeSection} onChange={selectSection} />
  </aside>

  <!-- Mobile nav -->
  <div class="md:hidden sticky top-0 z-10 w-full">
    <div class="ui-seg w-full overflow-x-auto flex">
      {#each navItems as item (item.id)}
        <button
          type="button"
          class="ui-seg-item whitespace-nowrap"
          class:is-active={activeSection === item.id}
          on:click={() => selectSection(item.id)}
        >
          {item.label}
        </button>
      {/each}
    </div>
  </div>

  <!-- Content -->
  <div class="flex-1 min-w-0 overflow-y-auto custom-scrollbar pb-4 pr-1 relative">
    {#if activeSection === "interface"}
      <div class="flex flex-col gap-4">
        <Card title="Масштаб интерфейса" hint="Применяется мгновенно. Ползунок показывает превью; значение сохраняется кнопкой «Сохранить» внизу.">
          <div class="flex flex-col gap-3">
            <div class="flex items-center gap-3">
              <input
                id="adv-ui-scale"
                type="range"
                min="0.85"
                max="1.60"
                step="0.05"
                bind:value={settings.ui_scale}
                class="flex-1"
                aria-label="Масштаб интерфейса"
              />
              <span
                class="w-14 text-right font-semibold tabular-nums text-[13px]"
                style:color="var(--text)"
              >
                {Math.round((settings.ui_scale || 1) * 100)}%
              </span>
              <Button
                variant="ghost"
                size="sm"
                on:click={() => (settings = { ...settings, ui_scale: 1.05 })}
              >
                Сброс
              </Button>
            </div>
            <div class="flex gap-1.5 flex-wrap">
              {#each [0.9, 1.0, 1.05, 1.15, 1.25, 1.4] as preset}
                <button
                  type="button"
                  class="ui-btn ui-btn-subtle ui-btn-sm"
                  class:ui-btn-primary={Math.abs((settings.ui_scale || 1) - preset) < 0.01}
                  on:click={() => (settings = { ...settings, ui_scale: preset })}
                >
                  {Math.round(preset * 100)}%
                </button>
              {/each}
            </div>
          </div>
        </Card>

        <p class="text-[11px]" style:color="var(--text-secondary)">
          Расположение панели, вкладки сверху как раньше, стили модалок и виджета загрузки —
          раздел «Оформление» ниже.
        </p>

        <Card padding="p-0">
          <SettingRow
            label="Отключить визуальные эффекты"
            hint="Минимум анимаций и декоративного фона. Полезно на слабых машинах и при проблемах с отрисовкой на NVIDIA/Wayland."
          >
            <Toggle bind:checked={settings.reduce_motion} ariaLabel="Reduce motion" />
          </SettingRow>
        </Card>
      </div>
    {:else if activeSection === "chrome"}
      <div class="flex flex-col gap-4 pb-8">
        <Card
          title="Панель навигации"
          hint="Выберите расположение и плотность. Кнопка «Плотнее» на панели быстро переключает expanded ↔ compact для боковых режимов."
        >
          <div class="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-3 gap-2.5">
            {#each chromeLayouts as L (L.id)}
              {@const active = settings.chrome_layout === L.id}
              <button
                type="button"
                on:click={() =>
                  (settings = {
                    ...settings,
                    chrome_layout: L.id,
                    sidebar_style: sidebarStyleFromLayout(L.id),
                  })}
                class="flex flex-col gap-2 p-3 rounded-[var(--radius-sm)] border transition-colors text-left min-h-[108px]"
                style:background={active
                  ? "color-mix(in srgb, var(--accent) 10%, var(--surface-1))"
                  : "var(--surface-1)"}
                style:border-color={active ? "var(--accent)" : "var(--border)"}
              >
                <div
                  class="w-full h-14 rounded-[6px] border overflow-hidden shrink-0 relative"
                  style:border-color="var(--border)"
                  style:background="color-mix(in srgb, var(--text) 6%, transparent)"
                >
                  {#if L.id.startsWith("sidebar_left")}
                    <div class="absolute inset-y-1 left-1 w-[22%] rounded-[4px]" style:background="var(--accent-soft)"></div>
                  {:else if L.id.startsWith("sidebar_right")}
                    <div class="absolute inset-y-1 right-1 w-[22%] rounded-[4px]" style:background="var(--accent-soft)"></div>
                  {:else if L.id === "top_tabs"}
                    <div class="absolute top-1 left-1 right-1 h-[26%] rounded-[4px]" style:background="var(--accent-soft)"></div>
                  {:else}
                    <div class="absolute bottom-1 left-1 right-1 h-[26%] rounded-[4px]" style:background="var(--accent-soft)"></div>
                  {/if}
                  <div
                    class="absolute rounded-[3px]"
                    style:background="color-mix(in srgb, var(--text) 14%, transparent)"
                    style:left={L.id.startsWith('sidebar_right') ? '10%' : L.id.startsWith('sidebar_left') ? '28%' : '12%'}
                    style:right={L.id.startsWith('sidebar_right') ? '28%' : L.id.startsWith('sidebar_left') ? '12%' : '12%'}
                    style:top={L.id === 'top_tabs' ? '38%' : L.id === 'bottom_tabs' ? '14%' : '26%'}
                    style:bottom={L.id === 'bottom_tabs' ? '38%' : '14%'}
                  ></div>
                </div>
                <div class="flex flex-col gap-0.5">
                  <span class="text-[13px] font-semibold">{L.title}</span>
                  <span class="text-[11px]" style:color="var(--text-secondary)">{L.hint}</span>
                </div>
              </button>
            {/each}
          </div>
        </Card>

        <Card title="Стиль модальных окон и оверлеев" hint="Влияет на затемнение фона и поверхность (например встроенный браузер). Остальные диалоги постепенно переводятся на те же токены.">
          <div class="grid grid-cols-2 sm:grid-cols-4 gap-2">
            {#each modalOptions as opt (opt.id)}
              {@const active = settings.modal_preset === opt.id}
              <button
                type="button"
                on:click={() => (settings = { ...settings, modal_preset: opt.id })}
                class="flex flex-col gap-1 p-3 rounded-[var(--radius-sm)] border text-left transition-colors"
                style:background={active ? "color-mix(in srgb, var(--accent) 12%, var(--surface-1))" : "var(--surface-1)"}
                style:border-color={active ? "var(--accent)" : "var(--border)"}
              >
                <span class="text-[13px] font-semibold">{opt.label}</span>
                <span class="text-[11px]" style:color="var(--text-secondary)">{opt.sub}</span>
              </button>
            {/each}
          </div>
        </Card>

        <Card title="Виджет загрузки и уведомления" hint="Где показывается индикатор загрузки файлов и ленточные уведомления (тосты используют тот же угол).">
          <div class="flex flex-wrap gap-2">
            {#each downloadOptions as c (c.id)}
              {@const active = settings.download_corner === c.id}
              <button
                type="button"
                on:click={() => (settings = { ...settings, download_corner: c.id })}
                class="ui-btn px-4 py-2 text-[12px]"
                class:ui-btn-primary={active}
                class:ui-btn-subtle={!active}
              >
                {c.label}
              </button>
            {/each}
          </div>
        </Card>
      </div>
    {:else if activeSection === "session"}
      <div class="flex flex-col gap-4">
        <Card title="Обновление Microsoft сессии" hint="Накопление идёт, пока окно лаунчера на переднем плане. При достижении порога токен обновляется через refresh token.">
          <div class="space-y-4">
            <div>
              <label for="adv-hrs" class="block text-sm font-medium mb-1">Часов активности до обновления</label>
              <input
                id="adv-hrs"
                type="number"
                min="0.5"
                max="168"
                step="0.5"
                bind:value={settings.token_refresh_active_hours}
                class="ui-input"
              />
            </div>
          </div>
        </Card>

        <Card padding="p-0">
          <SettingRow
            label="Обновлять перед запуском сборки"
            hint="Принудительный refresh Microsoft-токена перед каждым запуском — медленнее старт, но ниже шанс выбить аккаунт."
          >
            <Toggle bind:checked={settings.token_refresh_on_instance_launch} ariaLabel="Обновлять перед запуском" />
          </SettingRow>
        </Card>
      </div>
    {:else if activeSection === "java"}
      <div class="flex flex-col gap-4">
        <Card title="Поставщик Java" hint="Используется для автоматической подготовки JRE и в списке сборок ниже.">
          <div class="flex flex-wrap gap-2">
            {#each javaProviders as p (p.id)}
              <button
                type="button"
                on:click={() => setJavaProvider(p.id)}
                class="ui-btn ui-btn-sm"
                class:ui-btn-primary={settings.java_download_provider === p.id}
                class:ui-btn-subtle={settings.java_download_provider !== p.id}
              >
                {p.label}
              </button>
            {/each}
          </div>
        </Card>

        <Card title="Менеджер JRE" hint="Выберите major, обновите список, отметьте нужную сборку — скачайте в java/runtimes/ и задайте дефолт для запусков, где нужна эта major (если путь в сборке не переопределён).">
          <div class="space-y-3">
            <div class="flex flex-wrap items-center gap-2">
              <span class="text-xs font-medium" style:color="var(--text-secondary)">Java major:</span>
              {#each javaMajorChoices as m (m)}
                <button
                  type="button"
                  on:click={() => {
                    javaWizardMajor = m;
                    javaBuildList = [];
                    javaSelectedBuild = null;
                  }}
                  class="ui-btn ui-btn-sm"
                  class:ui-btn-primary={javaWizardMajor === m}
                  class:ui-btn-subtle={javaWizardMajor !== m}
                >
                  {m}
                </button>
              {/each}
            </div>

            {#if settings.java_major_default_subdir?.[String(javaWizardMajor)]}
              <div class="flex items-center justify-between gap-3 p-2 rounded-[var(--radius-sm)]" style:background="var(--surface-1)">
                <div class="min-w-0">
                  <p class="text-[11px] uppercase tracking-wider" style:color="var(--text-secondary)">Сейчас для Java {javaWizardMajor}</p>
                  <p class="text-xs font-mono break-all mt-0.5">java/{settings.java_major_default_subdir[String(javaWizardMajor)]}</p>
                </div>
                <Button variant="ghost" size="sm" on:click={() => void clearDefaultJavaForWizardMajor()}>
                  <RotateCcw size={12} strokeWidth={2.2} /> Сбросить
                </Button>
              </div>
            {/if}

            <div class="flex flex-wrap gap-2">
              <Button variant="subtle" size="sm" disabled={javaBuildLoading} on:click={() => void loadJavaBuildList()}>
                <RefreshCw size={13} strokeWidth={2.2} class={javaBuildLoading ? "animate-spin" : ""} />
                {javaBuildLoading ? "Загрузка…" : "Обновить список"}
              </Button>
              <Button variant="primary" size="sm" disabled={javaDownloadBusy || !javaSelectedBuild} on:click={() => void downloadSelectedJavaBuild()}>
                <DownloadIcon size={13} strokeWidth={2.2} /> Скачать выбранную
              </Button>
              <Button variant="subtle" size="sm" disabled={!javaSelectedBuild} on:click={() => void setDefaultJavaForWizardMajor()}>
                <Check size={13} strokeWidth={2.2} /> Дефолт для Java {javaWizardMajor}
              </Button>
            </div>

            <div class="rounded-[var(--radius)] border border-[var(--border)] overflow-hidden" style:background="var(--surface-1)">
              <div class="max-h-56 overflow-y-auto custom-scrollbar">
                {#if javaBuildList.length === 0 && !javaBuildLoading}
                  <p class="p-4 text-xs text-center" style:color="var(--text-secondary)">
                    Нажмите «Обновить список», чтобы увидеть доступные сборки.
                  </p>
                {:else if javaBuildLoading && javaBuildList.length === 0}
                  <p class="p-4 text-xs text-center" style:color="var(--text-secondary)">Загрузка…</p>
                {:else}
                  {#each javaBuildList as b (b.id + b.download_url)}
                    <button
                      type="button"
                      on:click={() => (javaSelectedBuild = b)}
                      class="w-full text-left px-3 py-2.5 border-b border-[var(--border)] last:border-0 transition-colors hover:bg-[var(--surface-hover)]"
                      class:bg-[var(--accent-softer)]={javaSelectedBuild?.id === b.id}
                    >
                      <div class="flex items-center gap-2">
                        {#if javaSelectedBuild?.id === b.id}
                          <Check size={12} class="text-[var(--accent-light)] shrink-0" strokeWidth={2.5} />
                        {:else}
                          <span class="w-3 h-3 rounded-full border border-[var(--border)] shrink-0"></span>
                        {/if}
                        <span class="font-mono text-xs flex-1 truncate">{b.id}</span>
                      </div>
                      <p class="ui-hint mt-1 pl-5">{b.label}</p>
                    </button>
                  {/each}
                {/if}
              </div>
            </div>
          </div>
        </Card>
      </div>
    {:else if activeSection === "storage"}
      <div class="flex flex-col gap-4">
        <Card title="Каталог данных" hint="По умолчанию: стандартный каталог приложения. Старый ~/.jentlememes_data мигрирует при первом запуске, если новый путь пуст.">
          <div class="space-y-3">
            <div class="p-3 rounded-[var(--radius)] border border-[var(--border)] space-y-2" style:background="var(--surface-1)">
              <div>
                <p class="text-[11px] uppercase tracking-wider" style:color="var(--text-secondary)">Используется сейчас</p>
                <p class="text-xs font-mono break-all mt-0.5">{effectiveDataDir || "—"}</p>
              </div>
              <div>
                <p class="text-[11px] uppercase tracking-wider" style:color="var(--text-secondary)">Стандартный путь</p>
                <p class="text-xs font-mono break-all mt-0.5" style:color="var(--text-secondary)">{defaultDataDir || "—"}</p>
              </div>
              {#if overridePath}
                <div>
                  <p class="text-[11px] uppercase tracking-wider" style:color="var(--accent)">Переопределение активно</p>
                  <p class="text-xs font-mono break-all mt-0.5" style:color="var(--accent-light)">{overridePath}</p>
                </div>
              {/if}
            </div>

            <div class="flex flex-wrap gap-2">
              <Button variant="subtle" size="sm" on:click={openDataDir}>
                <FolderOpen size={13} strokeWidth={2.2} /> Открыть в проводнике
              </Button>
              <Button variant="primary" size="sm" on:click={pickDataRoot}>
                <FolderSearch size={13} strokeWidth={2.2} /> Выбрать другой каталог…
              </Button>
              {#if overridePath}
                <Button variant="danger" size="sm" on:click={clearDataRoot}>
                  <Trash2 size={13} strokeWidth={2.2} /> Сбросить переопределение
                </Button>
              {/if}
            </div>
          </div>
        </Card>
      </div>
    {:else if activeSection === "mc-versions"}
      <div class="flex flex-col gap-4">
        <Card title="Фильтры списков версий Minecraft" hint="Влияет на создание сборки, смену ядра, каталог модов и загрузчики. Релизы (release) видны всегда.">
          <div class="ui-card ui-card-flat" style:padding="0">
            <SettingRow
              label="Снапшоты и предрелизы"
              hint="Modrinth: тип snapshot. Mojang: тип snapshot в манифесте."
            >
              <Toggle bind:checked={settings.show_mc_snapshot_versions} ariaLabel="Снапшоты" />
            </SettingRow>
            <SettingRow
              label="Альфа и бета (legacy)"
              hint="Modrinth: alpha и beta. Mojang: old_alpha и old_beta."
            >
              <Toggle bind:checked={settings.show_mc_alpha_beta_versions} ariaLabel="Альфа/Бета" />
            </SettingRow>
          </div>
        </Card>
      </div>
    {:else if activeSection === "overlay"}
      <div class="flex flex-col gap-4">
        <Card padding="p-0">
          <SettingRow
            label="Оверлей в игре"
            hint="Второе окно поверх Minecraft (запуск через лаунчер). Включите и сохраните, затем назначьте клавишу."
          >
            <Toggle bind:checked={settings.ingame_overlay_enabled} ariaLabel="Оверлей" />
          </SettingRow>
        </Card>

        <Card title="Горячая клавиша" hint="Формат плагина Tauri. Пример: Alt+Backquote — это Alt + `. Другое: Control+Shift+O. Esc — отмена записи.">
          <div class="flex flex-col sm:flex-row gap-2 items-stretch sm:items-center">
            <input
              type="text"
              bind:value={settings.ingame_overlay_hotkey}
              placeholder={recordingOverlayHotkey ? "Нажмите комбинацию…" : "Alt+Backquote"}
              readonly={recordingOverlayHotkey}
              class="ui-input font-mono text-xs flex-1 min-w-0"
              class:ring-2={recordingOverlayHotkey}
              class:ring-[var(--accent)]={recordingOverlayHotkey}
              autocomplete="off"
            />
            <Button
              variant={recordingOverlayHotkey ? "primary" : "subtle"}
              size="sm"
              on:click={() => (recordingOverlayHotkey = !recordingOverlayHotkey)}
            >
              <Keyboard size={13} strokeWidth={2.2} />
              {recordingOverlayHotkey ? "Отмена" : "Записать"}
            </Button>
          </div>
        </Card>
      </div>
    {:else if activeSection === "experiments"}
      <div class="flex flex-col gap-4">
        <Card title="Провайдеры и файлы" padding="p-0">
          <SettingRow
            label="Гибридный режим (Modrinth + CurseForge)"
            hint="Поиск и установка контента одновременно из двух источников."
          >
            <Toggle bind:checked={settings.hybrid_provider_enabled} ariaLabel="Hybrid provider" />
          </SettingRow>
          <SettingRow
            label="Внутренний обзор файлов у сборки"
            hint="Встроенный браузер файлов сборки вместо проводника ОС."
          >
            <Toggle bind:checked={settings.internal_file_browser} ariaLabel="Internal file browser" />
          </SettingRow>
          <SettingRow
            label="Альфа: LiteLoader и ModLoader (Risugami)"
            hint="LiteLoader — вшитый JSON в лаунчере. ModLoader — манифест + zip в modloader_patches/ (слияние в client.jar)."
          >
            <Toggle bind:checked={settings.enable_alpha_loaders} ariaLabel="Alpha loaders" />
          </SettingRow>
        </Card>

        <Card title="Сетевые">
          <div class="space-y-4">
            <div>
              <label for="adv-cf-key" class="block text-sm font-medium mb-1">CurseForge API key</label>
              <p class="ui-hint mb-2">
                Ключ с <code class="font-mono">console.curseforge.com</code>. Нужен для загрузок CurseForge CDN.
              </p>
              <input
                id="adv-cf-key"
                type="password"
                bind:value={settings.curseforge_api_key}
                placeholder="CurseForge API key"
                class="ui-input font-mono text-xs"
                autocomplete="off"
              />
            </div>
            <div>
              <label for="adv-proxy" class="block text-sm font-medium mb-1">Прокси для загрузок</label>
              <p class="ui-hint mb-2">
                HTTP(S) прокси для игры, модов, Java и API. Пример: <code class="font-mono">http://127.0.0.1:7890</code>. Пусто — прямое подключение. Кэш клиента сбрасывается автоматически.
              </p>
              <input
                id="adv-proxy"
                type="text"
                bind:value={settings.download_proxy_url}
                placeholder="http://127.0.0.1:7890"
                class="ui-input font-mono text-xs"
                autocomplete="off"
              />
            </div>
            <div>
              <label for="adv-api" class="block text-sm font-medium mb-1">Базовый URL API сайта</label>
              <input
                id="adv-api"
                type="text"
                bind:value={settings.jentlememes_api_base_url}
                placeholder="https://jentlememes.ru"
                class="ui-input font-mono text-xs"
                autocomplete="off"
              />
            </div>
          </div>
        </Card>

        <Card title="Чат и интеграции" padding="p-0">
          <SettingRow
            label="Вкладка «Чат» (друзья / сайт)"
            hint="Появится в шапке. Вход через API jentlememes.ru. Нужен запущенный бэкенд — см. docs/SERVER-SSH.md."
          >
            <Toggle bind:checked={settings.show_friends_chat_tab} ariaLabel="Friends chat" />
          </SettingRow>
          <SettingRow
            label="Сервер в профиле чата"
            hint="В карточке пользователя (ПКМ) показывать хост, MOTD, онлайн и головы через прямой Server List Ping (TCP)."
          >
            <Toggle bind:checked={settings.chat_profile_mc_server} ariaLabel="Chat server" />
          </SettingRow>
        </Card>
      </div>
    {/if}

    <div class="ui-sticky-footer mt-6">
      <Button variant="primary" size="md" on:click={handleSave}>
        <Save size={15} strokeWidth={2.2} /> Сохранить
      </Button>
    </div>
  </div>
</div>
