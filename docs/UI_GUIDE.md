# UI дизайн-система JentleMemes 2.0

Короткий референс по архитектуре темы и компонентам.

## Три оси кастомизации

Все стили в 2.0 управляются **тремя независимыми атрибутами** на `<html>`:

| Ось | Атрибут | Значения | Назначение |
| --- | --- | --- | --- |
| Визуальный пресет | `data-preset` | `blend`, `modrinth`, `discord`, `legacy`, `glass` | радиусы, плотность, тени, анимации |
| Режим | `data-mode` | `dark`, `light`, `auto` | тёмная/светлая |
| Акцент | CSS-класс `theme-*` | `theme-jentle-dark`, `theme-purple`, `theme-red`, `theme-gold`, `theme-cyan`, `theme-furry-pink`, `theme-custom` | цветовые палитры |

Пресет × Режим × Акцент = **5 × 2 × 7 = 70 штатных сочетаний** (плюс custom
HEX = 105).

## Трёхслойная структура CSS-переменных

```
Слой 0 — глобальные токены (в :root)
  --font-sans, --font-mono
  --text-xs .. --text-3xl
  --dur-fast/base/slow
  --ease-standard/out/emphasized
  --space-1 .. --space-10 (масштабируются через --density)

Слой 1 — структурные токены (в html[data-preset=...])
  --radius-xs/sm/md/lg/xl
  --shadow-sm/md/lg
  --density (0.9..1.1)
  --border-width
  --backdrop-blur

Слой 2 — цветовые токены (в .theme-*)
  --accent, --accent-light, --accent-soft
  --bg, --surface-1, --surface-2, --card
  --text, --text-secondary, --text-muted
  --border
```

## Визуальные пресеты

### `blend` (default)
Гибрид. Радиусы 10px, density 1, тени средние. Мягкий компромисс между
Modrinth и Discord. Это то, что видит пользователь без явного выбора.

### `modrinth`
Большие карточки. `--radius-lg: 14px`, `--density: 1.1`,
`--shadow-md: 0 14px 32px rgba(0,0,0,0.28)`. Много воздуха, крупные шрифты.

### `discord`
Плотный. `--radius-lg: 6px`, `--density: 0.9`, `--space-*` уменьшены.
Строгие тени. Удобно на ноутбучных экранах.

### `legacy`
Реанимация визуала 1.1.0. Специфичные правила — `src/styles/preset-legacy.css`:

- `.jm-titlebar` получает sheen-эффект (shimmering gradient overlay);
- `.sidebar-item--active` получает pulse-glow анимацию;
- карточки с усиленными тенями + внутренней подсветкой;
- фоновые орбы с более сильной амплитудой.

### `glass`
Экспериментальный акрил. `backdrop-filter: blur(16px)`, полупрозрачный фон,
усиленные тени. Требует WebView2 с GPU-ускорением (по умолчанию включено
на Win10+/Linux с современным mesa).

## Акцентные палитры

Все палитры определены в `src/themes.ts`. Для каждой задано 8 тонов в dark и
light, три экспорта (`accent`, `accent-light`, `accent-soft`).

- `jentle-green` — `#86a886` (default).
- `purple` — `#9d6dff`.
- `red` — `#e5484d`.
- `gold` — `#ffb224`.
- `cyan` — `#3b9eff`.
- `furry-pink` — `#ff6ec7`.
- `custom` — HEX-пикер; сохраняется в `settings.accent_custom_hex`.

## Общие компоненты

Каталог `src/components/ui/`:

| Компонент | Назначение |
| --- | --- |
| `Badge.svelte` | цветные метки (`variant: default/success/warning/danger/outline`) |
| `Tooltip.svelte` | всплывающая подсказка на hover/focus |
| `Dialog.svelte` | модальное окно с бэкдропом и `role="dialog"` |
| `ProgressRing.svelte` | круговой прогресс (используется на карточках сборок) |
| `EmptyState.svelte` | заглушка: иконка + заголовок + описание + опциональная кнопка |
| `Skeleton.svelte` | скелетон-лоадер (анимированный градиент) |
| `Tabs.svelte` | горизонтальные табы: pills / underline / segmented |
| `CommandPalette.svelte` | глобальная палитра команд (Ctrl+K) |
| `SectionNav.svelte` | элемент сайдбара |
| `ChromeNavigation.svelte` | хром с сайдбаром/табами/модалками |

