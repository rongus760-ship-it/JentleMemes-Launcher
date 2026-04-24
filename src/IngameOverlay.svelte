<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { PhysicalPosition, PhysicalSize } from "@tauri-apps/api/dpi";
  import {
    Cpu,
    Gamepad2,
    HardDrive,
    Layers,
    X,
    Plus,
    LayoutGrid,
    Sparkles,
    Square,
    Camera,
    ScrollText,
    GripVertical,
    Copy,
  } from "lucide-svelte";
  import { applyTheme, normalizeOverlayLayout, applyOverlayLayout } from "./lib/themeApply";
  import { LAUNCHER_VERSION } from "./version";
  import OverlayChatFeed from "./components/OverlayChatFeed.svelte";
  import {
    WIDGET_CATALOG,
    loadOverlayLayout,
    saveOverlayLayout,
    defaultLayout,
    randomOverlayWidgetId,
    type OverlayLayoutV2,
    type OverlayWidgetKind,
    OVERLAY_TIPS,
    MC_FACTS,
    SEED_IDEAS,
  } from "./lib/overlayDashboard";

  type OverlayRect = { x: number; y: number; width: number; height: number; source: string };
  type GameOverlayStats = {
    sessions: number;
    instance_ids: string[];
    memory_used_mb: number;
    cpu_percent_total: number;
    pids: number[];
  };

  let gameRunning = false;
  let rectSource = "";
  let stats: GameOverlayStats | null = null;
  let pollTimeout: ReturnType<typeof setTimeout> | null = null;
  let pollStop = false;
  let lastRectKey = "";
  let syncInFlight = false;
  // Интервал опроса (мс). При наведении окна — быстрее, когда фон/спрятан — реже.
  const POLL_ACTIVE = 800;
  const POLL_IDLE = 1600;
  let rotTick = 0;
  let clockTick = 0;
  let rotTimer: ReturnType<typeof setInterval> | null = null;
  let clockTimer: ReturnType<typeof setInterval> | null = null;
  let socialIv: ReturnType<typeof setInterval> | null = null;

  let layout: OverlayLayoutV2 = loadOverlayLayout(false);
  let pickerOpen = false;
  let chatTabEnabled = false;
  let apiBase = "https://jentlememes.ru";
  let pingMs: number | null = null;
  let friendsCount = 0;
  let convCount = 0;
  let instancesShort: { name?: string; id: string }[] = [];
  // Phase 4 (2.0): control panel + log tail + DnD
  let runningInstanceIds: string[] = [];
  let logVisible = false;
  let logText = "";
  let logTimer: ReturnType<typeof setInterval> | null = null;
  let toastMsg = "";
  let toastTimer: ReturnType<typeof setTimeout> | null = null;
  let dragIndex: number | null = null;
  let dragOverIndex: number | null = null;
  let busyAction: "stop" | "shot" | null = null;

  const TOKEN_KEY = "jm_social_access_token";

  function catalogMeta(kind: OverlayWidgetKind) {
    return WIDGET_CATALOG.find((c) => c.kind === kind);
  }

  function applyThemeFromStorage() {
    try {
      const raw = localStorage.getItem("jm_overlay_theme_v1");
      if (!raw) return;
      const data = JSON.parse(raw) as Record<string, string>;
      const el = document.documentElement;
      // Восстанавливаем прежний className, но обязательно сохраняем
      // jm-overlay-mode — его проставляет main.ts до гидратации, и без
      // него наш CSS-reset прозрачности не применится.
      if (data.className != null) {
        const hadOverlayMode = el.classList.contains("jm-overlay-mode");
        el.className = data.className;
        if (hadOverlayMode) el.classList.add("jm-overlay-mode");
      }
      if (data.preset) el.setAttribute("data-preset", data.preset);
      if (data.overlayLayout) {
        applyOverlayLayout(normalizeOverlayLayout(data.overlayLayout));
      }
      for (const [k, v] of Object.entries(data)) {
        if (k === "className" || k === "preset" || k === "shellLayout" || k === "overlayLayout") continue;
        if (v) el.style.setProperty(k, v);
      }
    } catch {
      /* ignore */
    }
  }

  async function hydrateTheme() {
    const snap = localStorage.getItem("jm_overlay_theme_v1");
    if (snap) {
      applyThemeFromStorage();
      return;
    }
    try {
      const s: Record<string, unknown> = await invoke("load_settings");
      await applyTheme(String(s.theme || "jentle-dark"), String(s.background || ""));
    } catch {
      document.documentElement.classList.add("theme-jentle-dark");
    }
  }

  async function hydrateSettings() {
    try {
      const s: Record<string, unknown> = await invoke("load_settings");
      chatTabEnabled = !!s.show_friends_chat_tab;
      apiBase = String(s.jentlememes_api_base_url || "https://jentlememes.ru").replace(/\/$/, "");
      layout = loadOverlayLayout(chatTabEnabled);
      applyOverlayLayout(normalizeOverlayLayout(s.overlay_layout));
    } catch {
      layout = loadOverlayLayout(false);
      applyOverlayLayout("panel");
    }
  }

  async function syncFrame() {
    // Защита от наложения тиков (если X11/sysinfo тормозят — просто пропускаем кадр).
    if (syncInFlight) return;
    syncInFlight = true;
    try {
      const [rectRes, statsRes, runRes, idsRes] = await Promise.allSettled([
        invoke<OverlayRect>("get_overlay_target_rect"),
        invoke<GameOverlayStats>("get_game_overlay_stats"),
        invoke<boolean>("is_game_session_running"),
        invoke<string[]>("get_running_instance_ids"),
      ]);

      if (rectRes.status === "fulfilled") {
        const r = rectRes.value;
        rectSource = r.source;
        const w = Math.max(320, r.width);
        const h = Math.max(240, r.height);
        const key = `${r.x}x${r.y}x${w}x${h}`;
        // Жмём setPosition / setSize только если прямоугольник действительно
        // изменился — иначе вызываем лишние IPC-раундтрипы, которые
        // на Linux/X11 могут ронять окно лаунчера в ожидание.
        if (key !== lastRectKey) {
          lastRectKey = key;
          try {
            const win = getCurrentWebviewWindow();
            await Promise.all([
              win.setPosition(new PhysicalPosition(r.x, r.y)),
              win.setSize(new PhysicalSize(w, h)),
            ]);
          } catch {
            /* ignore */
          }
        }
      }

      stats = statsRes.status === "fulfilled" ? statsRes.value : null;
      gameRunning = runRes.status === "fulfilled" ? runRes.value : false;
      runningInstanceIds = idsRes.status === "fulfilled" ? idsRes.value : [];
    } finally {
      syncInFlight = false;
    }
  }

  function schedulePoll() {
    if (pollStop) return;
    if (pollTimeout) clearTimeout(pollTimeout);
    const hidden =
      typeof document !== "undefined" && document.visibilityState === "hidden";
    const delay = hidden ? POLL_IDLE : POLL_ACTIVE;
    pollTimeout = setTimeout(async () => {
      if (pollStop) return;
      await syncFrame();
      schedulePoll();
    }, delay);
  }

  function showToast(msg: string, ms = 2800) {
    toastMsg = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      toastMsg = "";
    }, ms);
  }

  async function doStopGame() {
    busyAction = "stop";
    try {
      const id = runningInstanceIds[0] ?? null;
      await invoke("stop_game_from_overlay", { instanceId: id });
      showToast("Игра остановлена");
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    } finally {
      busyAction = null;
    }
  }

  async function doScreenshot() {
    busyAction = "shot";
    try {
      const id = runningInstanceIds[0] ?? null;
      const p = await invoke<string>("take_minecraft_screenshot", { instanceId: id });
      showToast(`Скриншот: ${p.split(/[\\/]/).pop()}`);
    } catch (e) {
      showToast(`Скриншот не удался: ${e}`);
    } finally {
      busyAction = null;
    }
  }

  async function refreshLogTail() {
    const id = runningInstanceIds[0];
    if (!id) {
      logText = "Нет активного инстанса.";
      return;
    }
    try {
      logText = await invoke<string>("tail_game_log", { instanceId: id, lines: 80 });
    } catch (e) {
      logText = `Ошибка чтения логов: ${e}`;
    }
  }

  function toggleLog() {
    logVisible = !logVisible;
    if (logVisible) {
      void refreshLogTail();
      if (logTimer) clearInterval(logTimer);
      logTimer = setInterval(() => void refreshLogTail(), 1500);
    } else if (logTimer) {
      clearInterval(logTimer);
      logTimer = null;
    }
  }

  async function copyLogTail() {
    try {
      await navigator.clipboard.writeText(logText);
      showToast("Логи скопированы");
    } catch (e) {
      showToast(`Не скопировано: ${e}`);
    }
  }

  function onDragStart(i: number, ev: DragEvent) {
    dragIndex = i;
    if (ev.dataTransfer) {
      ev.dataTransfer.effectAllowed = "move";
      ev.dataTransfer.setData("text/plain", String(i));
    }
  }
  function onDragOver(i: number, ev: DragEvent) {
    ev.preventDefault();
    if (ev.dataTransfer) ev.dataTransfer.dropEffect = "move";
    dragOverIndex = i;
  }
  function onDragEnd() {
    dragIndex = null;
    dragOverIndex = null;
  }
  function onDrop(i: number, ev: DragEvent) {
    ev.preventDefault();
    if (dragIndex == null || dragIndex === i) {
      dragIndex = null;
      dragOverIndex = null;
      return;
    }
    const widgets = [...layout.widgets];
    const [moved] = widgets.splice(dragIndex, 1);
    widgets.splice(i, 0, moved);
    layout = { ...layout, widgets };
    persistLayout();
    dragIndex = null;
    dragOverIndex = null;
  }

  async function loadSocialExtras() {
    const base = apiBase.replace(/\/$/, "");
    let token = "";
    try {
      token = localStorage.getItem(TOKEN_KEY) || "";
    } catch {
      token = "";
    }
    if (token && chatTabEnabled) {
      try {
        const t0 = performance.now();
        const r = await fetch(`${base}/api/v1/me`, {
          headers: { Authorization: `Bearer ${token}` },
        });
        pingMs = Math.round(performance.now() - t0);
        if (r.ok) {
          const fr = await fetch(`${base}/api/v1/friends`, {
            headers: { Authorization: `Bearer ${token}` },
          });
          if (fr.ok) {
            const fj = await fr.json();
            const list = fj.friends || fj || [];
            friendsCount = Array.isArray(list) ? list.length : 0;
          }
          const cr = await fetch(`${base}/api/v1/conversations`, {
            headers: { Authorization: `Bearer ${token}` },
          });
          if (cr.ok) {
            const cj = await cr.json();
            const list = cj.conversations || cj || [];
            convCount = Array.isArray(list) ? list.length : 0;
          }
        }
      } catch {
        pingMs = null;
      }
    } else {
      try {
        const t0 = performance.now();
        await fetch(`${base}/api/v1/me`, { method: "GET" });
        pingMs = Math.round(performance.now() - t0);
      } catch {
        pingMs = null;
      }
    }
    try {
      const list = (await invoke("get_instances")) as { id: string; name?: string }[];
      instancesShort = Array.isArray(list) ? list.slice(0, 8) : [];
    } catch {
      instancesShort = [];
    }
  }

  async function closeSelf() {
    try {
      await getCurrentWebviewWindow().hide();
    } catch {
      /* ignore */
    }
  }

  function onStorage(e: StorageEvent) {
    if (e.key === "jm_overlay_theme_v1") applyThemeFromStorage();
  }

  function srcLabel(s: string): string {
    if (s === "game") return "окно Minecraft";
    if (s === "monitor") return "основной монитор";
    return "запасной режим";
  }

  function persistLayout() {
    saveOverlayLayout(layout);
  }

  function removeWidget(id: string) {
    layout = { ...layout, widgets: layout.widgets.filter((w) => w.id !== id) };
    persistLayout();
  }

  function addWidget(kind: OverlayWidgetKind) {
    if (layout.widgets.some((w) => w.kind === kind)) return;
    layout = {
      ...layout,
      widgets: [...layout.widgets, { id: randomOverlayWidgetId(), kind }],
    };
    persistLayout();
    pickerOpen = false;
  }

  function resetLayout() {
    layout = defaultLayout(chatTabEnabled);
    persistLayout();
    pickerOpen = false;
  }

  $: tipText = OVERLAY_TIPS[rotTick % OVERLAY_TIPS.length];
  $: factText = MC_FACTS[rotTick % MC_FACTS.length];
  $: seedText = SEED_IDEAS[rotTick % SEED_IDEAS.length];

  // Умеренно-прозрачная радиальная виньетка вместо плотного градиента.
  // Strength=0 → почти невидимый фон; Strength=1 → ~55% в углах, центр чист.
  $: backdropStyle = `radial-gradient(120% 100% at 50% 50%,
    rgba(0, 0, 0, 0) 0%,
    rgba(6, 10, 18, ${Math.min(0.55, layout.backdropStrength * 0.35)}) 70%,
    rgba(2, 5, 10, ${Math.min(0.72, layout.backdropStrength * 0.55)}) 100%)`;

  function onVisibilityChange() {
    // При возврате окна в фокус сразу перезапускаем цикл с быстрым интервалом.
    if (pollTimeout) clearTimeout(pollTimeout);
    schedulePoll();
  }

  onMount(() => {
    void hydrateTheme();
    void hydrateSettings();
    void loadSocialExtras();
    window.addEventListener("storage", onStorage);
    document.addEventListener("visibilitychange", onVisibilityChange);
    // Сразу один тик, затем самоперепланирующийся цикл вместо жёсткого
    // setInterval — это устраняет наложение тиков (прежний 400 мс цикл
    // стэкался, пока X11-обход всё ещё висел, и замораживал лаунчер).
    void (async () => {
      await syncFrame();
      schedulePoll();
    })();
    rotTimer = setInterval(() => {
      rotTick++;
    }, 4500);
    clockTimer = setInterval(() => {
      clockTick++;
    }, 1000);
    socialIv = setInterval(() => void loadSocialExtras(), 12_000);
  });

  onDestroy(() => {
    pollStop = true;
    window.removeEventListener("storage", onStorage);
    document.removeEventListener("visibilitychange", onVisibilityChange);
    if (pollTimeout) clearTimeout(pollTimeout);
    if (rotTimer) clearInterval(rotTimer);
    if (clockTimer) clearInterval(clockTimer);
    if (socialIv) clearInterval(socialIv);
    if (logTimer) clearInterval(logTimer);
    if (toastTimer) clearTimeout(toastTimer);
  });
