import { useState, useEffect, useRef } from "react";
import { Home, Library, Compass, Settings, Shirt, Loader2, Info, Newspaper } from "lucide-react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { motion, AnimatePresence } from "framer-motion";

import HomeTab from "./tabs/HomeTab";
import LibraryTab from "./tabs/LibraryTab";
import AccountTab from "./tabs/AccountTab";
import SkinsTab from "./tabs/SkinsTab";
import DiscoverTab from "./tabs/DiscoverTab";
import SettingsTab from "./tabs/SettingsTab";
import NewsTab from "./tabs/NewsTab";
import Titlebar from "./Titlebar";

export const showToast = (msg: string) => window.dispatchEvent(new CustomEvent("jm_toast", { detail: msg }));

import { clearInlineThemeColors, applyThemeColors, generatePaletteFromAccent, extractColorsFromImage, loadCustomThemes } from "./themes";

export const applyTheme = async (theme: string, bg: string) => {
  const root = document.documentElement;
  root.className = root.className.replace(/\btheme-\S+/g, "");
  clearInlineThemeColors();

  if (theme === "auto-bg" && bg) {
    const imgSrc = convertFileSrc(bg);
    const { dominant, isLight } = await extractColorsFromImage(imgSrc);
    const palette = generatePaletteFromAccent(dominant, isLight);
    applyThemeColors(palette);
  } else if (theme.startsWith("custom-")) {
    const customs = await loadCustomThemes();
    const ct = customs.find((c) => c.id === theme);
    if (ct) {
      applyThemeColors(ct.colors);
    } else {
      root.classList.add("theme-jentle-dark");
    }
  } else {
    root.classList.add(`theme-${theme || "jentle-dark"}`);
  }

  window.dispatchEvent(new CustomEvent("jm_theme", { detail: { theme, bg } }));
};


const tabs = [
  { id: "home", label: "Главная", icon: Home },
  { id: "news", label: "Новости", icon: Newspaper },
  { id: "library", label: "Сборки", icon: Library },
  { id: "skins", label: "Скины", icon: Shirt },
  { id: "discover", label: "Браузер", icon: Compass },
  { id: "settings", label: "Настройки", icon: Settings },
];

