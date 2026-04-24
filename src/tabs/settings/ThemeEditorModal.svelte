<script lang="ts">
  import { fade, scale, slide } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import {
    Palette,
    X,
    Eye,
    SlidersHorizontal,
    Trash2,
    Save,
  } from "lucide-svelte";
  import { showToast } from "../../lib/jmEvents";
  import {
    type CustomThemeDef,
    type ThemeColors,
    generatePaletteFromAccent,
    applyThemeColors,
  } from "../../themes";

  export let initial: CustomThemeDef | null = null;
  export let onSave: (theme: CustomThemeDef) => void;
  export let onDelete: (() => void) | undefined = undefined;
  export let onCancel: () => void;

  let name = initial?.name || "";
  let isLight = initial?.isLight ?? false;
  let accentColor = initial?.colors.accent || "#7c3aed";
  let advanced = false;
  let colors: ThemeColors = initial?.colors || generatePaletteFromAccent("#7c3aed", false);
  const COLOR_FIELDS: { key: keyof ThemeColors; label: string }[] = [
    { key: "bg", label: "Фон" },
    { key: "accent", label: "Акцент" },
    { key: "accentLight", label: "Акцент (светлый)" },
    { key: "card", label: "Карточка" },
    { key: "text", label: "Текст" },
    { key: "textSecondary", label: "Текст (доп.)" },
    { key: "inputBg", label: "Поле ввода" },
    { key: "border", label: "Граница" },
  ];

  $: if (!advanced) {
    const palette = generatePaletteFromAccent(accentColor, isLight);
    colors = palette;
    applyThemeColors(palette);
  }

  function handleColorChange(key: keyof ThemeColors, value: string) {
    const next = { ...colors, [key]: value };
    if (key === "accent") {
      const r = parseInt(value.slice(1, 3), 16);
      const g = parseInt(value.slice(3, 5), 16);
      const b = parseInt(value.slice(5, 7), 16);
      next.accentRgb = `${r},${g},${b}`;
    }
    colors = next;
    applyThemeColors(next);
  }

  function handleSave() {
    if (!name.trim()) {
      showToast("Введите название темы");
      return;
    }
    const id = initial?.id || `custom-${Date.now()}`;
    onSave({ id, name: name.trim(), isLight, colors });
  }

  function parseColorForInput(val: string): string {
    if (val.startsWith("#") && (val.length === 7 || val.length === 4)) return val;
    if (val.startsWith("#") && val.length === 9) return val.slice(0, 7);
    return "#888888";
  }

  function onAccentColorInput(v: string) {
    accentColor = v;
    if (advanced) handleColorChange("accent", v);
  }

  function onAccentTextInput(v: string) {
    if (/^#[0-9a-fA-F]{6}$/.test(v)) {
      accentColor = v;
      if (advanced) handleColorChange("accent", v);
    }
    if (v.length <= 7) accentColor = v;
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-[9999] flex items-center justify-center p-4"
  transition:fade={{ duration: 200 }}
  on:click={onCancel}
  role="presentation"
>
  <div class="absolute inset-0 bg-black/60 backdrop-blur-md" />
  <div
    class="relative w-full max-w-2xl max-h-[85vh] overflow-y-auto custom-scrollbar rounded-2xl border border-[var(--border)] shadow-2xl"
    style:background="var(--input-bg)"
    transition:scale={{ duration: 320, start: 0.94, easing: quintOut }}
    on:click|stopPropagation
    role="dialog"
    aria-modal="true"
  >
    <div
      class="sticky top-0 z-10 flex items-center justify-between p-5 pb-3 border-b border-[var(--border)]"
      style:background="var(--input-bg)"
    >
      <h2 class="text-lg font-bold flex items-center gap-2">
        <Palette size={20} class="text-jm-accent" />
        {initial ? "Редактировать тему" : "Создать тему"}
      </h2>
      <button
        type="button"
        on:click={onCancel}
        class="p-1.5 rounded-lg hover:bg-white/10 jm-tap-scale"
      >
        <X size={18} />
      </button>
    </div>

    <div class="p-5 space-y-5">
      <div>
        <label class="text-xs font-bold text-[var(--text-secondary)] mb-1.5 block" for="jm-theme-name"
          >Название</label
        >
        <input
          id="jm-theme-name"
          type="text"
          placeholder="Моя тема"
          bind:value={name}
          class="w-full rounded-xl px-4 py-2.5 text-sm border border-[var(--border)] focus:border-jm-accent outline-none transition-colors"
          style:background="var(--bg)"
        />
      </div>

      <div>
        <label class="text-xs font-bold text-[var(--text-secondary)] mb-1.5 block">Режим</label>
        <div class="flex gap-2">
          <button
            type="button"
            on:click={() => (isLight = false)}
            class="flex-1 py-2.5 px-4 rounded-xl text-sm font-bold transition-all border {!isLight
              ? 'border-jm-accent bg-jm-accent/15 text-jm-accent'
              : 'border-[var(--border)] text-[var(--text-secondary)] hover:border-jm-accent/30'} jm-tap-scale"
          >
            Тёмная
          </button>
          <button
            type="button"
            on:click={() => (isLight = true)}
            class="flex-1 py-2.5 px-4 rounded-xl text-sm font-bold transition-all border {isLight
              ? 'border-jm-accent bg-jm-accent/15 text-jm-accent'
              : 'border-[var(--border)] text-[var(--text-secondary)] hover:border-jm-accent/30'} jm-tap-scale"
          >
            Светлая
          </button>
        </div>
      </div>

      <div>
        <label class="text-xs font-bold text-[var(--text-secondary)] mb-1.5 block">Цвет акцента</label>
        <div class="flex items-center gap-3 flex-wrap">
          <input
            type="color"
            value={accentColor.length >= 7 ? accentColor.slice(0, 7) : accentColor}
            on:input={(e) => onAccentColorInput(e.currentTarget.value)}
            class="w-12 h-12 rounded-xl cursor-pointer border-2 border-[var(--border)]"
            style:padding="2px"
          />
          <input
            type="text"
            value={accentColor}
            on:input={(e) => onAccentTextInput(e.currentTarget.value)}
            class="w-28 rounded-lg px-3 py-2 text-sm font-mono border border-[var(--border)] focus:border-jm-accent outline-none"
            style:background="var(--bg)"
          />
          <div
            class="flex-1 min-w-[120px] h-10 rounded-lg"
            style:background="linear-gradient(135deg, {accentColor}, {colors.accentLight})"
          />
        </div>
      </div>

      <div>
        <label class="text-xs font-bold text-[var(--text-secondary)] mb-1.5 flex items-center gap-1.5">
          <Eye size={12} /> Предпросмотр
        </label>
        <div class="rounded-xl overflow-hidden border border-[var(--border)]" style:background={colors.bg}>
          <div
            class="px-4 py-2 flex items-center gap-3"
            style:border-bottom="1px solid {colors.border}"
            style:background={colors.headerBg}
          >
            <div class="w-4 h-4 rounded-full" style:background={colors.accent} />
            <div class="h-2 w-20 rounded-full" style:background={colors.accent} style:opacity="0.6" />
            <div class="flex-1" />
            <div
              class="h-2 w-8 rounded-full"
              style:background={colors.textSecondary}
              style:opacity="0.4"
            />
          </div>
          <div class="p-4 flex gap-3">
            <div class="flex-1 rounded-lg p-3" style:background={colors.card}>
              <div class="h-2 w-16 rounded-full mb-2" style:background={colors.text} style:opacity="0.8" />
              <div
                class="h-2 w-24 rounded-full mb-2"
                style:background={colors.textSecondary}
                style:opacity="0.5"
              />
              <div class="h-6 rounded-md mt-3" style:background={colors.accent} />
            </div>
            <div class="flex-1 rounded-lg p-3" style:background={colors.card}>
              <div class="h-2 w-12 rounded-full mb-2" style:background={colors.text} style:opacity="0.8" />
              <div
                class="h-8 rounded-md mt-2"
                style:background={colors.inputBg}
                style:border="1px solid {colors.border}"
              />
              <div
                class="h-2 w-20 rounded-full mt-2"
                style:background={colors.textSecondary}
                style:opacity="0.4"
              />
            </div>
          </div>
        </div>
      </div>

      <button
        type="button"
        on:click={() => (advanced = !advanced)}
        class="w-full flex items-center justify-between p-3 rounded-xl border border-[var(--border)] text-sm font-bold transition-colors hover:border-jm-accent/30 jm-tap-scale"
        style:background="var(--bg)"
      >
        <span class="flex items-center gap-2">
          <SlidersHorizontal size={14} class="text-jm-accent" />
          Расширенные настройки
        </span>
        <span class="text-[var(--text-secondary)] transition-transform duration-300" class:rotate-180={advanced}>
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
            ><polyline points="6 9 12 15 18 9" /></svg
          >
        </span>
      </button>

      {#if advanced}
        <div transition:slide={{ duration: 220 }} class="overflow-hidden">
          <p class="text-[10px] text-[var(--text-secondary)] leading-relaxed pb-2">
            Полупрозрачные блоки чата и модалок (bg-black/35, text-white/40 и т.п.) теперь строятся из
            <strong class="text-[var(--text)]">карточки</strong> и
            <strong class="text-[var(--text)]">фона</strong> — настраивайте их выше.
          </p>
          <div class="grid grid-cols-2 gap-3 pt-1">
            {#each COLOR_FIELDS as field (field.key)}
              <div class="flex items-center gap-2">
                <input
                  type="color"
                  value={parseColorForInput(String(colors[field.key]))}
                  on:input={(e) => handleColorChange(field.key, e.currentTarget.value)}
                  class="w-8 h-8 rounded-lg cursor-pointer border border-[var(--border)] shrink-0"
                  style:padding="1px"
                />
                <div class="flex-1 min-w-0">
                  <p class="text-[10px] font-bold text-[var(--text-secondary)] truncate">{field.label}</p>
                  <p class="text-[10px] font-mono truncate" style:color={colors.textSecondary}>
                    {String(colors[field.key]).startsWith("#")
                      ? String(colors[field.key]).slice(0, 7)
                      : String(colors[field.key]).slice(0, 20)}
                  </p>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>

    <div
      class="sticky bottom-0 flex items-center justify-between p-5 pt-3 border-t border-[var(--border)]"
      style:background="var(--input-bg)"
    >
      <div>
        {#if onDelete}
          <button
            type="button"
            on:click={onDelete}
            class="text-red-400 hover:text-red-300 text-sm font-bold flex items-center gap-1.5 px-3 py-2 rounded-xl hover:bg-red-500/10 transition-colors jm-tap-scale"
          >
            <Trash2 size={14} /> Удалить
          </button>
        {/if}
      </div>
      <div class="flex gap-2">
        <button
          type="button"
          on:click={onCancel}
          class="px-5 py-2.5 rounded-xl text-sm font-bold border border-[var(--border)] hover:border-jm-accent/30 transition-colors jm-tap-scale"
        >
          Отмена
        </button>
        <button
          type="button"
          on:click={handleSave}
          class="bg-jm-accent hover:bg-jm-accent-light text-black font-bold px-6 py-2.5 rounded-xl text-sm flex items-center gap-2 shadow-lg jm-tap-scale"
        >
          <Save size={14} /> Сохранить
        </button>
      </div>
    </div>
  </div>
</div>
