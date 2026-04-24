/** Панели оверлея: типы, каталог (24+), сохранение в localStorage */

export const OVERLAY_LAYOUT_KEY = "jm_overlay_layout_v2";

export type OverlayWidgetKind =
  | "game_session"
  | "game_stats"
  | "clock_date"
  | "tips_rotation"
  | "overlay_hotkey"
  | "chat_feed"
  | "launcher_meta"
  | "api_ping"
  | "friends_online"
  | "conversations_count"
  | "screen_info"
  | "memory_bar"
  | "session_pids"
  | "rect_source_detail"
  | "keyboard_hints"
  | "linux_gl_tip"
  | "random_mc_fact"
  | "poll_meta"
  | "instance_list"
  | "jvm_ram_tip"
  | "network_status"
  | "clipboard_hint"
  | "modrinth_tip"
  | "shader_tip"
  | "coordinates_joke"
  | "water_reminder"
  | "breathing_hint"
  | "seed_idea";

export type OverlayWidgetInstance = { id: string; kind: OverlayWidgetKind };

export type OverlayLayoutV2 = {
  widgets: OverlayWidgetInstance[];
  /** 0 = почти прозрачно, 1 = сильное затемнение */
  backdropStrength: number;
};

export function randomOverlayWidgetId(): string {
  try {
    return crypto.randomUUID();
  } catch {
    return `w_${Date.now()}_${Math.random().toString(36).slice(2, 9)}`;
  }
}

export const WIDGET_CATALOG: {
  kind: OverlayWidgetKind;
  title: string;
  blurb: string;
}[] = [
  { kind: "game_session", title: "Сессия игры", blurb: "Запущена ли сборка из лаунчера" },
  { kind: "game_stats", title: "CPU / RAM", blurb: "Процессы Minecraft" },
  { kind: "clock_date", title: "Время и дата", blurb: "Локальные часы" },
  { kind: "tips_rotation", title: "Советы", blurb: "Подсказки по игре и лаунчеру" },
  { kind: "overlay_hotkey", title: "Горячая клавиша", blurb: "Как скрыть оверлей" },
  { kind: "chat_feed", title: "Чат", blurb: "Последние сообщения (вкладка «Чат»)" },
  { kind: "launcher_meta", title: "Версия лаунчера", blurb: "Номер сборки" },
  { kind: "api_ping", title: "Пинг API", blurb: "Задержка до сайта" },
  { kind: "friends_online", title: "Друзья", blurb: "Список друзей (кратко)" },
  { kind: "conversations_count", title: "Диалоги", blurb: "Число бесед" },
  { kind: "screen_info", title: "Экран", blurb: "Разрешение окна оверлея" },
  { kind: "memory_bar", title: "RAM полоса", blurb: "Визуализация RAM процессов" },
  { kind: "session_pids", title: "PID", blurb: "Идентификаторы процессов" },
  { kind: "rect_source_detail", title: "Источник рамки", blurb: "Окно игры или монитор" },
  { kind: "keyboard_hints", title: "Клавиши MC", blurb: "Полезные бинды" },
  { kind: "linux_gl_tip", title: "Linux / GLX", blurb: "Подсказка при проблемах с Forge" },
  { kind: "random_mc_fact", title: "Факт", blurb: "Случайный факт о Minecraft" },
  { kind: "poll_meta", title: "Обновление данных", blurb: "Как часто оверлей опрашивает Rust" },
  { kind: "instance_list", title: "Сборки", blurb: "Краткий список из лаунчера" },
  { kind: "jvm_ram_tip", title: "Память Java", blurb: "Совет по -Xmx" },
  { kind: "network_status", title: "Сеть", blurb: "On-line / офлайн навигация" },
  { kind: "clipboard_hint", title: "Буфер", blurb: "Копирование координат и т.д." },
  { kind: "modrinth_tip", title: "Моды", blurb: "Где искать моды" },
  { kind: "shader_tip", title: "Шейдеры", blurb: "Нагрузка на GPU" },
  { kind: "coordinates_joke", title: "Координаты", blurb: "Шутливая панель" },
  { kind: "water_reminder", title: "Вода", blurb: "Напоминание выпить" },
  { kind: "breathing_hint", title: "Пауза", blurb: "Короткий перерыв для глаз" },
  { kind: "seed_idea", title: "Сид", blurb: "Идея для мира" },
];

export const DEFAULT_BACKDROP = 0.18;

export function defaultWidgetKinds(chatEnabled: boolean): OverlayWidgetKind[] {
  return [
    "game_session",
    "game_stats",
    "clock_date",
    "tips_rotation",
    "overlay_hotkey",
    chatEnabled ? "chat_feed" : "launcher_meta",
  ];
}

export function defaultLayout(chatEnabled: boolean): OverlayLayoutV2 {
  return {
    backdropStrength: DEFAULT_BACKDROP,
    widgets: defaultWidgetKinds(chatEnabled).map((kind) => ({ id: randomOverlayWidgetId(), kind })),
  };
}

export function loadOverlayLayout(chatEnabled: boolean): OverlayLayoutV2 {
  try {
    const raw = localStorage.getItem(OVERLAY_LAYOUT_KEY);
    if (!raw) return defaultLayout(chatEnabled);
    const j = JSON.parse(raw) as Partial<OverlayLayoutV2>;
    if (!j.widgets || !Array.isArray(j.widgets) || j.widgets.length === 0) {
      return defaultLayout(chatEnabled);
    }
    const widgets = j.widgets.filter(
      (w): w is OverlayWidgetInstance =>
        w &&
        typeof w.id === "string" &&
        typeof w.kind === "string" &&
        WIDGET_CATALOG.some((c) => c.kind === w.kind),
    );
    if (!widgets.length) return defaultLayout(chatEnabled);
    return {
      widgets,
      backdropStrength:
        typeof j.backdropStrength === "number" && j.backdropStrength >= 0 && j.backdropStrength <= 1
          ? j.backdropStrength
          : DEFAULT_BACKDROP,
    };
  } catch {
    return defaultLayout(chatEnabled);
  }
}

export function saveOverlayLayout(state: OverlayLayoutV2): void {
  try {
    localStorage.setItem(OVERLAY_LAYOUT_KEY, JSON.stringify(state));
  } catch {
    /* ignore */
  }
}

export const OVERLAY_TIPS = [
  "F3 + H — расширенные подсказки по предметам.",
  "F3 + G — границы чанков (1.10+).",
  "Shift + ПКМ по сундуку с ведром — залить/опустошить.",
  "Костёр готовит еду без угля — положи сырые блоки сверху.",
  "Оверлей не заменяет F3: FPS и координаты смотри в игре.",
  "В расширенных настройках можно сменить горячую клавишу оверлея.",
  "Сборки с Forge: при GLXBadFBConfig лаунчер отключает раннее GLFW-окно на Linux.",
  "Папка mods у каждой сборки своя — не путай с глобальной.",
];

export const MC_FACTS = [
  "Изначально камень назывался «Stone» и не имел вариантов.",
  "Крипер родился из ошибки модели свиньи.",
  "Нижний мир в масштабе 1:8 к обычному миру.",
  "Эндермены агрятся, если смотреть им в глаза.",
  "Первый прототип Minecraft назывался Cave Game.",
];

export const SEED_IDEAS = [
  "Попробуй сид из даты рождения без нулей впереди.",
  "Случайный набор букв часто даёт интересный спавн.",
  "speedrun.com — готовые сиды для рекордов.",
];