function App() {
  const[activeTab, setActiveTab] = useState("home");
  const[pendingInstanceId, setPendingInstanceId] = useState<string | undefined>(undefined);
  const[pendingServerIp, setPendingServerIp] = useState<string | undefined>(undefined);
  const[pendingWorldName, setPendingWorldName] = useState<string | undefined>(undefined);
  const [activeAccount, setActiveAccount] = useState<any>(null);
  const[activeAvatar, setActiveAvatar] = useState("https://minotar.net/helm/Steve/32.png");

  const [progress, setProgress] = useState<{ task_name: string; downloaded: number; total: number; instance_id?: string }>({ task_name: "", downloaded: 0, total: 0 });
  const [busyInstanceId, setBusyInstanceId] = useState<string | null>(null);
  const [isHoveringDL, setIsHoveringDL] = useState(false);
  const [toasts, setToasts] = useState<{id: number, msg: string}[]>([]);
  const [, setTheme] = useState("jentle-dark");
  const [bgPath, setBgPath] = useState("");
  const [ready, setReady] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<any>(null);

  const navRef = useRef<HTMLDivElement>(null);
  const [indicatorStyle, setIndicatorStyle] = useState({ left: 0, width: 0 });

  useEffect(() => {
    const nav = navRef.current;
    if (!nav) return;
    const activeBtn = nav.querySelector(`[data-tab="${activeTab}"]`) as HTMLElement;
    if (activeBtn) {
      setIndicatorStyle({
        left: activeBtn.offsetLeft,
        width: activeBtn.offsetWidth,
      });
    }
  }, [activeTab]);

  useEffect(() => {
    const handler = (e: Event) => {
      const { theme: t, bg } = (e as CustomEvent).detail;
      const next = t || "jentle-dark";
      setTheme(next);
      setBgPath(bg || "");
    };
    window.addEventListener("jm_theme", handler);
    return () => window.removeEventListener("jm_theme", handler);
  }, []);

  async function loadSettings() {
    try {
      const s: any = await invoke("load_settings");
      const t = s.theme || "jentle-dark";
      setTheme(t);
      setBgPath(s.background || "");
      await applyTheme(t, s.background || "");
    } catch(e){}
    try {
      const upd: any = await invoke("check_launcher_update");
      if (upd?.available) setUpdateInfo(upd);
    } catch(e){}
  }

  async function loadActiveAccount() {
    try {
      const data: any = await invoke("load_profiles");
      const active = data.accounts.find((a: any) => a.id === data.active_account_id);
      setActiveAccount(active || null);
      if (active) {
        let avatarUrl = `https://minotar.net/helm/${active.username}/32.png`;
        if (active.active_skin_id) {
          const skin = data.skin_presets.find((p: any) => p.id === active.active_skin_id);
          if (skin) avatarUrl = skin.skin_type === "local" ? skin.skin_data : `https://minotar.net/helm/${skin.skin_data || skin.username}/32.png`;
        }
        setActiveAvatar(avatarUrl);
      } else setActiveAvatar("https://minotar.net/helm/Steve/32.png");
    } catch (e) {}
  }

  useEffect(() => {
    loadSettings();
    loadActiveAccount();
    setTimeout(() => setReady(true), 100);
    const unlistenProf = listen("profiles_updated", () => loadActiveAccount());
    const unlistenSettings = listen("settings_updated", () => loadSettings());
    const unlistenProg = listen<any>("download_progress", (e) => {
      const p = e.payload;
      setProgress(p);
      if (p.instance_id) setBusyInstanceId(p.instance_id);
      if (p.total > 0 && p.downloaded >= p.total) setTimeout(() => setBusyInstanceId(null), 500);
    });

    const handleToast = (e: any) => {
      const id = Date.now();
      setToasts(prev =>[...prev, { id, msg: e.detail }]);
      setTimeout(() => setToasts(prev => prev.filter(t => t.id !== id)), 3000);
    };
    window.addEventListener("jm_toast", handleToast);

    return () => { unlistenProf.then(f => f()); unlistenSettings.then(f=>f()); unlistenProg.then(f => f()); window.removeEventListener("jm_toast", handleToast); };
  },[]);

  const percent = progress.total > 0 ? Math.round((progress.downloaded / progress.total) * 100) : 0;
  const showDownload = progress.total > 0 && progress.downloaded < progress.total;

  return (
    <div className="flex flex-col h-screen bg-jm-bg overflow-hidden font-sans rounded-lg border border-white/5 shadow-2xl relative" style={{ color: "var(--text)" }}>
      <Titlebar />

      {/* Background image */}
      {bgPath && (
        <div className="absolute inset-0 z-0">
          <img
            src={convertFileSrc(bgPath)}
            alt=""
            className="w-full h-full object-cover"
          />
          <div className="absolute inset-0 bg-jm-bg/70 backdrop-blur-sm" />
        </div>
      )}

      {/* Ambient background effects */}
      <div className="absolute inset-0 pointer-events-none overflow-hidden z-0">
        <div className="absolute -top-40 -left-40 w-[500px] h-[500px] bg-jm-accent/[0.03] rounded-full blur-[120px] spin-slow" />
        <div className="absolute -bottom-60 -right-40 w-[600px] h-[600px] bg-jm-accent/[0.02] rounded-full blur-[150px] spin-slow" style={{ animationDirection: "reverse", animationDuration: "30s" }} />
      </div>

      {/* Header */}
      <motion.header
        initial={{ y: -40, opacity: 0 }}
        animate={{ y: 0, opacity: 1 }}
        transition={{ duration: 0.5, ease: [0.22, 1, 0.36, 1] }}
        className="flex items-center justify-between px-3 md:px-6 py-2 glass border-b border-[var(--border)] shadow-lg z-40 shrink-0 gap-2 min-h-0"
      >
        <motion.div
          initial={{ opacity: 0, x: -20 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ delay: 0.2, duration: 0.4 }}
          className="text-lg md:text-xl font-bold text-jm-accent-light tracking-wide shrink-0 hidden sm:block"
        >
          JentleMemes
        </motion.div>

        {/* Animated navigation */}
        <nav ref={navRef} className="relative flex bg-black/30 p-0.5 rounded-full border border-[var(--border)] shrink min-w-0 overflow-x-auto [&::-webkit-scrollbar]:hidden">
          {/* Sliding indicator */}
          <motion.div
            className="absolute top-0.5 bottom-0.5 bg-jm-accent/20 border border-jm-accent/30 rounded-full z-0"
            animate={{ left: indicatorStyle.left, width: indicatorStyle.width }}
            transition={{ type: "spring", stiffness: 400, damping: 30 }}
          />
          {tabs.map((item, i) => {
            const Icon = item.icon;
            const isActive = activeTab === item.id;
            return (
              <motion.button
                key={item.id}
                data-tab={item.id}
                initial={{ opacity: 0, y: -10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.1 + i * 0.05, duration: 0.3 }}
                onClick={() => setActiveTab(item.id)}
                className={`relative z-10 flex items-center gap-1.5 px-3 py-1.5 rounded-full transition-colors duration-200 whitespace-nowrap text-xs md:text-sm shrink-0 ${isActive ? "text-jm-accent-light font-bold" : "hover:text-jm-accent-light"}`}
                style={!isActive ? { color: "var(--text-secondary)" } : undefined}
              >
                <Icon size={16} />
                <span className="hidden lg:inline">{item.label}</span>
              </motion.button>
            );
          })}
        </nav>

        {/* Account button */}
        <motion.button
          initial={{ opacity: 0, x: 20 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ delay: 0.3, duration: 0.4 }}
          whileHover={{ scale: 1.05 }}
          whileTap={{ scale: 0.95 }}
          onClick={() => setActiveTab("account")}
          className={`flex items-center gap-2 px-2 py-1 pr-3 rounded-full border transition-all duration-200 shrink-0 ${activeTab === "account" ? "border-jm-accent bg-jm-accent/10 glow-pulse" : "border-[var(--border)] bg-black/30 hover:border-jm-accent/50"}`}
        >
          <motion.img
            src={activeAvatar}
            alt="Avatar"
            className="w-7 h-7 rounded-full object-cover shrink-0"
            style={{ imageRendering: "pixelated" }}
            whileHover={{ rotate: [0, -5, 5, 0] }}
            transition={{ duration: 0.4 }}
          />
          <div className="hidden sm:flex flex-col items-start overflow-hidden">
            <span className="text-xs font-bold leading-tight truncate max-w-[100px] text-left">{activeAccount ? activeAccount.username : "Offline"}</span>
            <span className="text-[9px] leading-tight uppercase" style={{ color: "var(--text-secondary)" }}>{activeAccount ? activeAccount.acc_type : "..."}</span>
          </div>
        </motion.button>
      </motion.header>

      {/* Main content with tab transitions */}
      {/* Update banner */}
      <AnimatePresence>
        {updateInfo && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            className="mx-3 mt-1 overflow-hidden z-30"
          >
            <div className="p-3 rounded-xl border border-jm-accent bg-jm-accent/10 flex items-center gap-3">
              <span className="text-sm font-bold flex-1">
                Доступно обновление v{updateInfo.latest}
              </span>
              <motion.button
                whileHover={{ scale: 1.05 }}
                whileTap={{ scale: 0.95 }}
                onClick={async () => {
                  showToast("Загрузка обновления...");
                  try {
                    await invoke("download_and_apply_update");
                  } catch (e) { showToast(`Ошибка: ${e}`); }
                }}
                className="bg-jm-accent text-black px-4 py-1.5 rounded-lg font-bold text-sm"
              >
                Обновить
              </motion.button>
              <button onClick={() => setUpdateInfo(null)} className="text-jm-accent hover:text-jm-accent-light text-sm">✕</button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <main className="flex-grow relative overflow-hidden z-10" style={{ background: "radial-gradient(ellipse at top, rgba(var(--accent-rgb),0.05) 0%, transparent 80%)" }}>
        <AnimatePresence mode="wait">
          <motion.div
            key={activeTab}
            initial={{ opacity: 0, scale: 0.96, y: 15, filter: "blur(4px)" }}
            animate={{ opacity: 1, scale: 1, y: 0, filter: "blur(0px)" }}
            exit={{ opacity: 0, scale: 1.02, y: -10, filter: "blur(4px)" }}
            transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
            className="absolute inset-0 p-3 md:p-6 overflow-y-auto custom-scrollbar"
          >
            {activeTab === "news" && <NewsTab />}
            {activeTab === "home" && <HomeTab setActiveTab={setActiveTab} openInstance={(id: string) => { setPendingInstanceId(id); setPendingServerIp(undefined); setPendingWorldName(undefined); setActiveTab("library"); }} onLaunchWithServer={(id, ip) => { setPendingInstanceId(id); setPendingServerIp(ip); setPendingWorldName(undefined); setActiveTab("library"); }} onLaunchWorld={(id, worldName) => { setPendingInstanceId(id); setPendingServerIp(undefined); setPendingWorldName(worldName); setActiveTab("library"); }} />}
            {activeTab === "library" && <LibraryTab initialInstanceId={pendingInstanceId} initialServerIp={pendingServerIp} initialWorldName={pendingWorldName} onInstanceOpened={() => setPendingInstanceId(undefined)} onServerLaunchConsumed={() => setPendingServerIp(undefined)} onWorldLaunchConsumed={() => setPendingWorldName(undefined)} busyInstanceId={busyInstanceId} progress={progress} />}
            {activeTab === "skins" && <SkinsTab />}
            {activeTab === "discover" && <DiscoverTab />}
            {activeTab === "settings" && <SettingsTab />}
            {activeTab === "account" && <AccountTab />}
          </motion.div>
        </AnimatePresence>
      </main>

      {/* Floating download indicator */}
      <AnimatePresence>
        {showDownload && (
          <div className="absolute inset-0 pointer-events-none z-[10000]" aria-hidden="true">
            <motion.div
              initial={{ opacity: 0, y: 50, scale: 0.8 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: 50, scale: 0.8 }}
              onMouseEnter={() => setIsHoveringDL(true)}
              onMouseLeave={() => setIsHoveringDL(false)}
              className={`absolute bottom-6 left-6 pointer-events-auto glass border border-jm-accent shadow-[0_10px_30px_rgba(var(--accent-rgb),0.2)] rounded-full flex items-center transition-all duration-300 overflow-hidden ${isHoveringDL ? 'w-80 p-3 rounded-2xl' : 'w-14 h-14 justify-center cursor-pointer glow-pulse'}`}
            >
              {isHoveringDL ? (
                <div className="flex flex-col w-full px-2">
                  <div className="flex justify-between items-center mb-2">
                    <span className="text-xs font-bold text-white truncate pr-2">{progress.task_name || "Загрузка..."}</span>
                    <span className="text-xs text-jm-accent font-bold">{percent}%</span>
                  </div>
                  <div className="w-full bg-white/10 rounded-full h-2 overflow-hidden">
                    <motion.div
                      className="bg-gradient-to-r from-jm-accent to-jm-accent-light h-full rounded-full progress-striped"
                      animate={{ width: `${percent}%` }}
                      transition={{ duration: 0.3 }}
                    />
                  </div>
                  <div className="text-[10px] mt-1 text-right" style={{ color: "var(--text-secondary)" }}>{progress.downloaded} / {progress.total} файлов</div>
                </div>
              ) : (
                <div className="relative flex items-center justify-center w-full h-full">
                  <svg className="w-full h-full transform -rotate-90 absolute inset-0 text-jm-accent/20" viewBox="0 0 36 36"><circle cx="18" cy="18" r="16" fill="none" strokeWidth="3" stroke="currentColor"></circle></svg>
                  <svg className="w-full h-full transform -rotate-90 absolute inset-0 text-jm-accent transition-all duration-200" viewBox="0 0 36 36"><circle cx="18" cy="18" r="16" fill="none" strokeWidth="3" strokeDasharray="100" strokeDashoffset={100 - percent} strokeLinecap="round" stroke="currentColor"></circle></svg>
                  <Loader2 size={16} className="text-jm-accent animate-spin" />
                </div>
              )}
            </motion.div>
          </div>
        )}
      </AnimatePresence>

      {/* Toast notifications */}
      <div className="absolute bottom-6 right-6 z-[9999] flex flex-col gap-2">
        <AnimatePresence>
          {toasts.map(t => (
            <motion.div
              key={t.id}
              initial={{ opacity: 0, x: 80, scale: 0.8 }}
              animate={{ opacity: 1, x: 0, scale: 1 }}
              exit={{ opacity: 0, x: 80, scale: 0.8 }}
              transition={{ type: "spring", stiffness: 300, damping: 25 }}
              className="glass border-l-4 border-jm-accent p-4 rounded-xl shadow-xl flex items-center gap-3"
            >
              <motion.div animate={{ rotate: [0, 15, -15, 0] }} transition={{ duration: 0.5 }}>
                <Info size={18} className="text-jm-accent" />
              </motion.div>
              <span className="text-sm font-bold text-white">{t.msg}</span>
            </motion.div>
          ))}
        </AnimatePresence>
      </div>

      {/* Initial load animation overlay */}
      <AnimatePresence>
        {!ready && (
          <motion.div
            exit={{ opacity: 0 }}
            transition={{ duration: 0.5 }}
            className="absolute inset-0 z-[99999] bg-jm-bg flex items-center justify-center"
          >
            <motion.div
              animate={{ scale: [1, 1.1, 1], opacity: [0.5, 1, 0.5] }}
              transition={{ duration: 1.5, repeat: Infinity }}
              className="text-2xl font-bold text-jm-accent-light"
            >
              JentleMemes
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
export default App;
