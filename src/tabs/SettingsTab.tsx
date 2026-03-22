import { useState, useEffect, useCallback, useRef } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { Save, Cpu, Terminal, FolderOpen, Layers, FlaskConical, Palette, Image, Check, RefreshCw, Download, X, Eye, Wand2, SlidersHorizontal, Trash2, Pencil } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { showToast, applyTheme } from "../App";
import { builtinThemes, type ThemeDef, type CustomThemeDef, type ThemeColors, generatePaletteFromAccent, applyThemeColors, loadCustomThemes, saveCustomThemes } from "../themes";
import { AnimatedSection } from "../components/AnimatedSection";
import { LAUNCHER_VERSION } from "../version";

export default function SettingsTab() {
  const [settings, setSettings] = useState<Record<string, any>>({
    ram_mb: 4096,
    jvm_args: "-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions",
    wrapper: "",
    close_on_launch: false,
    custom_java_path: "",
    show_news: true,
    download_dependencies: true,
    hybrid_provider_enabled: false,
    mod_provider: "modrinth",
    curseforge_api_key: "",
    theme: "jentle-dark",
    background: "",
    discord_rich_presence: true,
  });

  const [maxRam, setMaxRam] = useState(8192);
  const [backgrounds, setBackgrounds] = useState<string[]>([]);
  const [updateInfo, setUpdateInfo] = useState<any>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [customThemes, setCustomThemes] = useState<CustomThemeDef[]>([]);
  const [editorOpen, setEditorOpen] = useState(false);
  const [editingTheme, setEditingTheme] = useState<CustomThemeDef | null>(null);

  useEffect(() => {
    invoke("load_settings").then((data: any) => setSettings(data));
    invoke("get_system_ram").then((ram: any) => {
      if (ram && ram > 1024) setMaxRam(ram);
    }).catch(console.error);
    invoke("get_backgrounds").then((bgs: any) => setBackgrounds(bgs || []));
    loadCustomThemes().then(setCustomThemes);
  }, []);

  async function quickSaveThemeBg(theme: string, background: string) {
    try {
      const current: any = await invoke("load_settings");
      current.theme = theme;
      current.background = background;
      await invoke("save_settings", { settings: current });
    } catch {}
  }

  async function handleSave() {
    try {
      await invoke("save_settings", { settings });
      await applyTheme(settings.theme, settings.background);
      showToast("Настройки сохранены!");
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  async function handleCheckUpdate() {
    setCheckingUpdate(true);
    try {
      const upd: any = await invoke("check_launcher_update");
      if (upd?.available) {
        setUpdateInfo(upd);
      } else {
        setUpdateInfo(null);
        showToast("Вы используете последнюю версию!");
      }
    } catch (e) {
      showToast(`Ошибка проверки: ${e}`);
    } finally {
      setCheckingUpdate(false);
    }
  }

  async function handleDownloadUpdate() {
    setDownloading(true);
    showToast("Загрузка обновления...");
    try {
      await invoke("download_and_apply_update");
    } catch (e) {
      showToast(`Ошибка: ${e}`);
      setDownloading(false);
    }
  }

  async function handleAddBackground() {
    try {
      const selected: string | null = await invoke("pick_image_file");
      if (selected) {
        const newPath: string = await invoke("copy_background", { sourcePath: selected });
        setBackgrounds((prev) => [...prev, newPath]);
        setSettings((prev) => ({ ...prev, background: newPath }));
        await applyTheme(settings.theme, newPath);
        quickSaveThemeBg(settings.theme, newPath);
      }
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  function openCreateTheme() {
    setEditingTheme(null);
    setEditorOpen(true);
  }

  function openEditTheme(ct: CustomThemeDef) {
    setEditingTheme(ct);
    setEditorOpen(true);
  }

  async function handleEditorSave(theme: CustomThemeDef) {
    const updated = editingTheme
      ? customThemes.map((t) => (t.id === editingTheme.id ? theme : t))
      : [...customThemes, theme];
    setCustomThemes(updated);
    await saveCustomThemes(updated);
    setSettings((prev) => ({ ...prev, theme: theme.id }));
    await applyTheme(theme.id, settings.background);
    setEditorOpen(false);
    setEditingTheme(null);
  }

  async function handleEditorDelete(id: string) {
    const updated = customThemes.filter((t) => t.id !== id);
    setCustomThemes(updated);
    await saveCustomThemes(updated);
    if (settings.theme === id) {
      setSettings((prev) => ({ ...prev, theme: "jentle-dark" }));
      await applyTheme("jentle-dark", settings.background);
    }
    setEditorOpen(false);
    setEditingTheme(null);
  }

  function handleEditorCancel() {
    applyTheme(settings.theme, settings.background);
    setEditorOpen(false);
    setEditingTheme(null);
  }

  const currentTheme = settings.theme || "jentle-dark";
  const inputClass = "w-full rounded-xl px-4 py-2.5 text-sm border border-[var(--border)] focus:border-jm-accent outline-none transition-colors";

  return (
    <div className="flex flex-col items-center w-full max-w-4xl mx-auto h-full gap-6 pb-10">
      <AnimatedSection delay={0}>
        <div className="flex items-center justify-between w-full">
          <h2 className="text-xl md:text-2xl font-bold text-jm-accent-light mb-1 text-left">
            Настройки лаунчера
          </h2>
          <span className="text-xs text-[var(--text-secondary)] font-mono bg-white/5 px-2.5 py-1 rounded-lg border border-[var(--border)]">{LAUNCHER_VERSION}</span>
        </div>
      </AnimatedSection>

      {/* UPDATE CHECK */}
      <AnimatedSection delay={0.03} className="w-full">
        <div className="w-full bg-jm-card p-5 rounded-2xl border border-[var(--border)] shadow-xl">
          <div className="flex items-center justify-between flex-wrap gap-3">
            <div className="flex items-center gap-3">
              <Download size={18} className="text-jm-accent" />
              <div>
                <h3 className="text-base font-bold">Обновления</h3>
                <p className="text-xs text-[var(--text-secondary)]">
                  {updateInfo
                    ? `Доступна v${updateInfo.latest} (текущая: v${updateInfo.current})`
                    : "Проверьте наличие обновлений"}
                </p>
              </div>
            </div>
            <div className="flex gap-2">
              <motion.button
                whileHover={{ scale: 1.05 }}
                whileTap={{ scale: 0.95 }}
                onClick={handleCheckUpdate}
                disabled={checkingUpdate}
                className="bg-[var(--input-bg)] border border-[var(--border)] hover:border-jm-accent/50 text-sm font-bold px-4 py-2 rounded-xl flex items-center gap-2 disabled:opacity-50"
              >
                <RefreshCw size={14} className={checkingUpdate ? "animate-spin" : ""} />
                Проверить
              </motion.button>
              {updateInfo && (
                <motion.button
                  initial={{ opacity: 0, scale: 0.8 }}
                  animate={{ opacity: 1, scale: 1 }}
                  whileHover={{ scale: 1.05 }}
                  whileTap={{ scale: 0.95 }}
                  onClick={handleDownloadUpdate}
                  disabled={downloading}
                  className="bg-jm-accent hover:bg-jm-accent-light text-black font-bold px-4 py-2 rounded-xl text-sm flex items-center gap-2 disabled:opacity-50"
                >
                  <Download size={14} className={downloading ? "animate-bounce" : ""} />
                  {downloading ? "Загрузка..." : "Обновить"}
                </motion.button>
              )}
            </div>
          </div>
          {updateInfo?.changelog && (
            <motion.div
              initial={{ height: 0, opacity: 0 }}
              animate={{ height: "auto", opacity: 1 }}
              className="mt-3 p-3 rounded-lg border border-[var(--border)] text-xs whitespace-pre-wrap overflow-hidden"
              style={{ background: "var(--input-bg)", color: "var(--text-secondary)" }}
            >
              {updateInfo.changelog}
            </motion.div>
          )}
        </div>
      </AnimatedSection>

      {/* THEME PICKER */}
      <AnimatedSection delay={0.05} className="w-full">
        <div className="w-full bg-jm-card p-5 rounded-2xl border border-[var(--border)] shadow-xl">
          <h3 className="text-base md:text-lg font-bold mb-4 flex items-center gap-2">
            <Palette size={18} className="text-jm-accent" /> Тема оформления
          </h3>

          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
            {builtinThemes.map((t) => (
              <ThemeCard
                key={t.id}
                theme={t}
                isActive={currentTheme === t.id}
                onClick={() => {
                  setSettings((prev) => ({ ...prev, theme: t.id }));
                  applyTheme(t.id, settings.background);
                  quickSaveThemeBg(t.id, settings.background);
                }}
              />
            ))}

            {/* Auto-BG theme */}
            <AutoBgCard
              isActive={currentTheme === "auto-bg"}
              disabled={!settings.background}
              onClick={() => {
                if (!settings.background) {
                  showToast("Сначала выберите фоновое изображение");
                  return;
                }
                setSettings((prev) => ({ ...prev, theme: "auto-bg" }));
                applyTheme("auto-bg", settings.background);
                quickSaveThemeBg("auto-bg", settings.background);
              }}
            />

            {/* Custom themes */}
            {customThemes.map((ct) => (
              <CustomThemeCard
                key={ct.id}
                theme={ct}
                isActive={currentTheme === ct.id}
                onClick={() => {
                  setSettings((prev) => ({ ...prev, theme: ct.id }));
                  applyTheme(ct.id, settings.background);
                  quickSaveThemeBg(ct.id, settings.background);
                }}
                onEdit={() => openEditTheme(ct)}
              />
            ))}

            {/* Create new custom theme */}
            <motion.button
              whileHover={{ scale: 1.04, y: -2 }}
              whileTap={{ scale: 0.96 }}
              onClick={openCreateTheme}
              className="relative p-3 rounded-xl border-2 border-dashed border-[var(--border)] hover:border-jm-accent/40 transition-all overflow-hidden group flex flex-col items-center justify-center gap-2 min-h-[88px]"
              style={{ background: "var(--input-bg)" }}
            >
              <PlusIcon size={24} />
              <p className="text-[10px] font-bold text-[var(--text-secondary)] group-hover:text-jm-accent transition-colors">
                Создать тему
              </p>
            </motion.button>
          </div>
        </div>
      </AnimatedSection>

      {/* BACKGROUND */}
      <AnimatedSection delay={0.1} className="w-full">
        <div className="w-full bg-jm-card p-5 rounded-2xl border border-[var(--border)] shadow-xl">
          <h3 className="text-base md:text-lg font-bold mb-4 flex items-center gap-2">
            <Image size={18} className="text-jm-accent" /> Фоновое изображение
          </h3>

          <div className="flex flex-wrap gap-3">
            <motion.button
              whileHover={{ scale: 1.05 }}
              whileTap={{ scale: 0.95 }}
              onClick={() => {
                setSettings((prev) => ({ ...prev, background: "" }));
                applyTheme(settings.theme, "");
                quickSaveThemeBg(settings.theme, "");
              }}
              className={`w-24 h-16 rounded-xl border-2 flex items-center justify-center text-xs font-bold transition-all ${
                !settings.background
                  ? "border-jm-accent bg-jm-accent/10 text-jm-accent"
                  : "border-[var(--border)] bg-black/20 text-[var(--text-secondary)] hover:border-jm-accent/40"
              }`}
            >
              {!settings.background && <Check size={14} className="mr-1" />}
              Без фона
            </motion.button>

            {backgrounds.map((bgPath, i) => (
              <div key={i} className="relative group">
                <motion.button
                  whileHover={{ scale: 1.05 }}
                  whileTap={{ scale: 0.95 }}
                  onClick={() => {
                    setSettings((prev) => ({ ...prev, background: bgPath }));
                    applyTheme(settings.theme, bgPath);
                    quickSaveThemeBg(settings.theme, bgPath);
                  }}
                  className={`w-24 h-16 rounded-xl border-2 overflow-hidden relative transition-all ${
                    settings.background === bgPath
                      ? "border-jm-accent shadow-lg shadow-jm-accent/20"
                      : "border-[var(--border)] hover:border-jm-accent/40"
                  }`}
                >
                  <img src={convertFileSrc(bgPath)} alt="" className="w-full h-full object-cover" />
                  {settings.background === bgPath && (
                    <div className="absolute inset-0 bg-jm-accent/20 flex items-center justify-center">
                      <Check size={16} className="text-white drop-shadow-lg" />
                    </div>
                  )}
                </motion.button>
                <motion.button
                  whileHover={{ scale: 1.1 }}
                  whileTap={{ scale: 0.9 }}
                  onClick={async (e) => {
                    e.stopPropagation();
                    await invoke("delete_background", { path: bgPath });
                    setBackgrounds((prev) => prev.filter((b) => b !== bgPath));
                    if (settings.background === bgPath) {
                      setSettings((prev) => ({ ...prev, background: "" }));
                      await applyTheme(settings.theme, "");
                      quickSaveThemeBg(settings.theme, "");
                    }
                  }}
                  className="absolute -top-1.5 -right-1.5 w-5 h-5 rounded-full bg-red-500 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity z-10 shadow-md"
                >
                  <X size={10} className="text-white" />
                </motion.button>
              </div>
            ))}

            <motion.button
              whileHover={{ scale: 1.05 }}
              whileTap={{ scale: 0.95 }}
              onClick={handleAddBackground}
              className="w-24 h-16 rounded-xl border-2 border-dashed border-[var(--border)] hover:border-jm-accent/50 flex items-center justify-center text-[var(--text-secondary)] hover:text-jm-accent transition-colors"
            >
              <PlusIcon size={20} />
            </motion.button>
          </div>
        </div>
      </AnimatedSection>

      {/* GAME SETTINGS */}
      <AnimatedSection delay={0.15} className="w-full">
        <div className="w-full bg-jm-card p-5 rounded-2xl border border-[var(--border)] shadow-xl space-y-5">
          <div>
            <h3 className="text-base md:text-lg font-bold mb-1.5 flex items-center gap-2">
              <Cpu size={18} className="text-jm-accent" /> ОЗУ (RAM)
            </h3>
            <p className="text-xs text-[var(--text-secondary)] mb-3">
              Выделено: <strong className="text-jm-accent">{settings.ram_mb} MB</strong>
            </p>
            <input
              type="range"
              min="1024"
              max={maxRam}
              step="512"
              value={settings.ram_mb}
              onChange={(e) => setSettings({ ...settings, ram_mb: parseInt(e.target.value) })}
              className="w-full accent-jm-accent cursor-pointer"
            />
            <div className="flex justify-between text-xs text-[var(--text-secondary)] mt-2 font-bold">
              <span>1 GB</span>
              <span>MAX ({Math.round(maxRam / 1024)} GB)</span>
            </div>
          </div>

          <div>
            <h3 className="text-base md:text-lg font-bold mb-1.5 flex items-center gap-2">
              <FolderOpen size={18} className="text-jm-accent" /> Путь к Java
            </h3>
            <p className="text-xs text-[var(--text-secondary)] mb-2">Оставьте пустым для автопоиска.</p>
            <input
              type="text"
              placeholder="/usr/lib/jvm/java-21/bin/java"
              value={settings.custom_java_path}
              onChange={(e) => setSettings({ ...settings, custom_java_path: e.target.value })}
              className={inputClass}
              style={{ background: "var(--input-bg)" }}
            />
          </div>

          <div>
            <h3 className="text-base md:text-lg font-bold mb-1.5 flex items-center gap-2">
              <Terminal size={18} className="text-jm-accent" /> Аргументы JVM
            </h3>
            <input
              type="text"
              value={settings.jvm_args}
              onChange={(e) => setSettings({ ...settings, jvm_args: e.target.value })}
              className={`${inputClass} font-mono`}
              style={{ background: "var(--input-bg)" }}
            />
          </div>

          <div>
            <h3 className="text-base md:text-lg font-bold mb-1.5 flex items-center gap-2">
              <Layers size={18} className="text-jm-accent" /> Wrapper
            </h3>
            <p className="text-xs text-[var(--text-secondary)] mb-2">mangohud, gamemoderun и т.д.</p>
            <input
              type="text"
              placeholder="mangohud"
              value={settings.wrapper}
              onChange={(e) => setSettings({ ...settings, wrapper: e.target.value })}
              className={`${inputClass} font-mono`}
              style={{ background: "var(--input-bg)" }}
            />
          </div>

          <label className="flex items-center gap-2 cursor-pointer p-3 rounded-lg border border-[var(--border)]" style={{ background: "var(--input-bg)" }}>
            <input
              type="checkbox"
              checked={settings.show_news ?? true}
              onChange={(e) => setSettings({ ...settings, show_news: e.target.checked })}
              className="w-4 h-4 accent-jm-accent cursor-pointer"
            />
            <span className="text-sm font-bold">Показывать новости на главной</span>
          </label>

          <label className="flex items-center gap-2 cursor-pointer p-3 rounded-lg border border-[var(--border)]" style={{ background: "var(--input-bg)" }}>
            <input
              type="checkbox"
              checked={settings.download_dependencies ?? true}
              onChange={(e) => setSettings({ ...settings, download_dependencies: e.target.checked })}
              className="w-4 h-4 accent-jm-accent cursor-pointer"
            />
            <span className="text-sm font-bold">Скачивать зависимости автоматически</span>
          </label>

          <label className="flex items-center gap-2 cursor-pointer p-3 rounded-lg border border-[var(--border)]" style={{ background: "var(--input-bg)" }}>
            <input
              type="checkbox"
              checked={settings.discord_rich_presence ?? true}
              onChange={(e) => setSettings({ ...settings, discord_rich_presence: e.target.checked })}
              className="w-4 h-4 accent-jm-accent cursor-pointer"
            />
            <div className="flex flex-col gap-0.5">
              <span className="text-sm font-bold">Discord Rich Presence</span>
              <span className="text-xs text-[var(--text-secondary)]">Статус в Discord на время игры. Нужен Application ID в исходниках (см. README).</span>
            </div>
          </label>

          <div className="pt-4 border-t border-[var(--border)]">
            <h3 className="text-base md:text-lg font-bold mb-1.5 flex items-center gap-2">
              <FlaskConical size={18} className="text-jm-accent" /> Экспериментальные
            </h3>
            <label className="flex items-center gap-2 cursor-pointer p-3 rounded-lg border border-[var(--border)] mt-2" style={{ background: "var(--input-bg)" }}>
              <input
                type="checkbox"
                checked={!!settings.hybrid_provider_enabled}
                onChange={(e) => setSettings({ ...settings, hybrid_provider_enabled: e.target.checked })}
                className="w-4 h-4 accent-jm-accent cursor-pointer"
              />
              <span className="text-sm font-bold">Гибридный режим (Modrinth + CurseForge)</span>
            </label>
            <p className="text-xs text-[var(--text-secondary)] mt-1.5">
              Объединённый поиск без дубликатов.
            </p>
          </div>

          <div className="pt-4 border-t border-[var(--border)] flex items-center justify-end">
            <motion.button
              whileHover={{ scale: 1.05 }}
              whileTap={{ scale: 0.95 }}
              onClick={handleSave}
              className="bg-jm-accent hover:bg-jm-accent-light text-black font-bold px-6 py-2.5 rounded-xl text-sm flex items-center gap-2 shadow-lg"
            >
              <Save size={16} /> Сохранить
            </motion.button>
          </div>
        </div>
      </AnimatedSection>

      {/* THEME EDITOR MODAL */}
      <AnimatePresence>
        {editorOpen && (
          <ThemeEditorModal
            initial={editingTheme}
            onSave={handleEditorSave}
            onDelete={editingTheme ? () => handleEditorDelete(editingTheme.id) : undefined}
            onCancel={handleEditorCancel}
          />
        )}
      </AnimatePresence>
    </div>
  );
}

/* ═══ Sub-components ═══ */

function PlusIcon({ size }: { size: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <line x1="12" y1="5" x2="12" y2="19" />
      <line x1="5" y1="12" x2="19" y2="12" />
    </svg>
  );
}

function ThemeCard({ theme, isActive, onClick }: { theme: ThemeDef; isActive: boolean; onClick: () => void }) {
  return (
    <motion.button
      whileHover={{ scale: 1.04, y: -2 }}
      whileTap={{ scale: 0.96 }}
      onClick={onClick}
      className={`relative p-3 rounded-xl border-2 transition-all overflow-hidden group ${
        isActive ? "border-jm-accent shadow-lg shadow-jm-accent/20" : "border-[var(--border)] hover:border-jm-accent/30"
      }`}
      style={{ background: theme.preview.bg }}
    >
      <div className="flex flex-col gap-1.5">
        <div className="flex gap-1.5">
          <div className="w-full h-2 rounded-full" style={{ background: theme.preview.accent }} />
        </div>
        <div className="flex gap-1">
          <div className="flex-1 h-6 rounded-md" style={{ background: theme.preview.card }} />
          <div className="flex-1 h-6 rounded-md" style={{ background: theme.preview.card }} />
        </div>
        <div className="h-3 rounded-md opacity-40" style={{ background: theme.preview.card }} />
      </div>
      <p
        className="text-[10px] font-bold mt-2 text-center truncate"
        style={{ color: theme.isLight ? (theme.preview.bg === "#f8f5ee" ? "#2a2518" : "#333") : "#ddd" }}
      >
        {theme.name}
      </p>
      <AnimatePresence>
        {isActive && (
          <motion.div
            initial={{ scale: 0 }}
            animate={{ scale: 1 }}
            exit={{ scale: 0 }}
            className="absolute top-1.5 right-1.5 w-5 h-5 rounded-full flex items-center justify-center"
            style={{ background: theme.preview.accent }}
          >
            <Check size={12} className="text-white" strokeWidth={3} />
          </motion.div>
        )}
      </AnimatePresence>
    </motion.button>
  );
}

function AutoBgCard({ isActive, disabled, onClick }: { isActive: boolean; disabled: boolean; onClick: () => void }) {
  return (
    <motion.button
      whileHover={disabled ? {} : { scale: 1.04, y: -2 }}
      whileTap={disabled ? {} : { scale: 0.96 }}
      onClick={onClick}
      className={`relative p-3 rounded-xl border-2 transition-all overflow-hidden group min-h-[88px] ${
        isActive ? "border-jm-accent shadow-lg shadow-jm-accent/20" : "border-[var(--border)] hover:border-jm-accent/30"
      } ${disabled ? "opacity-40 cursor-not-allowed" : ""}`}
      style={{
        background: "linear-gradient(135deg, #ff6b6b 0%, #feca57 25%, #48dbfb 50%, #ff9ff3 75%, #54a0ff 100%)",
      }}
    >
      <div className="flex flex-col items-center justify-center h-full gap-1.5">
        <Wand2 size={20} className="text-white drop-shadow-md" />
        <p className="text-[10px] font-bold text-white drop-shadow-md text-center">Авто (по фону)</p>
      </div>
      <AnimatePresence>
        {isActive && (
          <motion.div
            initial={{ scale: 0 }}
            animate={{ scale: 1 }}
            exit={{ scale: 0 }}
            className="absolute top-1.5 right-1.5 w-5 h-5 rounded-full bg-white flex items-center justify-center"
          >
            <Check size={12} className="text-black" strokeWidth={3} />
          </motion.div>
        )}
      </AnimatePresence>
    </motion.button>
  );
}

function CustomThemeCard({ theme, isActive, onClick, onEdit }: { theme: CustomThemeDef; isActive: boolean; onClick: () => void; onEdit: () => void }) {
  return (
    <motion.button
      whileHover={{ scale: 1.04, y: -2 }}
      whileTap={{ scale: 0.96 }}
      onClick={onClick}
      className={`relative p-3 rounded-xl border-2 transition-all overflow-hidden group ${
        isActive ? "border-jm-accent shadow-lg shadow-jm-accent/20" : "border-[var(--border)] hover:border-jm-accent/30"
      }`}
      style={{ background: theme.colors.bg }}
    >
      <div className="flex flex-col gap-1.5">
        <div className="w-full h-2 rounded-full" style={{ background: theme.colors.accent }} />
        <div className="flex gap-1">
          <div className="flex-1 h-6 rounded-md" style={{ background: theme.colors.card }} />
          <div className="flex-1 h-6 rounded-md" style={{ background: theme.colors.card }} />
        </div>
        <div className="h-3 rounded-md opacity-40" style={{ background: theme.colors.card }} />
      </div>
      <p className="text-[10px] font-bold mt-2 text-center truncate" style={{ color: theme.isLight ? "#333" : "#ddd" }}>
        {theme.name}
      </p>
      <AnimatePresence>
        {isActive && (
          <motion.div
            initial={{ scale: 0 }}
            animate={{ scale: 1 }}
            exit={{ scale: 0 }}
            className="absolute top-1.5 right-1.5 w-5 h-5 rounded-full flex items-center justify-center"
            style={{ background: theme.colors.accent }}
          >
            <Check size={12} className="text-white" strokeWidth={3} />
          </motion.div>
        )}
      </AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        whileHover={{ opacity: 1 }}
        onClick={(e) => { e.stopPropagation(); onEdit(); }}
        className="absolute top-1.5 left-1.5 w-6 h-6 rounded-full bg-black/60 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
      >
        <Pencil size={10} className="text-white" />
      </motion.div>
    </motion.button>
  );
}

/* ═══ Theme Editor Modal ═══ */

interface EditorProps {
  initial: CustomThemeDef | null;
  onSave: (theme: CustomThemeDef) => void;
  onDelete?: () => void;
  onCancel: () => void;
}

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

function ThemeEditorModal({ initial, onSave, onDelete, onCancel }: EditorProps) {
  const [name, setName] = useState(initial?.name || "");
  const [isLight, setIsLight] = useState(initial?.isLight ?? false);
  const [accentColor, setAccentColor] = useState(initial?.colors.accent || "#7c3aed");
  const [advanced, setAdvanced] = useState(false);
  const [colors, setColors] = useState<ThemeColors>(
    initial?.colors || generatePaletteFromAccent("#7c3aed", false)
  );
  const prevThemeRef = useRef<string>("");

  useEffect(() => {
    prevThemeRef.current = document.documentElement.className;
  }, []);

  const updateFromAccent = useCallback(
    (hex: string, light: boolean) => {
      const palette = generatePaletteFromAccent(hex, light);
      setColors(palette);
      applyThemeColors(palette);
    },
    []
  );

  useEffect(() => {
    if (!advanced) {
      updateFromAccent(accentColor, isLight);
    }
  }, [accentColor, isLight, advanced, updateFromAccent]);

  function handleColorChange(key: keyof ThemeColors, value: string) {
    const next = { ...colors, [key]: value };
    if (key === "accent") {
      const r = parseInt(value.slice(1, 3), 16);
      const g = parseInt(value.slice(3, 5), 16);
      const b = parseInt(value.slice(5, 7), 16);
      next.accentRgb = `${r},${g},${b}`;
    }
    setColors(next);
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

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className="fixed inset-0 z-[9999] flex items-center justify-center p-4"
      onClick={onCancel}
    >
      <div className="absolute inset-0 bg-black/60 backdrop-blur-md" />
      <motion.div
        initial={{ scale: 0.9, y: 30, opacity: 0 }}
        animate={{ scale: 1, y: 0, opacity: 1 }}
        exit={{ scale: 0.9, y: 30, opacity: 0 }}
        transition={{ type: "spring", stiffness: 300, damping: 25 }}
        onClick={(e) => e.stopPropagation()}
        className="relative w-full max-w-2xl max-h-[85vh] overflow-y-auto custom-scrollbar rounded-2xl border border-[var(--border)] shadow-2xl"
        style={{ background: "var(--input-bg)" }}
      >
        {/* Header */}
        <div className="sticky top-0 z-10 flex items-center justify-between p-5 pb-3 border-b border-[var(--border)]" style={{ background: "var(--input-bg)" }}>
          <h2 className="text-lg font-bold flex items-center gap-2">
            <Palette size={20} className="text-jm-accent" />
            {initial ? "Редактировать тему" : "Создать тему"}
          </h2>
          <motion.button whileHover={{ scale: 1.1 }} whileTap={{ scale: 0.9 }} onClick={onCancel} className="p-1.5 rounded-lg hover:bg-white/10">
            <X size={18} />
          </motion.button>
        </div>

        <div className="p-5 space-y-5">
          {/* Name */}
          <div>
            <label className="text-xs font-bold text-[var(--text-secondary)] mb-1.5 block">Название</label>
            <input
              type="text"
              placeholder="Моя тема"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full rounded-xl px-4 py-2.5 text-sm border border-[var(--border)] focus:border-jm-accent outline-none transition-colors"
              style={{ background: "var(--bg)" }}
              autoFocus
            />
          </div>

          {/* Light/Dark toggle */}
          <div>
            <label className="text-xs font-bold text-[var(--text-secondary)] mb-1.5 block">Режим</label>
            <div className="flex gap-2">
              <motion.button
                whileHover={{ scale: 1.03 }}
                whileTap={{ scale: 0.97 }}
                onClick={() => setIsLight(false)}
                className={`flex-1 py-2.5 px-4 rounded-xl text-sm font-bold transition-all border ${
                  !isLight ? "border-jm-accent bg-jm-accent/15 text-jm-accent" : "border-[var(--border)] text-[var(--text-secondary)] hover:border-jm-accent/30"
                }`}
              >
                Тёмная
              </motion.button>
              <motion.button
                whileHover={{ scale: 1.03 }}
                whileTap={{ scale: 0.97 }}
                onClick={() => setIsLight(true)}
                className={`flex-1 py-2.5 px-4 rounded-xl text-sm font-bold transition-all border ${
                  isLight ? "border-jm-accent bg-jm-accent/15 text-jm-accent" : "border-[var(--border)] text-[var(--text-secondary)] hover:border-jm-accent/30"
                }`}
              >
                Светлая
              </motion.button>
            </div>
          </div>

          {/* Accent color (simple mode) */}
          <div>
            <label className="text-xs font-bold text-[var(--text-secondary)] mb-1.5 block">Цвет акцента</label>
            <div className="flex items-center gap-3">
              <div className="relative">
                <input
                  type="color"
                  value={accentColor}
                  onChange={(e) => {
                    setAccentColor(e.target.value);
                    if (advanced) handleColorChange("accent", e.target.value);
                  }}
                  className="w-12 h-12 rounded-xl cursor-pointer border-2 border-[var(--border)]"
                  style={{ padding: 2 }}
                />
              </div>
              <input
                type="text"
                value={accentColor}
                onChange={(e) => {
                  const v = e.target.value;
                  if (/^#[0-9a-fA-F]{6}$/.test(v)) {
                    setAccentColor(v);
                    if (advanced) handleColorChange("accent", v);
                  }
                  if (v.length <= 7) setAccentColor(v);
                }}
                className="w-28 rounded-lg px-3 py-2 text-sm font-mono border border-[var(--border)] focus:border-jm-accent outline-none"
                style={{ background: "var(--bg)" }}
              />
              <div className="flex-1 h-10 rounded-lg" style={{ background: `linear-gradient(135deg, ${accentColor}, ${colors.accentLight})` }} />
            </div>
          </div>

          {/* Live preview */}
          <div>
            <label className="text-xs font-bold text-[var(--text-secondary)] mb-1.5 flex items-center gap-1.5">
              <Eye size={12} /> Предпросмотр
            </label>
            <div className="rounded-xl overflow-hidden border border-[var(--border)]" style={{ background: colors.bg }}>
              <div className="px-4 py-2 flex items-center gap-3" style={{ background: colors.headerBg, borderBottom: `1px solid ${colors.border}` }}>
                <div className="w-4 h-4 rounded-full" style={{ background: colors.accent }} />
                <div className="h-2 w-20 rounded-full" style={{ background: colors.accent, opacity: 0.6 }} />
                <div className="flex-1" />
                <div className="h-2 w-8 rounded-full" style={{ background: colors.textSecondary, opacity: 0.4 }} />
              </div>
              <div className="p-4 flex gap-3">
                <div className="flex-1 rounded-lg p-3" style={{ background: colors.card }}>
                  <div className="h-2 w-16 rounded-full mb-2" style={{ background: colors.text, opacity: 0.8 }} />
                  <div className="h-2 w-24 rounded-full mb-2" style={{ background: colors.textSecondary, opacity: 0.5 }} />
                  <div className="h-6 rounded-md mt-3" style={{ background: colors.accent }} />
                </div>
                <div className="flex-1 rounded-lg p-3" style={{ background: colors.card }}>
                  <div className="h-2 w-12 rounded-full mb-2" style={{ background: colors.text, opacity: 0.8 }} />
                  <div className="h-8 rounded-md mt-2" style={{ background: colors.inputBg, border: `1px solid ${colors.border}` }} />
                  <div className="h-2 w-20 rounded-full mt-2" style={{ background: colors.textSecondary, opacity: 0.4 }} />
                </div>
              </div>
            </div>
          </div>

          {/* Advanced toggle */}
          <motion.button
            whileHover={{ scale: 1.01 }}
            onClick={() => setAdvanced(!advanced)}
            className="w-full flex items-center justify-between p-3 rounded-xl border border-[var(--border)] text-sm font-bold transition-colors hover:border-jm-accent/30"
            style={{ background: "var(--bg)" }}
          >
            <span className="flex items-center gap-2">
              <SlidersHorizontal size={14} className="text-jm-accent" />
              Расширенные настройки
            </span>
            <motion.span animate={{ rotate: advanced ? 180 : 0 }} className="text-[var(--text-secondary)]">
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polyline points="6 9 12 15 18 9" /></svg>
            </motion.span>
          </motion.button>

          <AnimatePresence>
            {advanced && (
              <motion.div
                initial={{ height: 0, opacity: 0 }}
                animate={{ height: "auto", opacity: 1 }}
                exit={{ height: 0, opacity: 0 }}
                className="overflow-hidden"
              >
                <div className="grid grid-cols-2 gap-3 pt-1">
                  {COLOR_FIELDS.map(({ key, label }) => (
                    <div key={key} className="flex items-center gap-2">
                      <input
                        type="color"
                        value={parseColorForInput(colors[key])}
                        onChange={(e) => handleColorChange(key, e.target.value)}
                        className="w-8 h-8 rounded-lg cursor-pointer border border-[var(--border)] shrink-0"
                        style={{ padding: 1 }}
                      />
                      <div className="flex-1 min-w-0">
                        <p className="text-[10px] font-bold text-[var(--text-secondary)] truncate">{label}</p>
                        <p className="text-[10px] font-mono truncate" style={{ color: colors.textSecondary }}>
                          {colors[key].startsWith("#") ? colors[key].slice(0, 7) : colors[key].slice(0, 20)}
                        </p>
                      </div>
                    </div>
                  ))}
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>

        {/* Footer */}
        <div className="sticky bottom-0 flex items-center justify-between p-5 pt-3 border-t border-[var(--border)]" style={{ background: "var(--input-bg)" }}>
          <div>
            {onDelete && (
              <motion.button
                whileHover={{ scale: 1.05 }}
                whileTap={{ scale: 0.95 }}
                onClick={onDelete}
                className="text-red-400 hover:text-red-300 text-sm font-bold flex items-center gap-1.5 px-3 py-2 rounded-xl hover:bg-red-500/10 transition-colors"
              >
                <Trash2 size={14} /> Удалить
              </motion.button>
            )}
          </div>
          <div className="flex gap-2">
            <motion.button
              whileHover={{ scale: 1.05 }}
              whileTap={{ scale: 0.95 }}
              onClick={onCancel}
              className="px-5 py-2.5 rounded-xl text-sm font-bold border border-[var(--border)] hover:border-jm-accent/30 transition-colors"
            >
              Отмена
            </motion.button>
            <motion.button
              whileHover={{ scale: 1.05 }}
              whileTap={{ scale: 0.95 }}
              onClick={handleSave}
              className="bg-jm-accent hover:bg-jm-accent-light text-black font-bold px-6 py-2.5 rounded-xl text-sm flex items-center gap-2 shadow-lg"
            >
              <Save size={14} /> Сохранить
            </motion.button>
          </div>
        </div>
      </motion.div>
    </motion.div>
  );
}