</script>

<div
  class="jm-ingame-overlay-root h-screen w-full relative overflow-hidden"
  style="color: var(--text, #fff);"
>
  <button
    type="button"
    class="absolute inset-0 z-0 border-0 p-0 cursor-default"
    style="background: {backdropStyle};"
    aria-label="Закрыть оверлей"
    on:click={() => void closeSelf()}
  ></button>

  <div
    class="relative z-10 h-full min-h-0 flex flex-col items-stretch p-2 sm:p-4 pt-3 sm:pt-6 overflow-hidden pointer-events-none"
  >
    <div
      class="pointer-events-auto shrink-0 flex flex-wrap items-center justify-between gap-2 mb-2 px-1"
    >
      <div class="flex items-center gap-2 min-w-0">
        <Sparkles size={18} class="shrink-0 text-jm-accent-light opacity-90" />
        <div class="min-w-0">
          <p class="text-sm font-bold truncate" style="color: var(--accent-light, #b4dbb4);">
            JentleMemes оверлей
          </p>
          <p class="text-[10px] opacity-65 truncate">{srcLabel(rectSource)} · v{LAUNCHER_VERSION}</p>
        </div>
      </div>
      <div class="flex items-center gap-1.5 flex-wrap justify-end">
        <button
          type="button"
          disabled={!gameRunning || busyAction === "stop"}
          class="flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-[11px] font-semibold border transition-colors hover:bg-red-500/15 disabled:opacity-40 disabled:pointer-events-none"
          style="border-color: color-mix(in srgb, #ef4444 60%, var(--border, rgba(255,255,255,0.15))); color: #fca5a5;"
          title="Остановить игру"
          on:click={() => void doStopGame()}
        >
          <Square size={14} />
          <span class="hidden sm:inline">Стоп</span>
        </button>
        <button
          type="button"
          disabled={busyAction === "shot"}
          class="flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-[11px] font-semibold border transition-colors hover:bg-white/10 disabled:opacity-50"
          style="border-color: var(--border, rgba(255,255,255,0.15)); color: var(--text, #e5e7eb);"
          title="Скриншот окна (grim/scrot/powershell)"
          on:click={() => void doScreenshot()}
        >
          <Camera size={14} />
          <span class="hidden sm:inline">Скрин</span>
        </button>
        <button
          type="button"
          class="flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-[11px] font-semibold border transition-colors hover:bg-white/10"
          style="border-color: var(--border, rgba(255,255,255,0.15)); color: {logVisible ? 'var(--accent-light, #b4dbb4)' : 'var(--text-secondary, #9ca3af)'};"
          title="Показать/скрыть хвост логов"
          on:click={toggleLog}
        >
          <ScrollText size={14} />
          <span class="hidden sm:inline">Логи</span>
        </button>
        <label class="flex items-center gap-2 text-[10px] opacity-80 cursor-pointer">
          <span class="hidden sm:inline">Затемнение</span>
          <input
            type="range"
            min="0"
            max="100"
            value={Math.round(layout.backdropStrength * 100)}
            on:input={(e) => {
              layout = {
                ...layout,
                backdropStrength: Number(e.currentTarget.value) / 100,
              };
              persistLayout();
            }}
            class="w-24 accent-jm-accent"
          />
        </label>
        <button
          type="button"
          class="flex items-center gap-1 px-2.5 py-1.5 rounded-lg text-[11px] font-semibold border transition-colors hover:bg-white/10"
          style="border-color: var(--border, rgba(255,255,255,0.15)); color: var(--text-secondary, #9ca3af);"
          on:click={() => (pickerOpen = !pickerOpen)}
        >
          <LayoutGrid size={15} />
          <span class="hidden sm:inline">Панели</span>
        </button>
        <button
          type="button"
          class="p-2 rounded-xl transition-colors hover:bg-white/10"
          style="color: var(--text-secondary, #9ca3af);"
          title="Скрыть"
          aria-label="Скрыть"
          on:click={() => void closeSelf()}
        >
          <X size={20} />
        </button>
      </div>
    </div>

    {#if logVisible}
      <div
        class="pointer-events-auto mb-2 rounded-xl border backdrop-blur-md shrink-0"
        style="border-color: var(--border); background: color-mix(in srgb, var(--card, #0f1115) 92%, transparent);"
      >
        <div
          class="flex items-center justify-between gap-2 px-3 py-1.5 border-b"
          style="border-color: var(--border, rgba(255,255,255,0.08));"
        >
          <p class="text-[11px] font-bold opacity-90 flex items-center gap-2">
            <ScrollText size={13} />
            latest.log — хвост {runningInstanceIds[0] ? `(${runningInstanceIds[0].slice(0, 8)}…)` : ""}
          </p>
          <div class="flex items-center gap-1">
            <button
              type="button"
              class="p-1 rounded-lg hover:bg-white/10"
              title="Скопировать"
              aria-label="Скопировать логи"
              on:click={() => void copyLogTail()}
            >
              <Copy size={12} />
            </button>
            <button
              type="button"
              class="p-1 rounded-lg hover:bg-white/10"
              title="Закрыть панель логов"
              aria-label="Закрыть"
              on:click={toggleLog}
            >
              <X size={12} />
            </button>
          </div>
        </div>
        <pre
          class="text-[10px] leading-[1.35] font-mono p-2 max-h-40 overflow-auto custom-scrollbar whitespace-pre-wrap break-all opacity-90"
          style="color: var(--text, #e5e7eb);">{logText || "(нет данных)"}</pre>
      </div>
    {/if}

    {#if pickerOpen}
      <div
        class="pointer-events-auto mb-2 max-h-[40vh] overflow-y-auto custom-scrollbar rounded-xl border p-3 space-y-2"
        style="border-color: var(--border); background: color-mix(in srgb, var(--card, #14181c) 92%, transparent);"
      >
        <div class="flex items-center justify-between gap-2">
          <p class="text-xs font-bold opacity-90">Добавить панель</p>
          <button
            type="button"
            class="text-[10px] text-jm-accent-light hover:underline"
            on:click={resetLayout}>Сброс к 6 по умолчанию</button
          >
        </div>
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2">
          {#each WIDGET_CATALOG as c (c.kind)}
            {@const on = layout.widgets.some((w) => w.kind === c.kind)}
            <button
              type="button"
              disabled={on}
              class="text-left rounded-lg border px-2.5 py-2 text-[11px] transition-colors disabled:opacity-40 disabled:pointer-events-none hover:bg-white/5"
              style="border-color: var(--border, rgba(255,255,255,0.1));"
              on:click={() => addWidget(c.kind)}
            >
              <span class="font-semibold block">{c.title}</span>
              <span class="opacity-60 text-[10px] leading-snug">{c.blurb}</span>
              {#if on}<span class="text-[9px] text-jm-accent">уже есть</span>{/if}
            </button>
          {/each}
        </div>
      </div>
    {/if}

    <div
      class="pointer-events-auto flex-1 min-h-0 overflow-y-auto custom-scrollbar pr-1 grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-2 sm:gap-3 content-start"
    >
      {#each layout.widgets as w, i (w.id)}
        {@const meta = catalogMeta(w.kind)}
        <div
          class="rounded-xl border shadow-lg backdrop-blur-md flex flex-col min-h-0 max-h-[min(52vh,28rem)] transition-transform"
          class:opacity-50={dragIndex === i}
          class:ring-2={dragOverIndex === i && dragIndex !== i}
          style="background: color-mix(in srgb, var(--card, rgba(20,24,28,0.88)) 88%, transparent); border-color: var(--border, rgba(255,255,255,0.12)); box-shadow: 0 12px 40px rgba(0,0,0,0.35);"
          role="listitem"
          on:dragover={(e) => onDragOver(i, e)}
          on:drop={(e) => onDrop(i, e)}
        >
          <div
            class="flex items-start justify-between gap-1 px-3 py-2 border-b shrink-0"
            style="border-color: var(--border, rgba(255,255,255,0.08));"
          >
            <div class="flex items-center gap-1.5 min-w-0">
              <button
                type="button"
                class="shrink-0 p-0.5 rounded cursor-grab active:cursor-grabbing text-white/40 hover:text-white/80 hover:bg-white/5"
                title="Перетащить для изменения порядка"
                aria-label="Перетащить панель"
                draggable="true"
                on:dragstart={(e) => onDragStart(i, e)}
                on:dragend={onDragEnd}
              >
                <GripVertical size={12} />
              </button>
              <p class="text-[11px] font-bold truncate opacity-95">{meta?.title ?? w.kind}</p>
            </div>
            <button
              type="button"
              class="shrink-0 p-1 rounded-lg hover:bg-red-500/20 text-red-300/90"
              title="Убрать панель"
              aria-label="Убрать"
              on:click={() => removeWidget(w.id)}
            >
              <X size={14} />
            </button>
          </div>
          <div class="p-3 text-xs min-h-0 overflow-y-auto custom-scrollbar flex-1 space-y-2">
            {#if w.kind === "game_session"}
              <div class="flex items-center gap-2">
                <Gamepad2 size={16} style="color: var(--accent);" />
                {#if gameRunning}
                  <span class="font-semibold text-jm-accent-light">Игра запущена из лаунчера</span>
                {:else}
                  <span class="text-amber-200/90">Нет активной сессии</span>
                {/if}
              </div>
            {:else if w.kind === "game_stats"}
              {#if stats}
                <div class="grid grid-cols-2 gap-2 text-[11px]">
                  <div class="flex items-center gap-1 opacity-90">
                    <Layers size={14} class="opacity-70 shrink-0" />
                    Сборок: <b>{stats.sessions}</b>
                  </div>
                  <div class="flex items-center gap-1 opacity-90">
                    <Cpu size={14} class="opacity-70 shrink-0" />
                    CPU Σ: <b>{stats.cpu_percent_total.toFixed(1)}%</b>
                  </div>
                  <div class="flex items-center gap-1 opacity-90 col-span-2">
                    <HardDrive size={14} class="opacity-70 shrink-0" />
                    RAM: <b>{stats.memory_used_mb.toFixed(0)}</b> МБ
                  </div>
                </div>
                {#if stats.instance_ids.length}
                  <p class="text-[10px] opacity-55 font-mono break-all">{stats.instance_ids.join(", ")}</p>
                {/if}
              {:else}
                <p class="opacity-60">Нет данных</p>
              {/if}
            {:else if w.kind === "clock_date"}
              {#key clockTick}
                <p class="text-lg font-mono font-bold tracking-tight">
                  {new Date().toLocaleTimeString(undefined, {
                    hour: "2-digit",
                    minute: "2-digit",
                    second: "2-digit",
                  })}
                </p>
                <p class="text-[11px] opacity-75">
                  {new Date().toLocaleDateString(undefined, {
                    weekday: "long",
                    year: "numeric",
                    month: "long",
                    day: "numeric",
                  })}
                </p>
              {/key}
            {:else if w.kind === "tips_rotation"}
              <p class="text-[11px] leading-relaxed opacity-90">{tipText}</p>
            {:else if w.kind === "overlay_hotkey"}
              <p class="text-[11px] leading-relaxed opacity-85">
                Закрой оверлей кликом по фону, крестиком или <b>той же горячей клавишей</b>, что в
                расширенных настройках (по умолчанию Alt+` ).
              </p>
            {:else if w.kind === "chat_feed"}
              <OverlayChatFeed apiBase={apiBase} chatTabEnabled={chatTabEnabled} />
            {:else if w.kind === "launcher_meta"}
              <p class="font-mono text-sm text-jm-accent-light">{LAUNCHER_VERSION}</p>
              <p class="text-[10px] opacity-60">Сборка лаунчера JentleMemes</p>
            {:else if w.kind === "api_ping"}
              <p class="text-sm font-mono">
                {#if pingMs != null}{pingMs} мс{:else}—{/if}
              </p>
              <p class="text-[10px] opacity-60 truncate">{apiBase}</p>
            {:else if w.kind === "friends_online"}
              <p class="text-2xl font-bold text-jm-accent-light">{friendsCount}</p>
              <p class="text-[10px] opacity-65">Записей в списке друзей (API)</p>
            {:else if w.kind === "conversations_count"}
              <p class="text-2xl font-bold text-jm-accent-light">{convCount}</p>
              <p class="text-[10px] opacity-65">Бесед в аккаунте</p>
            {:else if w.kind === "screen_info"}
              <p class="font-mono text-[11px]">{window.innerWidth}×{window.innerHeight}</p>
              <p class="text-[10px] opacity-60">Логические px окна оверлея</p>
            {:else if w.kind === "memory_bar"}
              {#if stats}
                {@const pct = Math.min(100, (stats.memory_used_mb / 8192) * 100)}
                <div class="h-2 rounded-full bg-white/10 overflow-hidden">
                  <div
                    class="h-full rounded-full transition-all duration-300"
                    style="width: {pct}%; background: var(--jm-accent, #86a886);"
                  ></div>
                </div>
                <p class="text-[10px] opacity-70">~{stats.memory_used_mb.toFixed(0)} МБ к условным 8 ГБ</p>
              {:else}
                <p class="opacity-60">Нет данных</p>
              {/if}
            {:else if w.kind === "session_pids"}
              {#if stats?.pids?.length}
                <p class="font-mono text-[10px] break-all">{stats.pids.join(", ")}</p>
              {:else}
                <p class="opacity-60">Нет PID</p>
              {/if}
            {:else if w.kind === "rect_source_detail"}
              <p class="text-[11px]">{srcLabel(rectSource)}</p>
              <p class="text-[10px] opacity-60 font-mono">{rectSource || "—"}</p>
            {:else if w.kind === "keyboard_hints"}
              <ul class="text-[10px] space-y-1 opacity-90 list-disc pl-4">
                <li>F3 + Q — список отладочных комбинаций</li>
                <li>F3 + B — хитбоксы сущностей</li>
                <li>Двойной W — бег (если включено)</li>
              </ul>
            {:else if w.kind === "linux_gl_tip"}
              <p class="text-[10px] leading-relaxed opacity-85">
                Если Forge пишет GLXBadFBConfig — отключи «дискретную GPU» в сборке без NVIDIA или обнови Mesa.
                Лаунчер снимает лишние PRIME-переменные, если галочка выключена.
              </p>
            {:else if w.kind === "random_mc_fact"}
              <p class="text-[11px] leading-relaxed opacity-90">{factText}</p>
            {:else if w.kind === "poll_meta"}
              <p class="text-[11px] opacity-85">
                Рамка и статистика обновляются примерно каждые <b>400 мс</b>. Соц. API — реже.
              </p>
            {:else if w.kind === "instance_list"}
              {#if instancesShort.length}
                <ul class="text-[10px] space-y-1 opacity-90">
                  {#each instancesShort as ins (ins.id)}
                    <li class="truncate">
                      <span class="font-mono opacity-60">{ins.id.slice(0, 8)}…</span>
                      {ins.name || "без имени"}
                    </li>
                  {/each}
                </ul>
              {:else}
                <p class="opacity-60">Нет сборок</p>
              {/if}
            {:else if w.kind === "jvm_ram_tip"}
              <p class="text-[10px] leading-relaxed opacity-85">
                Выставь -Xmx в настройках лаунчера с запасом ~1–2 ГБ до лимита ОЗУ. Слишком большой хип
                может лагать из-за GC.
              </p>
            {:else if w.kind === "network_status"}
              <p class="text-sm font-semibold">
                {typeof navigator !== "undefined" && navigator.onLine ? "Онлайн" : "Офлайн"}
              </p>
              <p class="text-[10px] opacity-60">navigator.onLine</p>
            {:else if w.kind === "clipboard_hint"}
              <p class="text-[10px] leading-relaxed opacity-85">
                В игре: F3 + C копирует данные мира в буфер (зависит от версии). В чате лаунчера —
                обычное выделение и Ctrl+C.
              </p>
            {:else if w.kind === "modrinth_tip"}
              <p class="text-[10px] leading-relaxed opacity-85">
                Моды для Fabric/NeoForge ищи на Modrinth и CurseForge; проверяй версию Minecraft и загрузчика.
              </p>
            {:else if w.kind === "shader_tip"}
              <p class="text-[10px] leading-relaxed opacity-85">
                Шейдеры грузят GPU сильнее ванильного MC. При просадках FPS начни с отключения шейдер-пака.
              </p>
            {:else if w.kind === "coordinates_joke"}
              <p class="text-[11px] font-mono opacity-90">X: ??? · Y: стой · Z: подумай</p>
              <p class="text-[10px] opacity-55">Реальные координаты — только в игре (F3)</p>
            {:else if w.kind === "water_reminder"}
              <p class="text-[11px] opacity-90">Не забудь воду, если долгий забег в шахту.</p>
            {:else if w.kind === "breathing_hint"}
              <p class="text-[11px] leading-relaxed opacity-85">
                20–20–20: каждые 20 мин смотри 20 секунд на что-то в 6+ метрах — разгрузка для глаз.
              </p>
            {:else if w.kind === "seed_idea"}
              <p class="text-[11px] leading-relaxed opacity-90">{seedText}</p>
            {:else}
              <p class="opacity-60">{w.kind}</p>
            {/if}
          </div>
        </div>
      {/each}

      {#if layout.widgets.length === 0}
        <button
          type="button"
          class="col-span-full flex items-center justify-center gap-2 py-8 rounded-xl border border-dashed opacity-70 hover:opacity-100"
          on:click={() => (pickerOpen = true)}
        >
          <Plus size={20} />
          Добавьте панели через меню «Панели»
        </button>
      {/if}
    </div>
  </div>

  {#if toastMsg}
    <div
      class="absolute bottom-4 left-1/2 -translate-x-1/2 z-20 pointer-events-none px-3 py-2 rounded-xl text-[11px] font-semibold shadow-lg backdrop-blur-md border"
      style="background: color-mix(in srgb, var(--card, #0f1115) 92%, transparent); border-color: var(--border, rgba(255,255,255,0.18)); color: var(--text, #e5e7eb);"
    >
      {toastMsg}
    </div>
  {/if}
</div>
