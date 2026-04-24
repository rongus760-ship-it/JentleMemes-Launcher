<script lang="ts">
  import { onMount, createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { fly } from "svelte/transition";
  import {
    Sparkles,
    Palette,
    LayoutGrid,
    AppWindow,
    Crosshair,
    Gauge,
    Zap,
    Check,
    ChevronRight,
    ChevronLeft,
    PartyPopper,
  } from "lucide-svelte";
  import { builtinThemes, type ThemeDef } from "./themes";
  import {
    applyTheme,
    applyVisualPreset,
    normalizeVisualPreset,
    type VisualPreset,
  } from "./lib/themeApply";
  import {
    applyChromeDocumentAttrs,
    migrateChromeLayout,
    sidebarStyleFromLayout,
    type ChromeLayout,
    type ModalPreset,
    type DownloadCorner,
  } from "./lib/chromeLayout";

  const dispatch = createEventDispatcher<{ done: void }>();

  type StepId =
    | "welcome"
    | "theme"
    | "preset"
    | "chrome"
    | "scale"
    | "modals"
    | "hud"
    | "motion"
    | "finish";

  const steps: { id: StepId; label: string }[] = [
    { id: "welcome", label: "Приветствие" },
    { id: "theme", label: "Тема" },
    { id: "preset", label: "Визуал" },
    { id: "chrome", label: "Панель" },
    { id: "scale", label: "Масштаб" },
    { id: "modals", label: "Модалки" },
    { id: "hud", label: "Кнопки / HUD" },
    { id: "motion", label: "Анимации" },
    { id: "finish", label: "Готово" },
  ];

  let stepIndex = 0;
  $: step = steps[stepIndex];
  $: isFirst = stepIndex === 0;
  $: isLast = stepIndex === steps.length - 1;

  let chosenTheme: string = "jentle-dark";
  let chosenPreset: VisualPreset = "blend";
  let chosenChrome: ChromeLayout = "sidebar_left_expanded";
  let chosenModal: ModalPreset = "minimal";
  let chosenDownload: DownloadCorner = "bl";
  let chosenScale: number = 1.05;
  let chosenReduceMotion = false;
  let saving = false;

  const chromePick: { id: ChromeLayout; title: string; desc: string }[] = [
    { id: "sidebar_left_expanded", title: "Слева, полный", desc: "Как сейчас в JentleMemes" },
    { id: "sidebar_left_compact", title: "Слева, компакт", desc: "Только иконки" },
    { id: "sidebar_right_expanded", title: "Справа, полный", desc: "Контент слева" },
    { id: "sidebar_right_compact", title: "Справа, компакт", desc: "Иконки справа" },
    { id: "top_tabs", title: "Вкладки сверху", desc: "Классика — полоса под заголовком" },
    { id: "bottom_tabs", title: "Вкладки снизу", desc: "Как на мобильных" },
  ];

  onMount(() => {
    void (async () => {
      try {
        const s: any = await invoke("load_settings");
        if (s.theme) chosenTheme = s.theme;
        chosenPreset = normalizeVisualPreset(s.visual_preset);
        chosenChrome = migrateChromeLayout(s.chrome_layout, s.sidebar_style);
        const mp = String(s.modal_preset || "minimal");
        if (["minimal", "glass", "dense", "sheet"].includes(mp)) chosenModal = mp as ModalPreset;
        const dc = String(s.download_corner || "bl");
        if (["bl", "br", "tl", "tr", "hidden"].includes(dc)) chosenDownload = dc as DownloadCorner;
        const rawScale =
          typeof s.ui_scale === "number" ? s.ui_scale : parseFloat(s.ui_scale || "1.05");
        if (Number.isFinite(rawScale)) {
          chosenScale = Math.min(1.6, Math.max(0.85, rawScale));
        }
        chosenReduceMotion = !!s.reduce_motion;
      } catch {
        /* первый запуск — значений может не быть */
      }
      applyScalePreview(chosenScale);
      void applyThemePreview(chosenTheme);
      applyVisualPreset(chosenPreset);
      applyChromeDocumentAttrs(chosenChrome, chosenModal);
    })();
  });

  $: applyChromeDocumentAttrs(chosenChrome, chosenModal);

  function applyScalePreview(v: number) {
    try {
      document.documentElement.style.setProperty("--ui-scale", String(v));
    } catch {
      /* ignore */
    }
  }

  async function applyThemePreview(id: string) {
    try {
      await applyTheme(id, "");
    } catch {
      /* ignore */
    }
  }

  function pickTheme(t: ThemeDef) {
    chosenTheme = t.id;
    void applyThemePreview(t.id);
  }

  function nextStep() {
    if (stepIndex < steps.length - 1) stepIndex += 1;
  }
  function prevStep() {
    if (stepIndex > 0) stepIndex -= 1;
  }

  async function finish() {
    if (saving) return;
    saving = true;
    try {
      const current: any = await invoke("load_settings");
      const next = {
        ...current,
        theme: chosenTheme,
        visual_preset: chosenPreset,
        chrome_layout: chosenChrome,
        sidebar_style: sidebarStyleFromLayout(chosenChrome),
        modal_preset: chosenModal,
        download_corner: chosenDownload,
        ui_scale: chosenScale,
        reduce_motion: chosenReduceMotion,
        onboarding_completed: true,
      };
      await invoke("save_settings", { settings: next });
      try {
        await invoke("complete_onboarding");
      } catch {
        /* уже отмечено через save_settings */
      }
      dispatch("done");
    } catch (e) {
      console.error("onboarding finish error", e);
      dispatch("done");
    } finally {
      saving = false;
    }
  }

  async function skip() {
    saving = true;
    try {
      try {
        await invoke("complete_onboarding");
      } catch {
        /* ignore */
      }
    } finally {
      saving = false;
      dispatch("done");
    }
  }

  const scalePresets = [
    { v: 0.9, l: "90%" },
    { v: 1.0, l: "100%" },
    { v: 1.1, l: "110%" },
    { v: 1.25, l: "125%" },
  ];

  const modalStepOptions: { id: ModalPreset; t: string; d: string }[] = [
    { id: "minimal", t: "Минимум", d: "Лёгкая тень" },
    { id: "glass", t: "Стекло", d: "Blur + прозрачность" },
    { id: "dense", t: "Плотный", d: "Тёмнее фон" },
    { id: "sheet", t: "Шит", d: "Жёстче края" },
  ];

  const hudOptions: { id: DownloadCorner; label: string }[] = [
    { id: "bl", label: "Низ · слева" },
    { id: "br", label: "Низ · справа" },
    { id: "tl", label: "Верх · слева" },
    { id: "tr", label: "Верх · справа" },
    { id: "hidden", label: "Скрыть виджет" },
  ];

  type PresetInfo = {
    id: VisualPreset;
    title: string;
    desc: string;
    radius: string;
    density: string;
    shadow: string;
    blur: string;
  };

  const presetOptions: PresetInfo[] = [
    {
      id: "blend",
      title: "Blend (по умолчанию)",
      desc: "Гибрид лучшего из Modrinth, Discord и Linear.",
      radius: "10px",
      density: "1",
      shadow: "0 8px 20px rgba(0,0,0,.25)",
      blur: "0px",
    },
    {
      id: "modrinth",
      title: "Modrinth",
      desc: "Просторный, крупные карточки, мягкие углы.",
      radius: "14px",
      density: "1.1",
      shadow: "0 14px 32px rgba(0,0,0,.28)",
      blur: "0px",
    },
    {
      id: "discord",
      title: "Discord",
      desc: "Плотный, компактный, строгие радиусы.",
      radius: "6px",
      density: "0.9",
      shadow: "0 4px 10px rgba(0,0,0,.22)",
      blur: "0px",
    },
    {
      id: "legacy",
      title: "Legacy 1.1",
      desc: "Визуал старого лаунчера — тени, свечения, орбы.",
      radius: "12px",
      density: "1",
      shadow: "0 18px 40px rgba(0,0,0,.45)",
      blur: "0px",
    },
    {
      id: "glass",
      title: "Glass (experimental)",
      desc: "Акрил + backdrop-blur. Требует GPU.",
      radius: "14px",
      density: "1",
      shadow: "0 20px 40px rgba(0,0,0,.35)",
      blur: "16px",
    },
  ];

  function pickPreset(p: VisualPreset) {
    chosenPreset = p;
    applyVisualPreset(p);
  }

  const motionOptions: { id: boolean; title: string; desc: string }[] = [
    {
      id: false,
      title: "Всё включено",
      desc: "Плавные переходы, микроанимации и эффекты.",
    },
    {
      id: true,
      title: "Спокойный режим",
      desc: "Отключает анимации. Интерфейс становится заметно быстрее на слабых ПК.",
    },
  ];
</script>

<div
  class="fixed inset-0 z-[15000] flex items-center justify-center p-6 jm-onboarding-root {chosenReduceMotion
    ? 'jm-reduce-motion'
    : ''}"
  style:background="var(--bg)"
  style:color="var(--text)"
