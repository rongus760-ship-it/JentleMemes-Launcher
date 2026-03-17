import { useState, useEffect } from "react";
import { Home, Library, Compass, Settings, Shirt, Loader2, Info } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { motion, AnimatePresence } from "framer-motion";

import HomeTab from "./tabs/HomeTab";
import LibraryTab from "./tabs/LibraryTab";
import AccountTab from "./tabs/AccountTab";
import SkinsTab from "./tabs/SkinsTab";
import DiscoverTab from "./tabs/DiscoverTab";
import SettingsTab from "./tabs/SettingsTab";
import Titlebar from "./Titlebar";

export const showToast = (msg: string) => window.dispatchEvent(new CustomEvent("jm_toast", { detail: msg }));

// Звук клика (Осциллятор, без файлов)
export const playClickSound = () => {
  try {
    const ctx = new (window.AudioContext || (window as any).webkitAudioContext)();
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.connect(gain); gain.connect(ctx.destination);
    osc.type = 'sine'; osc.frequency.setValueAtTime(600, ctx.currentTime);
    osc.frequency.exponentialRampToValueAtTime(300, ctx.currentTime + 0.1);
    gain.gain.setValueAtTime(0.05, ctx.currentTime); gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.1);
    osc.start(); osc.stop(ctx.currentTime + 0.1);
  } catch(e){}
};

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

  const [theme, setTheme] = useState("jentle-dark");

  async function loadSettings() {
    try {
      const s: any = await invoke("load_settings");
      setTheme(s.theme || "jentle-dark");
      if (s.background) {
        // Подгружаем картинку из папки при необходимости
      }
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
    <div className={`flex flex-col h-screen bg-jm-bg text-white overflow-hidden font-sans rounded-lg border border-white/5 shadow-2xl relative ${theme === 'theme-furry' ? 'theme-furry' : ''}`}>
      <Titlebar />
      
      <header className="grid grid-cols-3 items-center px-8 py-4 bg-black/40 backdrop-blur-md border-b border-white/10 shadow-lg z-40 shrink-0">
        <div className="text-2xl font-bold text-jm-accent-light tracking-wide justify-self-start">JentleMemes</div>
        <nav className="flex bg-black/50 p-1 rounded-full border border-white/5 justify-self-center">
          {[
            { id: "home", label: "Главная", icon: <Home size={18} /> },
            { id: "library", label: "Сборки", icon: <Library size={18} /> },
            { id: "skins", label: "Скины", icon: <Shirt size={18} /> },
            { id: "discover", label: "Браузер", icon: <Compass size={18} /> },
            { id: "settings", label: "Настройки", icon: <Settings size={18} /> },
          ].map((item) => (
            <button key={item.id} onClick={() => setActiveTab(item.id)} className={`flex items-center gap-2 px-5 py-2 rounded-full transition-all duration-300 ${activeTab === item.id ? "bg-jm-accent/20 text-jm-accent-light font-bold shadow-[0_0_15px_rgba(134,168,134,0.2)] border border-jm-accent/30 scale-105" : "text-gray-400 hover:text-white hover:bg-white/5 border border-transparent"}`}>
              {item.icon} <span className="text-sm hidden md:block">{item.label}</span>
            </button>
          ))}
        </nav>
        <div className="justify-self-end flex justify-end">
          <button onClick={() => setActiveTab("account")} className={`flex items-center gap-3 px-2 py-1 pr-4 rounded-full border transition-all duration-300 max-w-[200px] ${activeTab === "account" ? "border-jm-accent bg-jm-accent/10 scale-105" : "border-white/10 bg-black/50 hover:border-jm-accent/50"}`}>
            <img src={activeAvatar} alt="Avatar" className="w-8 h-8 rounded-full object-cover shrink-0" style={{ imageRendering: "pixelated" }} />
            <div className="flex flex-col items-start overflow-hidden">
              <span className="text-sm font-bold leading-tight truncate w-full text-left">{activeAccount ? activeAccount.username : "Offline"}</span>
              <span className="text-[10px] text-gray-400 leading-tight uppercase">{activeAccount ? activeAccount.acc_type : "Не выбран"}</span>
            </div>
          </button>
        </div>
      </header>

      <main className="flex-grow relative overflow-hidden bg-[radial-gradient(ellipse_at_top,rgba(134,168,134,0.05)_0%,transparent_80%)]">
        <AnimatePresence mode="wait">
          <motion.div key={activeTab} initial={{ opacity: 0, scale: 0.96, y: 15 }} animate={{ opacity: 1, scale: 1, y: 0 }} exit={{ opacity: 0, scale: 1.02, y: -10 }} transition={{ duration: 0.25, ease: "easeOut" }} className="absolute inset-0 p-4 md:p-8 overflow-y-auto custom-scrollbar">
            {activeTab === "home" && <HomeTab setActiveTab={setActiveTab} openInstance={(id: string) => { setPendingInstanceId(id); setPendingServerIp(undefined); setPendingWorldName(undefined); setActiveTab("library"); }} onLaunchWithServer={(id, ip) => { setPendingInstanceId(id); setPendingServerIp(ip); setPendingWorldName(undefined); setActiveTab("library"); }} onLaunchWorld={(id, worldName) => { setPendingInstanceId(id); setPendingServerIp(undefined); setPendingWorldName(worldName); setActiveTab("library"); }} />}
            {activeTab === "library" && <LibraryTab initialInstanceId={pendingInstanceId} initialServerIp={pendingServerIp} initialWorldName={pendingWorldName} onInstanceOpened={() => setPendingInstanceId(undefined)} onServerLaunchConsumed={() => setPendingServerIp(undefined)} onWorldLaunchConsumed={() => setPendingWorldName(undefined)} busyInstanceId={busyInstanceId} progress={progress} />}
            {activeTab === "skins" && <SkinsTab />}
            {activeTab === "discover" && <DiscoverTab />}
            {activeTab === "settings" && <SettingsTab />}
            {activeTab === "account" && <AccountTab />}
          </motion.div>
        </AnimatePresence>
      </main>

      {/* Прогресс-бар поверх всего UI (высокий z-index, вне main) */}
      <AnimatePresence>
        {showDownload && (
          <div className="absolute inset-0 pointer-events-none z-[10000]" aria-hidden="true">
            <motion.div initial={{ opacity: 0, y: 50, scale: 0.8 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: 50, scale: 0.8 }} onMouseEnter={() => setIsHoveringDL(true)} onMouseLeave={() => setIsHoveringDL(false)} className={`absolute bottom-6 left-6 pointer-events-auto bg-black/80 backdrop-blur-xl border border-jm-accent shadow-[0_10px_30px_rgba(134,168,134,0.2)] rounded-full flex items-center transition-all duration-300 overflow-hidden ${isHoveringDL ? 'w-80 p-3 rounded-2xl' : 'w-14 h-14 justify-center cursor-pointer'}`}>
            {isHoveringDL ? (
              <div className="flex flex-col w-full px-2">
                <div className="flex justify-between items-center mb-2"><span className="text-xs font-bold text-white truncate pr-2">{progress.task_name || "Загрузка..."}</span><span className="text-xs text-jm-accent">{percent}%</span></div>
                <div className="w-full bg-white/10 rounded-full h-1.5 overflow-hidden"><div className="bg-jm-accent h-full transition-all duration-200" style={{ width: `${percent}%` }}></div></div>
                <div className="text-[10px] text-gray-400 mt-1 text-right">{progress.downloaded} / {progress.total} файлов</div>
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

      <div className="absolute bottom-6 right-6 z-[9999] flex flex-col gap-2">
        <AnimatePresence>
          {toasts.map(t => (
            <motion.div key={t.id} initial={{ opacity: 0, x: 50 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, scale: 0.9 }} className="bg-jm-card border-l-4 border-jm-accent p-4 rounded-xl shadow-xl flex items-center gap-3">
              <Info size={18} className="text-jm-accent" />
              <span className="text-sm font-bold text-white">{t.msg}</span>
            </motion.div>
          ))}
        </AnimatePresence>
      </div>
    </div>
  );
}
export default App;