## Утилиты Tailwind / классы

- `.ui-card` — стандартная карточка (background + border + radius + shadow).
- `.ui-card-compact` — то же, но меньшие отступы (для плотных пресетов).
- `.ui-btn-primary`, `.ui-btn-secondary`, `.ui-btn-ghost` — кнопки.
- `.jm-titlebar` — titlebar контейнер (включает legacy-специфику).
- `.jm-splash-*` — все классы сплеш-экрана (см. `SplashScreen.svelte`).
- `.jm-onboarding-*` — классы мастера.
- `.jm-reduce-motion` — глобально отключает все CSS-анимации.

## Мотион-токены

```css
:root {
  --dur-fast: 120ms;
  --dur-base: 220ms;
  --dur-slow: 380ms;
  --ease-standard: cubic-bezier(0.4, 0, 0.2, 1);
  --ease-out: cubic-bezier(0.22, 1, 0.36, 1);
  --ease-emphasized: cubic-bezier(0.2, 0.8, 0.2, 1);
}
```

`html.jm-reduce-motion` дополнительно переопределяет всё в `animation: none !important;`.

## Шрифты

- `Inter` через `@fontsource/inter` — основной (Cyrillic + Latin, variable).
- `JetBrains Mono` через `@fontsource/jetbrains-mono` — для логов, кода, хвостов
  файлов.

Размеры:

```
--text-xs:   11px  (подписи в badge, tooltip)
--text-sm:   13px  (подписи к input’ам, заголовки виджетов)
--text-base: 14px  (основной текст)
--text-lg:   16px  (заголовок секции)
--text-xl:   18px  (заголовок карточки)
--text-2xl:  22px  (заголовок модалки)
--text-3xl:  28px  (hero-заголовок)
```

## Команды палитры (реестр)

Регистрация из любого таба:

```ts
import { registerCommands } from "./lib/commandRegistry";

onMount(() => {
  const unregister = registerCommands([
    {
      id: "lib.new-instance",
      title: "Новая сборка",
      group: "Библиотека",
      icon: Plus,
      keywords: ["создать", "new", "instance"],
      run: () => openNewInstanceWizard(),
    },
  ]);
  return () => unregister();
});
```

Правила:

1. `id` должен быть уникальным в пределах всего реестра.
2. `group` — строка, используется для разделителей в палитре.
3. `icon` — компонент `lucide-svelte`.
4. `run` может быть sync или async; возвращаемое значение игнорируется.
5. При размонтировании компонента **обязательно** вернуть `unregister` из
   `onMount` (или вызвать его в `onDestroy`).

## Рекомендации по стилю

- Использовать CSS-переменные вместо хардкоженых цветов. Например, **не**
  `color: #86a886`, а `color: var(--accent)`.
- Хочется прозрачный бэкграунд с оттенком акцента —
  `color-mix(in srgb, var(--accent) 15%, transparent)`.
- В оверлее — всегда `pointer-events: auto` только там, где действительно
  интерактивно; бекдроп должен быть `pointer-events: none`, чтобы не
  перехватывать клики по игре.

## Пример: кастомный пресет

Можно добавить свой пресет без правки всего дерева. Создайте файл
`src/styles/preset-terminal.css`:

```css
html[data-preset="terminal"] {
  --radius-sm: 0;
  --radius-lg: 0;
  --density: 0.85;
  --shadow-md: none;
  --border-width: 1px;
  font-family: "JetBrains Mono", monospace !important;
}
```

Импортируйте его в `src/index.css` сразу после `preset-legacy.css`, обновите
`ALL_VISUAL_PRESETS` в `src/lib/themeApply.ts` и добавьте плитку в
`presetsMeta` в `SettingsTab.svelte`. Готово — ваш пресет доступен наравне со
штатными.

## Полезные ссылки

- Общий гайд: `docs/GUIDE.md`.
- Внутренности запуска: `docs/LAUNCH_INTERNALS.md`.
- Исходник токенов: `src/index.css`.
- Пресет legacy: `src/styles/preset-legacy.css`.
- Применение темы: `src/lib/themeApply.ts`.