>
  <div class="absolute inset-0 pointer-events-none overflow-hidden">
    <div
      class="absolute -top-24 left-1/2 -translate-x-1/2 w-[520px] h-[260px] rounded-full jm-onboarding-orb"
      style:background="radial-gradient(ellipse at center, color-mix(in srgb, var(--accent) 15%, transparent), transparent 70%)"
    ></div>
    <div
      class="absolute inset-x-0 bottom-0 h-[120px]"
      style:background="linear-gradient(to top, color-mix(in srgb, var(--accent) 6%, transparent), transparent)"
    ></div>
  </div>

  <div
    class="relative w-full max-w-[920px] rounded-[var(--radius-lg)] border overflow-hidden flex flex-col"
    style:background="var(--card)"
    style:border-color="var(--border)"
    style="max-height: 90vh;"
  >
    <!-- Header with stepper -->
    <header
      class="flex items-center gap-4 px-6 py-4 border-b"
      style:border-color="var(--border)"
    >
      <div
        class="w-9 h-9 flex items-center justify-center rounded-[var(--radius-sm)]"
        style:background="color-mix(in srgb, var(--accent) 15%, transparent)"
        style:color="var(--accent-light)"
      >
        <Sparkles size={18} />
      </div>
      <div class="flex-1 min-w-0">
        <div class="text-[11px] uppercase tracking-[0.22em]" style:color="var(--text-secondary)">
          Первичная настройка
        </div>
        <div class="text-[15px] font-semibold truncate">{step.label}</div>
      </div>
      <button
        type="button"
        on:click={skip}
        disabled={saving}
        class="ui-btn ui-btn-subtle h-8 text-[12px] px-3 disabled:opacity-50"
      >
        Пропустить
      </button>
    </header>

    <!-- Stepper dots -->
    <div class="flex items-center gap-1.5 px-6 py-3 border-b" style:border-color="var(--border)">
      {#each steps as s, i (s.id)}
        <button
          type="button"
          on:click={() => (stepIndex = i)}
          class="flex-1 h-1 rounded-full transition-colors"
          style:background={i <= stepIndex
            ? "var(--accent)"
            : "color-mix(in srgb, var(--text) 10%, transparent)"}
          aria-label={`Шаг ${i + 1}: ${s.label}`}
        ></button>
      {/each}
    </div>

    <!-- Body -->
    <div class="relative flex-1 overflow-auto custom-scrollbar">
      {#key step.id}
        <div
          class="p-7 flex flex-col gap-5"
          in:fly={{ x: 24, duration: 240, opacity: 0 }}
        >
          {#if step.id === "welcome"}
            <div class="flex flex-col items-center text-center gap-3 py-6">
              <div
                class="w-20 h-20 rounded-[var(--radius-lg)] flex items-center justify-center border"
                style:background="color-mix(in srgb, var(--accent) 10%, var(--surface-1))"
                style:border-color="color-mix(in srgb, var(--accent) 35%, transparent)"
              >
                <Sparkles size={34} style="color: var(--accent-light)" />
              </div>
              <h1 class="text-2xl font-semibold tracking-tight">Добро пожаловать!</h1>
              <p
                class="text-[14px] max-w-[440px] leading-relaxed"
                style:color="var(--text-secondary)"
              >
                Настроим тему, панель навигации (в том числе вкладки сверху как раньше),
                масштаб, модальные окна и расположение индикаторов — без лишней суеты. Всё
                можно изменить позже в «Расширенные» → «Оформление».
              </p>
            </div>
          {:else if step.id === "theme"}
            <div class="flex items-center gap-2">
              <Palette size={16} style="color: var(--accent)" />
              <h2 class="text-[15px] font-semibold">Выберите тему</h2>
            </div>
            <p class="text-[13px]" style:color="var(--text-secondary)">
              Цветовую схему можно менять в любой момент в «Настройках».
            </p>
            <div class="grid grid-cols-2 sm:grid-cols-4 gap-2.5">
              {#each builtinThemes as t (t.id)}
                {@const active = chosenTheme === t.id}
                <button
                  type="button"
                  on:click={() => pickTheme(t)}
                  class="group relative flex flex-col gap-2 p-2.5 rounded-[var(--radius-sm)] border transition-colors text-left"
                  style:background={active
                    ? "color-mix(in srgb, var(--accent) 12%, var(--surface-1))"
                    : "var(--surface-1)"}
                  style:border-color={active ? "var(--accent)" : "var(--border)"}
                >
                  <div
                    class="w-full h-16 rounded-[var(--radius-sm)] flex items-end p-1.5 gap-1 overflow-hidden"
                    style:background={t.preview.bg}
                  >
                    <span
                      class="flex-1 h-2.5 rounded-full"
                      style:background={t.preview.accent}
                    ></span>
                    <span
                      class="w-4 h-4 rounded-[4px]"
                      style:background={t.preview.card}
                    ></span>
                  </div>
                  <div class="flex items-center justify-between gap-1">
                    <span class="text-[12px] font-medium truncate">{t.name}</span>
                    {#if active}
                      <Check size={13} style="color: var(--accent)" />
                    {/if}
                  </div>
                </button>
              {/each}
            </div>
          {:else if step.id === "preset"}
            <div class="flex items-center gap-2">
              <Sparkles size={16} style="color: var(--accent)" />
              <h2 class="text-[15px] font-semibold">Визуальный стиль</h2>
            </div>
            <p class="text-[13px]" style:color="var(--text-secondary)">
              Определяет плотность, радиусы, тени и анимации. Цветовую палитру уже выбрали на
              прошлом шаге — эти оси независимы.
            </p>
            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2.5">
              {#each presetOptions as p (p.id)}
                {@const active = chosenPreset === p.id}
                <button
                  type="button"
                  on:click={() => pickPreset(p.id)}
                  class="group relative flex flex-col gap-2 p-3 rounded-[var(--radius-sm)] border text-left transition-colors"
                  style:background={active
                    ? "color-mix(in srgb, var(--accent) 12%, var(--surface-1))"
                    : "var(--surface-1)"}
                  style:border-color={active ? "var(--accent)" : "var(--border)"}
                >
                  <div
                    class="w-full h-20 flex items-end gap-1.5 p-2 overflow-hidden"
                    style:border-radius={p.radius}
                    style:background="color-mix(in srgb, var(--accent) 7%, var(--surface-2, var(--surface-1)))"
                    style:box-shadow={p.shadow}
                    style:backdrop-filter={p.blur !== "0px" ? `blur(${p.blur})` : "none"}
                  >
                    <span
                      class="flex-1 rounded-[6px]"
                      style:height={`calc(24px * ${p.density})`}
                      style:background="var(--accent)"
                    ></span>
                    <span
                      class="rounded-[6px] opacity-80"
                      style:width="22%"
                      style:height={`calc(18px * ${p.density})`}
                      style:background="var(--accent-light, var(--accent))"
                    ></span>
                    <span
                      class="rounded-[6px] opacity-60"
                      style:width="14%"
                      style:height={`calc(12px * ${p.density})`}
                      style:background="color-mix(in srgb, var(--text) 35%, transparent)"
                    ></span>
                  </div>
                  <div class="flex items-center justify-between gap-1">
                    <span class="text-[12px] font-semibold truncate">{p.title}</span>
                    {#if active}<Check size={13} style="color: var(--accent)" />{/if}
                  </div>
                  <span
                    class="text-[10px] leading-snug"
                    style:color="var(--text-secondary)"
                  >{p.desc}</span>
                </button>
              {/each}
            </div>
          {:else if step.id === "chrome"}
            <div class="flex items-center gap-2">
              <LayoutGrid size={16} style="color: var(--accent)" />
              <h2 class="text-[15px] font-semibold">Панель навигации</h2>
            </div>
            <p class="text-[13px]" style:color="var(--text-secondary)">
              Боковая слева/справа, компакт или полный текст, либо горизонтальные вкладки
              сверху или снизу — как вам удобнее на вашем мониторе.
            </p>
            <div class="grid grid-cols-2 sm:grid-cols-3 gap-2.5">
              {#each chromePick as L (L.id)}
                {@const active = chosenChrome === L.id}
                <button
                  type="button"
                  on:click={() => (chosenChrome = L.id)}
                  class="flex flex-col gap-2 p-3 rounded-[var(--radius-sm)] border text-left transition-colors min-h-[100px]"
                  style:background={active
                    ? "color-mix(in srgb, var(--accent) 10%, var(--surface-1))"
                    : "var(--surface-1)"}
                  style:border-color={active ? "var(--accent)" : "var(--border)"}
                >
                  <div
                    class="w-full h-12 rounded-[6px] border relative overflow-hidden shrink-0"
                    style:border-color="var(--border)"
                    style:background="color-mix(in srgb, var(--text) 6%, transparent)"
                  >
                    {#if L.id.startsWith("sidebar_left")}
                      <div class="absolute inset-y-1 left-1 w-[24%] rounded-[4px]" style:background="var(--accent-soft)"></div>
                    {:else if L.id.startsWith("sidebar_right")}
                      <div class="absolute inset-y-1 right-1 w-[24%] rounded-[4px]" style:background="var(--accent-soft)"></div>
                    {:else if L.id === "top_tabs"}
                      <div class="absolute top-1 left-1 right-1 h-[28%] rounded-[4px]" style:background="var(--accent-soft)"></div>
                    {:else}
                      <div class="absolute bottom-1 left-1 right-1 h-[28%] rounded-[4px]" style:background="var(--accent-soft)"></div>
                    {/if}
                  </div>
                  <div class="flex items-start justify-between gap-1">
                    <span class="text-[12px] font-semibold leading-snug">{L.title}</span>
                    {#if active}
                      <Check size={14} class="shrink-0" style="color: var(--accent)" />
                    {/if}
                  </div>
                  <span class="text-[11px] leading-snug" style:color="var(--text-secondary)">{L.desc}</span>
                </button>
              {/each}
            </div>
          {:else if step.id === "scale"}
            <div class="flex items-center gap-2">
              <Gauge size={16} style="color: var(--accent)" />
              <h2 class="text-[15px] font-semibold">Масштаб интерфейса</h2>
            </div>
            <p class="text-[13px]" style:color="var(--text-secondary)">
              Настройте размер элементов. Для превью масштаб применяется сразу.
            </p>
            <div
              class="flex flex-col gap-3 p-4 rounded-[var(--radius-sm)] border"
              style:background="var(--surface-1)"
              style:border-color="var(--border)"
            >
              <div class="flex items-baseline justify-between gap-3">
                <span class="text-[12px]" style:color="var(--text-secondary)">
                  Текущий масштаб
                </span>
                <span class="text-[20px] font-semibold tabular-nums">
                  {Math.round(chosenScale * 100)}%
                </span>
              </div>
              <input
                type="range"
                min="0.85"
                max="1.6"
                step="0.05"
                bind:value={chosenScale}
                on:input={() => applyScalePreview(chosenScale)}
                class="w-full accent-[var(--accent)]"
              />
              <div class="flex flex-wrap gap-1.5">
                {#each scalePresets as p (p.v)}
                  <button
                    type="button"
                    on:click={() => {
                      chosenScale = p.v;
                      applyScalePreview(chosenScale);
                    }}
                    class="ui-btn ui-btn-subtle h-7 text-[11px] px-2.5"
                    aria-pressed={Math.abs(chosenScale - p.v) < 0.001}
                  >
                    {p.l}
                  </button>
                {/each}
              </div>
            </div>
          {:else if step.id === "modals"}
            <div class="flex items-center gap-2">
              <AppWindow size={16} style="color: var(--accent)" />
              <h2 class="text-[15px] font-semibold">Модальные окна и оверлеи</h2>
            </div>
            <p class="text-[13px]" style:color="var(--text-secondary)">
              Затемнение фона, тени и размытие для встроенного браузера и других полноэкранных панелей.
            </p>
            <div class="grid grid-cols-2 sm:grid-cols-4 gap-2">
              {#each modalStepOptions as m (m.id)}
                {@const active = chosenModal === m.id}
                <button
                  type="button"
                  on:click={() => (chosenModal = m.id)}
                  class="flex flex-col gap-1 p-3 rounded-[var(--radius-sm)] border text-left transition-colors"
                  style:background={active ? "color-mix(in srgb, var(--accent) 12%, var(--surface-1))" : "var(--surface-1)"}
                  style:border-color={active ? "var(--accent)" : "var(--border)"}
                >
                  <span class="text-[13px] font-semibold">{m.t}</span>
                  <span class="text-[11px]" style:color="var(--text-secondary)">{m.d}</span>
                </button>
              {/each}
            </div>
          {:else if step.id === "hud"}
            <div class="flex items-center gap-2">
              <Crosshair size={16} style="color: var(--accent)" />
              <h2 class="text-[15px] font-semibold">Индикатор загрузки и уведомления</h2>
            </div>
            <p class="text-[13px]" style:color="var(--text-secondary)">
              Виджет круговой загрузки и тосты используют один угол экрана — выберите удобный.
            </p>
            <div class="flex flex-wrap gap-2">
              {#each hudOptions as h (h.id)}
                {@const active = chosenDownload === h.id}
                <button
                  type="button"
                  on:click={() => (chosenDownload = h.id)}
                  class="ui-btn px-4 py-2 text-[12px]"
                  class:ui-btn-primary={active}
                  class:ui-btn-subtle={!active}
                >
                  {h.label}
                </button>
              {/each}
            </div>
          {:else if step.id === "motion"}
            <div class="flex items-center gap-2">
              <Zap size={16} style="color: var(--accent)" />
              <h2 class="text-[15px] font-semibold">Анимации и эффекты</h2>
            </div>
            <p class="text-[13px]" style:color="var(--text-secondary)">
              Если вы чувствительны к движению или у вас слабое железо — отключите
              плавные переходы.
            </p>
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
              {#each motionOptions as opt (String(opt.id))}
                {@const active = chosenReduceMotion === opt.id}
                <button
                  type="button"
                  on:click={() => (chosenReduceMotion = opt.id)}
                  class="p-4 rounded-[var(--radius-sm)] border text-left transition-colors flex flex-col gap-1"
                  style:background={active
                    ? "color-mix(in srgb, var(--accent) 10%, var(--surface-1))"
                    : "var(--surface-1)"}
                  style:border-color={active ? "var(--accent)" : "var(--border)"}
                >
                  <div class="flex items-center gap-2">
                    <span class="text-[14px] font-semibold">{opt.title}</span>
                    {#if active}
                      <Check size={14} style="color: var(--accent)" />
                    {/if}
                  </div>
                  <span class="text-[12px]" style:color="var(--text-secondary)">
                    {opt.desc}
                  </span>
                </button>
              {/each}
            </div>
          {:else if step.id === "finish"}
            <div class="flex flex-col items-center text-center gap-3 py-4">
              <div
                class="w-20 h-20 rounded-[var(--radius-lg)] flex items-center justify-center border"
                style:background="color-mix(in srgb, var(--accent) 10%, var(--surface-1))"
                style:border-color="color-mix(in srgb, var(--accent) 35%, transparent)"
              >
                <PartyPopper size={34} style="color: var(--accent-light)" />
              </div>
              <h2 class="text-xl font-semibold tracking-tight">Готово к запуску</h2>
              <p class="text-[13px]" style:color="var(--text-secondary)">
                Позже всё можно поменять в
                <span class="text-[var(--text)]">Расширенные настройки → Оформление</span>.
              </p>
              <div
                class="mt-2 w-full rounded-[var(--radius-sm)] border p-4 flex flex-col gap-2 text-[12px]"
                style:background="var(--surface-1)"
                style:border-color="var(--border)"
              >
                <div class="flex justify-between gap-3">
                  <span style:color="var(--text-secondary)">Тема</span>
                  <span class="font-medium">
                    {builtinThemes.find((t) => t.id === chosenTheme)?.name ?? chosenTheme}
                  </span>
                </div>
                <div class="flex justify-between gap-3">
                  <span style:color="var(--text-secondary)">Навигация</span>
                  <span class="font-medium text-right">
                    {chromePick.find((c) => c.id === chosenChrome)?.title ?? chosenChrome}
                  </span>
                </div>
                <div class="flex justify-between gap-3">
                  <span style:color="var(--text-secondary)">Модалки</span>
                  <span class="font-medium">
                    {chosenModal === "minimal"
                      ? "Минимум"
                      : chosenModal === "glass"
                        ? "Стекло"
                        : chosenModal === "dense"
                          ? "Плотный"
                          : "Шит"}
                  </span>
                </div>
                <div class="flex justify-between gap-3">
                  <span style:color="var(--text-secondary)">Индикатор</span>
                  <span class="font-medium">
                    {chosenDownload === "bl"
                      ? "Низ-слева"
                      : chosenDownload === "br"
                        ? "Низ-справа"
                        : chosenDownload === "tl"
                          ? "Верх-слева"
                          : chosenDownload === "tr"
                            ? "Верх-справа"
                            : "Скрыт"}
                  </span>
                </div>
                <div class="flex justify-between gap-3">
                  <span style:color="var(--text-secondary)">Масштаб</span>
                  <span class="font-medium tabular-nums">
                    {Math.round(chosenScale * 100)}%
                  </span>
                </div>
                <div class="flex justify-between gap-3">
                  <span style:color="var(--text-secondary)">Анимации</span>
                  <span class="font-medium">
                    {chosenReduceMotion ? "Выключены" : "Включены"}
                  </span>
                </div>
              </div>
            </div>
          {/if}
        </div>
      {/key}
    </div>

    <!-- Footer nav -->
    <footer
      class="flex items-center gap-3 px-6 py-4 border-t"
      style:border-color="var(--border)"
      style:background="color-mix(in srgb, var(--bg) 50%, transparent)"
    >
      <button
        type="button"
        on:click={prevStep}
        disabled={isFirst || saving}
        class="ui-btn ui-btn-subtle h-9 px-3 disabled:opacity-40"
      >
        <ChevronLeft size={14} />
        <span>Назад</span>
      </button>
      <div class="flex-1 text-[12px] text-center" style:color="var(--text-secondary)">
        Шаг {stepIndex + 1} из {steps.length}
      </div>
      {#if isLast}
        <button
          type="button"
          on:click={finish}
          disabled={saving}
          class="ui-btn ui-btn-primary h-9 px-4 disabled:opacity-60"
        >
          <Check size={14} />
          <span>{saving ? "Сохранение…" : "Завершить"}</span>
        </button>
      {:else}
        <button
          type="button"
          on:click={nextStep}
          class="ui-btn ui-btn-primary h-9 px-4"
        >
          <span>Далее</span>
          <ChevronRight size={14} />
        </button>
      {/if}
    </footer>
  </div>
</div>

<style>
  @keyframes jm-onboarding-orb {
    0%, 100% { transform: translate(-50%, 0) scale(1); opacity: 0.7; }
    50% { transform: translate(-50%, -6%) scale(1.06); opacity: 1; }
  }
  .jm-onboarding-orb {
    animation: jm-onboarding-orb 10s ease-in-out infinite;
  }
  :global(.jm-reduce-motion) .jm-onboarding-orb {
    animation: none !important;
  }
</style>
