import React, { useState, useEffect, useRef, useMemo } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Play, Plus, Trash2, PackageOpen, Archive, Loader2, Settings, Terminal, Puzzle, ArrowLeft, Square, Search, FolderOpen, RefreshCw, X, Wrench, Download, Globe, Server } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import DiscoverTab from "./DiscoverTab";
import LoaderIcon from "../components/LoaderIcon";
import { showToast } from "../App";

// ЛОКАЛЬНЫЙ ПРЕДОХРАНИТЕЛЬ
class SafeBoundary extends React.Component<any, { hasError: boolean, error: any }> {
  constructor(props: any) { super(props); this.state = { hasError: false, error: null }; }
  static getDerivedStateFromError(error: any) { return { hasError: true, error }; }
  render() {
    if (this.state.hasError) return <div className="p-6 bg-red-900/50 border border-red-500 text-white rounded-xl m-6">Критическая ошибка компонента: {this.state.error?.toString()}</div>;
    return this.props.children;
  }
}

function ToggleSwitch({ checked, onChange }: { checked: boolean, onChange: (c: boolean) => void }) {
  return (
    <div onClick={(e) => { e.stopPropagation(); onChange(!checked); }} className={`w-12 h-6 flex items-center rounded-full p-1 cursor-pointer transition-colors duration-300 ${checked ? 'bg-jm-accent' : 'bg-gray-600'}`}>
      <div className={`bg-white w-4 h-4 rounded-full shadow-md transform transition-transform duration-300 ${checked ? 'translate-x-6' : ''}`} />
    </div>
  );
}

function CustomSelect({ label, value, options, onChange, disabled = false }: any) {
  const[isOpen, setIsOpen] = useState(false);
  const selectRef = useRef<HTMLDivElement>(null);
  const safeOptions = Array.isArray(options) ? options :[];
  const selectedOption = safeOptions.find((o: any) => o.value === value) || { label: "Выбрать..." };

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) { if (selectRef.current && !selectRef.current.contains(event.target as Node)) setIsOpen(false); }
    document.addEventListener("mousedown", handleClickOutside); return () => document.removeEventListener("mousedown", handleClickOutside);
  },[]);

  return (
    <div className={`flex flex-col relative w-full ${disabled ? 'opacity-50 pointer-events-none' : ''}`} ref={selectRef}>
      {label && <label className="text-sm text-[var(--text-secondary)] mb-1">{label}</label>}
      <div onClick={() => setIsOpen(!isOpen)} className="bg-black/50 border border-white/10 rounded-xl px-4 py-3 text-white cursor-pointer select-none hover:border-jm-accent transition-colors flex justify-between items-center">
        <span className="truncate pr-2">{selectedOption.label}</span><span className="text-xs opacity-50">▼</span>
      </div>
      <AnimatePresence>
        {isOpen && (
          <motion.div initial={{ opacity: 0, y: -10 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -10 }} className="absolute top-[100%] mt-2 w-full bg-[var(--input-bg)] border border-white/10 rounded-xl z-50 max-h-60 overflow-y-auto custom-scrollbar shadow-2xl">
            {safeOptions.map((o: any) => (
              <div key={o.value} onClick={() => { onChange(o.value); setIsOpen(false); }} className={`px-4 py-3 cursor-pointer transition-colors text-sm ${value === o.value ? 'bg-jm-accent/20 text-jm-accent-light' : 'text-white hover:bg-white/10'}`}>
                {o.label}
              </div>
            ))}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function ExportModal({ instanceId, instanceName, onClose, showToast }: { instanceId: string; instanceName: string; onClose: () => void; showToast: (msg: string) => void }) {
  const [format, setFormat] = useState<"zip" | "mrpack" | "jentlepack">("zip");
  const [folders, setFolders] = useState<Record<string, boolean>>({});
  const [exporting, setExporting] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);

  useEffect(() => {
    invoke("list_instance_folders", { id: instanceId }).then((dirs: any) => {
      const state: Record<string, boolean> = {};
      for (const d of (dirs || [])) {
        state[d] = ["mods", "config", "resourcepacks", "shaderpacks"].includes(d);
      }
      setFolders(state);
    }).catch(() => setFolders({ mods: true, config: true, resourcepacks: true }));
  }, [instanceId]);

  const toggleFolder = (key: string) => setFolders(prev => ({ ...prev, [key]: !prev[key] }));
  const toggleAll = (on: boolean) => setFolders(prev => Object.fromEntries(Object.keys(prev).map(k => [k, on])));
  const selectedFolders = Object.entries(folders).filter(([, v]) => v).map(([k]) => k);

  const doExport = async () => {
    setExporting(true);
    try {
      const cmd = format === "mrpack" ? "export_mrpack" : "export_instance";
      const res = await invoke(cmd, { id: instanceId, selectedFolders });
      showToast(res as string);
      onClose();
    } catch (e) {
      showToast(`Ошибка экспорта: ${e}`);
    } finally {
      setExporting(false);
    }
  };

  const folderLabels: Record<string, string> = {
    mods: "Моды", config: "Конфигурация", resourcepacks: "Ресурспаки",
    shaderpacks: "Шейдеры", saves: "Миры", scripts: "Скрипты",
    logs: "Логи", crash_reports: "Краш-репорты", options: "Настройки игры",
    screenshots: "Скриншоты", schematics: "Схематики",
  };
  const commonFolders = ["mods", "config", "resourcepacks", "shaderpacks", "saves"];
  const advancedFolders = Object.keys(folders).filter(k => !commonFolders.includes(k));

  return (
    <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="fixed inset-0 z-[110] bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
      <motion.div initial={{ scale: 0.95, y: 20 }} animate={{ scale: 1, y: 0 }} exit={{ scale: 0.95, y: 20 }} className="bg-jm-card border border-white/10 p-8 rounded-3xl w-full max-w-md shadow-2xl max-h-[85vh] overflow-y-auto custom-scrollbar">
        <h3 className="text-2xl font-bold text-white mb-6">Экспорт «{instanceName}»</h3>

        <div className="mb-6">
          <label className="text-sm text-[var(--text-secondary)] mb-2 block">Формат</label>
          <div className="flex gap-2">
            {([["zip", ".zip"], ["mrpack", ".mrpack"], ["jentlepack", ".jentlepack"]] as const).map(([f, label]) => (
              <button key={f} onClick={() => setFormat(f)} className={`flex-1 py-2.5 rounded-xl font-bold text-xs transition-colors border ${format === f ? "bg-jm-accent text-black border-jm-accent" : "bg-white/5 text-[var(--text-secondary)] border-white/10 hover:border-white/30"}`}>{label}</button>
            ))}
          </div>
        </div>

        <div className="mb-4">
          <div className="flex items-center justify-between mb-2">
            <label className="text-sm text-[var(--text-secondary)]">Включить папки</label>
            <div className="flex gap-2 text-[10px]">
              <button onClick={() => toggleAll(true)} className="text-jm-accent hover:underline font-bold">Все</button>
              <button onClick={() => toggleAll(false)} className="text-[var(--text-secondary)] hover:underline font-bold">Нет</button>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-2">
            {commonFolders.filter(k => k in folders).map(key => (
              <label key={key} className="flex items-center gap-2 cursor-pointer p-3 bg-black/30 rounded-xl border border-white/5 hover:border-white/20 transition-colors">
                <input type="checkbox" checked={folders[key]} onChange={() => toggleFolder(key)} className="w-4 h-4 accent-jm-accent cursor-pointer" />
                <div>
                  <span className="text-white text-sm font-bold block">{folderLabels[key] || key}</span>
                  <span className="text-[10px] text-[var(--text-secondary)]">/{key}</span>
                </div>
              </label>
            ))}
          </div>
        </div>

        {advancedFolders.length > 0 && (
          <div className="mb-6">
            <button onClick={() => setShowAdvanced(!showAdvanced)} className="flex items-center gap-2 text-sm text-[var(--text-secondary)] hover:text-jm-accent font-bold transition-colors mb-2">
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" style={{ transform: showAdvanced ? "rotate(180deg)" : "none", transition: "transform 0.2s" }}><polyline points="6 9 12 15 18 9" /></svg>
              Расширенные ({advancedFolders.length})
            </button>
            {showAdvanced && (
              <div className="grid grid-cols-2 gap-2">
                {advancedFolders.map(key => (
                  <label key={key} className="flex items-center gap-2 cursor-pointer p-2.5 bg-black/20 rounded-lg border border-white/5 hover:border-white/15 transition-colors">
                    <input type="checkbox" checked={folders[key]} onChange={() => toggleFolder(key)} className="w-3.5 h-3.5 accent-jm-accent cursor-pointer" />
                    <div>
                      <span className="text-white text-xs font-bold block">{folderLabels[key] || key}</span>
                      <span className="text-[9px] text-[var(--text-secondary)]">/{key}</span>
                    </div>
                  </label>
                ))}
              </div>
            )}
          </div>
        )}

        <div className="flex gap-3">
          <button onClick={doExport} disabled={exporting || selectedFolders.length === 0} className="flex-1 bg-jm-accent hover:bg-jm-accent-light text-black py-3 rounded-xl font-bold transition-colors disabled:opacity-50">
            {exporting ? "Экспорт..." : "Экспортировать"}
          </button>
          <button onClick={onClose} className="px-6 py-3 bg-white/10 hover:bg-white/20 text-white rounded-xl font-bold transition-colors">Отмена</button>
        </div>
      </motion.div>
    </motion.div>
  );
}

function ImportDropdown({ onImported }: { onImported: () => void }) {
  return (
    <button
      onClick={async () => {
        try { const res = await invoke("import_instance"); showToast(res as string); onImported(); } catch (e) { showToast(`Ошибка импорта: ${e}`); }
      }}
      className="bg-white/10 hover:bg-white/20 text-white px-3 py-2 rounded-lg font-bold text-xs flex items-center gap-1.5 transition-colors"
    >
      <PackageOpen size={14} /> Импорт
    </button>
  );
}

type ProgressPayload = { task_name: string; downloaded: number; total: number; instance_id?: string };
export default function LibraryTab({ initialInstanceId, initialServerIp, initialWorldName, onInstanceOpened, onServerLaunchConsumed, onWorldLaunchConsumed, busyInstanceId = null, progress = { task_name: "", downloaded: 0, total: 0 } }: { initialInstanceId?: string, initialServerIp?: string, initialWorldName?: string, onInstanceOpened?: () => void, onServerLaunchConsumed?: () => void, onWorldLaunchConsumed?: () => void, busyInstanceId?: string | null, progress?: ProgressPayload } = {}) {
  const[instances, setInstances] = useState<any[]>([]);
  const[activeAccount, setActiveAccount] = useState<any>(null);
  
  const[isCreating, setIsCreating] = useState(false);
  const[newName, setNewName] = useState("");
  const[newLoader, setNewLoader] = useState("fabric");
  const [newVersion, setNewVersion] = useState("1.20.1");
  const[newLoaderVersion, setNewLoaderVersion] = useState("");
  const[newIcon, setNewIcon] = useState("");
  
  const[availableVersions, setAvailableVersions] = useState<string[]>([]);
  const[availableLoaderVersions, setAvailableLoaderVersions] = useState<string[]>([]);
  const[isLoadingVersions, setIsLoadingVersions] = useState(false);
  const[isLoadingLoaderVersions, setIsLoadingLoaderVersions] = useState(false);

  const[selectedInstance, setSelectedInstance] = useState<any>(null);
  const[instanceTab, setInstanceTab] = useState("content"); 
  const[settingsSubTab, setSettingsSubTab] = useState("general");
  
  const[runningInstances, setRunningInstances] = useState<string[]>([]);
  const [logs, setLogs] = useState<string[]>([]);
  const [packUpdateInfo, setPackUpdateInfo] = useState<any>(null);
  const [isCheckingPackUpdate, setIsCheckingPackUpdate] = useState(false);
  const [isUpdatingPack, setIsUpdatingPack] = useState(false);
  const logsEndRef = useRef<HTMLDivElement>(null);

  const [contentByFolder, setContentByFolder] = useState<{ mods: any[]; resourcepacks: any[]; shaderpacks: any[] }>({ mods: [], resourcepacks: [], shaderpacks: [] });
  const[modSearch, setModSearch] = useState("");
  const [modFilter, setModFilter] = useState("all");
  const[updates, setUpdates] = useState<any>({});
  const[isCheckingUpdates, setIsCheckingUpdates] = useState(false);
  const[showModBrowser, setShowModBrowser] = useState(false);
  const[openModProjectId, setOpenModProjectId] = useState<string | undefined>(undefined);

  const[instSettings, setInstSettings] = useState({ override_global: false, ram_mb: 4096, jvm_args: "", use_discrete_gpu: false });
  const [maxRam, setMaxRam] = useState(8192);
  const[globalFilter, setGlobalFilter] = useState("all");
  const[confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const[confirmDeleteMod, setConfirmDeleteMod] = useState<string | null>(null);
  const[showExportModal, setShowExportModal] = useState(false);
  const[contentTab, setContentTab] = useState("mods");
  const[isLaunching, setIsLaunching] = useState(false);
  const[isRepairing, setIsRepairing] = useState(false);
  const[coreLoader, setCoreLoader] = useState("");
  const[coreGameVer, setCoreGameVer] = useState("");
  const[coreLoaderVer, setCoreLoaderVer] = useState("");
  const[coreVersions, setCoreVersions] = useState<string[]>([]);
  const[coreLoaderVersions, setCoreLoaderVersions] = useState<string[]>([]);
  const [packSourceInfo, setPackSourceInfo] = useState<any>(null);
  const [packVersions, setPackVersions] = useState<any[]>([]);
  const [selectedPackVersion, setSelectedPackVersion] = useState<any>(null);
  const [renameValue, setRenameValue] = useState("");
  const serverLaunchHandledRef = useRef(false);
  const worldLaunchHandledRef = useRef(false);
  const [worldsList, setWorldsList] = useState<string[]>([]);
  const [recentServers, setRecentServers] = useState<any[]>([]);
  const [lastWorldGlobal, setLastWorldGlobal] = useState<{ instance_id: string; instance_name: string; world_name: string } | null>(null);

  async function loadData() {
    try {
      const [insts, profs] = await Promise.all([
        invoke("get_instances"),
        invoke("load_profiles"),
      ]);
      const acc = (profs as any)?.accounts?.find((a: any) => a.id === (profs as any).active_account_id) || null;
      setActiveAccount(acc);
      const instanceList = (insts as any[]) || [];
      setInstances(instanceList);
      invoke("get_system_ram").then((r: any) => setMaxRam(r));

      if (initialServerIp && initialInstanceId && !serverLaunchHandledRef.current) {
        const inst = instanceList.find((i: any) => i.id === initialInstanceId);
        if (inst) {
          serverLaunchHandledRef.current = true;
          setSelectedInstance(inst);
          onServerLaunchConsumed?.();
          onInstanceOpened?.();
          invoke("update_server_last_played", { ip: initialServerIp, name: "", instanceId: inst.id }).catch(() => {});
          launchInstance(inst, initialServerIp, undefined, acc);
        }
      }
      if (initialWorldName && initialInstanceId && !initialServerIp && !worldLaunchHandledRef.current) {
        const inst = instanceList.find((i: any) => i.id === initialInstanceId);
        if (inst) {
          worldLaunchHandledRef.current = true;
          setSelectedInstance(inst);
          onWorldLaunchConsumed?.();
          onInstanceOpened?.();
          launchInstance(inst, undefined, initialWorldName, acc);
        }
      }
    } catch (e) { console.error(e); }
  }

  useEffect(() => { loadData(); }, []);

  useEffect(() => {
    if (!initialInstanceId || !instances.length || initialServerIp) return;
    const inst = instances.find(i => i.id === initialInstanceId);
    if (inst) {
      setSelectedInstance(inst);
      onInstanceOpened?.();
    }
  }, [initialInstanceId, instances]);

  useEffect(() => {
    if (selectedInstance) {
      setInstSettings(selectedInstance.settings || { override_global: false, ram_mb: 4096, jvm_args: "", use_discrete_gpu: false });
      setPackUpdateInfo(null);
      setRenameValue(selectedInstance.name || "");
      invoke("get_pack_source_info", { instance_id: selectedInstance.id }).then((info: any) => {
        setPackSourceInfo(info || null);
        if (info?.project_id) {
          invoke("get_modrinth_versions", { id: info.project_id }).then((vers: any) => {
            setPackVersions(Array.isArray(vers) ? vers : []);
            setSelectedPackVersion(info.version_id || null);
          }).catch(() => setPackVersions([]));
        } else setPackVersions([]);
      }).catch(() => setPackSourceInfo(null));
    }
  },[selectedInstance]);

  useEffect(() => {
    if (instanceTab === "options" && settingsSubTab === "core" && selectedInstance) {
      const loader = selectedInstance.loader;
      invoke("get_loader_versions", { loader }).then((vers: any) => setCoreVersions(vers || [])).catch(() => setCoreVersions([]));
      if (loader !== "vanilla") {
        invoke("get_specific_loader_versions", { loader, gameVersion: selectedInstance.game_version }).then((vers: any) => setCoreLoaderVersions(vers || [])).catch(() => setCoreLoaderVersions([]));
      }
    }
  }, [instanceTab, settingsSubTab, selectedInstance]);

  useEffect(() => {
    if (instanceTab === "options" && settingsSubTab === "core" && selectedInstance) {
      const loader = selectedInstance.loader;
      invoke("fetch_vanilla_versions").then((manifest: any) => {
        const vers = (manifest?.versions || []).map((v: any) => v.id).filter(Boolean);
        setCoreVersions(vers.length > 0 ? vers : [selectedInstance.game_version]);
      }).catch(() => setCoreVersions([selectedInstance.game_version]));
      if (loader !== "vanilla") {
        invoke("get_loader_versions", { loader }).then((vers: any) => {
          if (vers?.length) setCoreVersions(vers);
        }).catch(() => {});
        invoke("get_specific_loader_versions", { loader, gameVersion: selectedInstance.game_version }).then((vers: any) => {
          setCoreLoaderVersions(vers || []);
        }).catch(() => setCoreLoaderVersions([]));
      }
    }
  }, [instanceTab, settingsSubTab, selectedInstance]);

  useEffect(() => {
    if (!isCreating) return;
    setIsLoadingVersions(true);
    invoke("get_loader_versions", { loader: newLoader }).then((vers: any) => {
      setAvailableVersions(vers ||[]); 
      if (vers && vers.length > 0 && !vers.includes(newVersion)) setNewVersion(vers[0]);
    }).catch(() => { setAvailableVersions(["1.20.1"]); setNewVersion("1.20.1"); }).finally(() => setIsLoadingVersions(false));
  },[newLoader, isCreating]);

  useEffect(() => {
    if (!isCreating || !newVersion || newLoader === "vanilla") return;
    setIsLoadingLoaderVersions(true);
    invoke("get_specific_loader_versions", { loader: newLoader, gameVersion: newVersion }).then((vers: any) => {
      setAvailableLoaderVersions(vers ||[]); 
      if (vers && vers.length > 0) setNewLoaderVersion(vers[0]);
      else setNewLoaderVersion("");
    }).catch(() => setAvailableLoaderVersions([])).finally(() => setIsLoadingLoaderVersions(false));
  },[newVersion, newLoader, isCreating]);

  async function loadContent(folder: "mods" | "resourcepacks" | "shaderpacks") {
    if (!selectedInstance) return;
    try {
      const m: any = await invoke("get_installed_content", { instanceId: selectedInstance.id, folder });
      setContentByFolder(prev => ({ ...prev, [folder]: m || [] }));
    } catch (e) {
      setContentByFolder(prev => ({ ...prev, [folder]: [] }));
    }
  }
  const loadMods = () => loadContent("mods");

  useEffect(() => {
    if (settingsSubTab === "core" && selectedInstance) {
      const loader = selectedInstance.loader || "fabric";
      const gv = selectedInstance.game_version || "1.20.1";
      invoke("fetch_vanilla_versions").then((manifest: any) => {
        const vers = (manifest?.versions || []).map((v: any) => v.id).filter(Boolean);
        if (vers.length > 0) setCoreVersions(vers);
      }).catch(() => {});
      invoke("get_loader_versions", { loader }).then((vers: any) => {
        if (vers && vers.length > 0) setCoreVersions(vers);
      }).catch(() => {});
      if (loader !== "vanilla") {
        invoke("get_specific_loader_versions", { loader, gameVersion: gv }).then((vers: any) => {
          setCoreLoaderVersions(vers || []);
        }).catch(() => setCoreLoaderVersions([]));
      }
    }
  }, [settingsSubTab, selectedInstance]);

  useEffect(() => {
    const unlistenExit = listen<string>("exit_", (e) => {
      const id = e.payload;
      if (id) {
        setRunningInstances(prev => prev.filter(x => x !== id));
        showToast("Игра закрыта");
      }
    });
    return () => { unlistenExit.then(f=>f()); };
  }, []);

  useEffect(() => {
    if (selectedInstance) {
      loadContent("mods");
      loadContent("resourcepacks");
      loadContent("shaderpacks");
      const unlistenLog = listen(`log_${selectedInstance.id}`, (e: any) => {
        setLogs(prev =>[...prev.slice(-100), e.payload]);
        setTimeout(() => logsEndRef.current?.scrollIntoView({ behavior: "smooth" }), 50);
      });
      return () => { unlistenLog.then(f=>f()); };
    }
  },[selectedInstance]);

  useEffect(() => {
    if (settingsSubTab === "core" && selectedInstance) {
      const loader = selectedInstance.loader || "fabric";
      invoke("get_loader_versions", { loader }).then((vers: any) => setCoreVersions(vers || []));
      if (loader !== "vanilla") {
        invoke("get_specific_loader_versions", { loader, gameVersion: selectedInstance.game_version }).then((vers: any) => setCoreLoaderVersions(vers || []));
      }
    }
  }, [settingsSubTab, selectedInstance]);

  async function handleCreate() {
    if (!newName.trim() || !newVersion) return;
    await invoke("create_instance", { name: newName, gameVersion: newVersion, loader: newLoader, loaderVersion: newLoader === "vanilla" ? "" : newLoaderVersion, icon: newIcon || null });
    setIsCreating(false); setNewName(""); setNewIcon(""); loadData(); showToast("Сборка создана!");
  }

  const saveInstSettingsDebounced = useRef<ReturnType<typeof setTimeout> | null>(null);
  async function saveInstSettings(newSettings: any, skipDebounce = false) {
    setInstSettings(newSettings);
    if (skipDebounce) {
      if (saveInstSettingsDebounced.current) clearTimeout(saveInstSettingsDebounced.current);
      await invoke("save_instance_settings", { id: selectedInstance.id, settings: newSettings });
      loadData();
      showToast("Настройки сохранены");
      return;
    }
    if (saveInstSettingsDebounced.current) clearTimeout(saveInstSettingsDebounced.current);
    saveInstSettingsDebounced.current = setTimeout(async () => {
      saveInstSettingsDebounced.current = null;
      await invoke("save_instance_settings", { id: selectedInstance.id, settings: newSettings });
      loadData();
      showToast("Настройки сохранены");
    }, 600);
  }

  async function handleDelete(id: string) {
    try {
      showToast("Удаление сборки...");
      await invoke("delete_instance", { id });
      setConfirmDeleteId(null);
      if (selectedInstance?.id === id) {
        setSelectedInstance(null);
      }
      loadData();
      showToast("Сборка успешно удалена");
    } catch (e) {
      showToast(`Ошибка удаления: ${e}`);
    }
  }

  async function checkPackUpdate() {
    if (!selectedInstance) return;
    setIsCheckingPackUpdate(true);
    setPackUpdateInfo(null);
    try {
      const info: any = await invoke("check_modpack_update", { instanceId: selectedInstance.id });
      setPackUpdateInfo(info);
      if (info?.has_update) {
        showToast(`Доступна новая версия: ${info.latest_version || ""}`);
      } else if (info?.reason) {
        showToast(info.reason);
      } else {
        showToast("Сборка актуальна");
      }
    } catch (e) {
      showToast(`Ошибка проверки: ${e}`);
      setPackUpdateInfo(null);
    } finally {
      setIsCheckingPackUpdate(false);
    }
  }

  async function applyPackUpdate() {
    if (!selectedInstance || !packUpdateInfo?.has_update || !packUpdateInfo?.update_url) return;
    setIsUpdatingPack(true);
    try {
      await invoke("update_modpack", { instanceId: selectedInstance.id, updateUrl: packUpdateInfo.update_url });
      showToast("Сборка обновлена!");
      setPackUpdateInfo(null);
      loadMods();
      loadData();
    } catch (e) {
      showToast(`Ошибка обновления: ${e}`);
    } finally {
      setIsUpdatingPack(false);
    }
  }

  async function launchInstance(inst: any, serverIp?: string, worldName?: string, accountOverride?: any) {
    const account = accountOverride || activeAccount;
    if (!account) {
      // Попробуем загрузить аккаунт вручную
      try {
        const profs: any = await invoke("load_profiles");
        const acc = profs?.accounts?.find((a: any) => a.id === profs.active_account_id) || null;
        if (acc) { setActiveAccount(acc); return launchInstance(inst, serverIp, worldName, acc); }
      } catch {}
      return showToast("Выберите аккаунт в профиле!");
    }
    if (runningInstances.includes(inst.id)) {
      await invoke("stop_instance", { instanceId: inst.id });
      setRunningInstances(prev => prev.filter(id => id !== inst.id));
      return;
    }

    try {
      setIsLaunching(true);
      showToast("Подготовка к запуску...");
      setInstanceTab("logs"); 
      setLogs([]);
      
      let playName = account.username;
      if (account.active_skin_id) {
        const profs: any = await invoke("load_profiles");
        const skin = profs.skin_presets.find((p: any) => p.id === account.active_skin_id);
        if (skin && (skin.skin_type === "nickname" || skin.username)) playName = skin.skin_data || skin.username;
      }
      
      let launchVersion = inst.game_version;
      const manifest: any = await invoke("fetch_vanilla_versions");
      const vInfo = manifest.versions.find((v:any) => v.id === inst.game_version);
      if(!vInfo) throw "Версия не найдена!";

      await invoke("install_version", { versionId: inst.game_version, url: vInfo.url });

      if (inst.loader === "fabric") {
        launchVersion = await invoke("install_fabric", { versionId: inst.game_version }) as string;
      } else if (inst.loader === "quilt") {
        launchVersion = await invoke("install_quilt", { versionId: inst.game_version }) as string;
      } else if (inst.loader === "forge" || inst.loader === "neoforge") {
        launchVersion = await invoke("install_forge", { 
          instanceId: inst.id, 
          gameVersion: inst.game_version, 
          loaderVersion: inst.loader_version || "", 
          loaderName: inst.loader 
        });
      }

      await invoke("download_game_files", { versionId: launchVersion, instanceId: inst.id });
      await invoke("extract_natives", { versionId: launchVersion });

      setRunningInstances([...runningInstances, inst.id]);
      
      if (serverIp) invoke("update_server_last_played", { ip: serverIp, name: "", instanceId: inst.id }).catch(() => {});
      
      await invoke("launch_game", {
        instanceId: inst.id,
        versionId: launchVersion,
        username: playName,
        uuid: account.uuid || "00000000-0000-0000-0000-000000000000",
        token: account.token || "0",
        accType: account.acc_type || "offline",
        serverIp: serverIp || "",
        worldName: worldName || null
      });
    } catch(e) { showToast(`Ошибка: ${e}`); setRunningInstances(prev => prev.filter(id => id !== inst.id)); }
    finally { setIsLaunching(false); }
  }

  async function toggleMod(filename: string, enable: boolean) {
    await invoke("toggle_mod", { instanceId: selectedInstance.id, filename, enable, folder: contentTab });
    loadMods();
  }

  async function deleteMod(filename: string) {
    try {
      await invoke("delete_mod", { instanceId: selectedInstance.id, filename, folder: contentTab });
      setConfirmDeleteMod(null);
      loadMods();
    } catch (e) { showToast(`Ошибка: ${e}`); }
  }

  async function checkForUpdates() {
    setIsCheckingUpdates(true);
    try {
      const hashes = (contentByFolder.mods || []).map(m => m.hash);
      const res: any = await invoke("check_mod_updates", { hashes, loader: selectedInstance.loader, gameVersion: selectedInstance.game_version });
      
      const actualUpdates: any = {};
      let count = 0;
      for (const hash in res) {
         const mod = contentByFolder.mods.find(m => m.hash === hash);
         if (mod && res[hash].id !== mod.version_id) {
             actualUpdates[hash] = res[hash];
             count++;
         }
      }

      setUpdates(actualUpdates);
      if (count > 0) showToast(`Найдено обновлений: ${count}`);
      else showToast("Все моды обновлены!");
    } catch (e) { showToast("Ошибка проверки"); }
    finally { setIsCheckingUpdates(false); }
  }

  async function updateMod(oldFilename: string, oldHash: string) {
    const updateInfo = updates[oldHash];
    if (!updateInfo) return;
    showToast("Обновление мода...");
    try {
      await invoke("delete_mod", { instanceId: selectedInstance.id, filename: oldFilename, folder: contentTab });
      await invoke("install_mod_with_dependencies", { 
        instanceId: selectedInstance.id, 
        versionId: updateInfo.id,
        gameVersion: selectedInstance.game_version,
        loader: selectedInstance.loader
      });
      showToast("Мод обновлен!");
      loadMods();
      const newUpdates = {...updates}; delete newUpdates[oldHash]; setUpdates(newUpdates);
    } catch (e) { showToast("Ошибка обновления"); }
  }

  const displayedMods = useMemo(() => {
    const safeMods = Array.isArray((contentByFolder as Record<string, any[]>)[contentTab]) ? (contentByFolder as Record<string, any[]>)[contentTab] : [];
    return safeMods.filter(m => {
      if (!m) return false;
      if (modFilter === "enabled" && !m.enabled) return false;
      if (modFilter === "disabled" && m.enabled) return false;
      if (modSearch) {
        const s = modSearch.toLowerCase();
        const t = (m.title || "").toLowerCase();
        const c = (m.clean_name || "").toLowerCase();
        if (!t.includes(s) && !c.includes(s)) return false;
      }
      return true;
    });
  },[contentByFolder, contentTab, modFilter, modSearch]);

  const displayedInstances = useMemo(() => {
    if (globalFilter === "all") return instances;
    return instances.filter(i => i.loader === globalFilter);
  }, [instances, globalFilter]);

  if (selectedInstance) {
    const isRunning = runningInstances.includes(selectedInstance.id);
    const isBusy = busyInstanceId === selectedInstance.id;
    const busyPercent = progress.total > 0 ? Math.round((progress.downloaded / progress.total) * 100) : 0;
    return (
      <div className="flex flex-col w-full max-w-6xl mx-auto h-full animate-in fade-in slide-in-from-right-8 duration-300">
        
        <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-3 mb-4 bg-jm-card p-3 md:p-4 rounded-2xl border border-white/10 shadow-xl shrink-0">
          <div className="flex items-center gap-3">
            <button onClick={() => setSelectedInstance(null)} className="p-2 bg-black/50 hover:bg-jm-accent hover:text-black text-white rounded-lg transition-colors shrink-0">
              <ArrowLeft size={18} />
            </button>
            <div className="w-12 h-12 rounded-xl overflow-hidden shrink-0 bg-black/50 flex items-center justify-center border border-white/20">
              {(() => { const iconSrc = selectedInstance.icon || packSourceInfo?.icon_url; return iconSrc ? <><img src={iconSrc.startsWith("http") ? iconSrc : convertFileSrc(iconSrc)} alt="" className="w-full h-full object-cover" onError={e => { (e.target as HTMLImageElement).style.display = "none"; (e.target as HTMLImageElement).nextElementSibling?.classList.remove("hidden"); }} /><span className="hidden w-full h-full flex items-center justify-center text-sm font-semibold text-white/70">{selectedInstance.name?.charAt(0)?.toUpperCase() || "?"}</span></> : <span className="w-full h-full flex items-center justify-center text-sm font-semibold text-white/70">{selectedInstance.name?.charAt(0)?.toUpperCase() || "?"}</span>; })()}
            </div>
            <div className="min-w-0">
              <h2 className="text-lg md:text-xl font-bold text-white truncate">{selectedInstance.name}</h2>
              <div className="flex gap-1.5 text-xs">
                <span className="bg-white/10 px-2 py-0.5 rounded-md text-[var(--text-secondary)] capitalize">{selectedInstance.loader}</span>
                <span className="bg-white/10 px-2 py-0.5 rounded-md text-[var(--text-secondary)]">{selectedInstance.game_version}</span>
                {selectedInstance.loader_version && <span className="bg-jm-accent/20 text-jm-accent-light px-2 py-0.5 rounded-md border border-jm-accent/30">{selectedInstance.loader_version}</span>}
              </div>
            </div>
          </div>
          
          <div className="flex gap-2 flex-wrap">
            <button onClick={checkPackUpdate} disabled={isCheckingPackUpdate} className="bg-white/10 hover:bg-white/20 text-white px-3 py-2 rounded-lg font-bold text-xs transition-colors flex items-center gap-1.5 disabled:opacity-50" title="Проверить обновления сборки">
              <RefreshCw size={14} className={isCheckingPackUpdate ? "animate-spin" : ""} /> <span className="hidden sm:inline">Обновления</span>
            </button>
            {packUpdateInfo?.has_update && packUpdateInfo?.update_url && (
              <button onClick={applyPackUpdate} disabled={isUpdatingPack} className="bg-blue-500 hover:bg-blue-400 text-white px-3 py-2 rounded-lg font-bold text-xs transition-colors flex items-center gap-1.5 disabled:opacity-50">
                {isUpdatingPack ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />} Обновить
              </button>
            )}
            <button onClick={() => setShowExportModal(true)} className="bg-white/10 hover:bg-white/20 text-white px-3 py-2 rounded-lg font-bold text-xs transition-colors flex items-center gap-1.5"><Archive size={14}/> <span className="hidden sm:inline">Экспорт</span></button>
            <button onClick={() => launchInstance(selectedInstance)} disabled={isLaunching || isRepairing || isBusy} className={`${isRunning ? 'bg-red-500 hover:bg-red-600 text-white' : 'bg-jm-accent hover:bg-jm-accent-light text-black'} px-5 py-2 rounded-lg font-bold text-xs transition-transform hover:scale-105 shadow-lg flex items-center gap-1.5 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100`}>
              {isBusy ? <><Loader2 size={14} className="animate-spin" /> {busyPercent}%</> : isLaunching || isRepairing ? <><Loader2 size={14} className="animate-spin" /> ...</> : isRunning ? <><Square size={14} fill="currentColor" /> СТОП</> : <><Play size={14} fill="currentColor" /> ИГРАТЬ</>}
            </button>
            <button onClick={() => invoke("open_folder", { id: selectedInstance.id })} className="w-9 h-9 bg-white/10 hover:bg-white/20 rounded-lg flex items-center justify-center transition-colors shrink-0" title="Открыть папку сборки">
              <FolderOpen size={14} className="text-white" />
            </button>
          </div>
        </div>

        <div className="flex flex-col lg:flex-row gap-3 h-full min-h-0">
          <div className="lg:w-48 shrink-0 flex lg:flex-col gap-1.5 overflow-x-auto lg:overflow-x-visible [&::-webkit-scrollbar]:hidden">
            {[{ id: "content", label: "Контент", icon: <Puzzle size={16} /> }, { id: "worlds", label: "Миры", icon: <Globe size={16} /> }, { id: "logs", label: "Логи", icon: <Terminal size={16} /> }, { id: "options", label: "Настройки", icon: <Settings size={16} /> }].map(tab => (
              <button key={tab.id} onClick={() => { setInstanceTab(tab.id); if (tab.id === "worlds" && selectedInstance) { invoke("list_worlds", { instanceId: selectedInstance.id }).then((w: any) => setWorldsList(w || [])); invoke("load_servers", { instanceId: selectedInstance.id }).then((s: any) => setRecentServers(Array.isArray(s) ? s : [])); invoke("get_last_world", { instanceId: selectedInstance.id }).then((lw: any) => setLastWorldGlobal(lw && lw.instance_id ? lw : null)).catch(() => setLastWorldGlobal(null)); } }} className={`flex items-center gap-2 px-3 py-2.5 rounded-xl text-sm font-bold transition-all whitespace-nowrap shrink-0 ${instanceTab === tab.id ? 'bg-jm-accent text-black shadow-md' : 'bg-jm-card text-[var(--text-secondary)] hover:text-white border border-white/5 hover:border-white/20'}`}>
                {tab.icon} {tab.label}
              </button>
            ))}
            {confirmDeleteId === selectedInstance.id ? (
              <div className="lg:mt-auto flex flex-col gap-1.5 p-3 rounded-xl bg-red-500/10 border border-red-500/30">
                <p className="text-xs text-red-400 font-bold text-center">Удалить?</p>
                <div className="flex gap-1.5">
                  <button onClick={() => handleDelete(selectedInstance.id)} className="flex-1 py-1.5 rounded-lg font-bold text-white bg-red-500 hover:bg-red-600 transition-colors text-xs">Да</button>
                  <button onClick={() => setConfirmDeleteId(null)} className="flex-1 py-1.5 rounded-lg font-bold text-[var(--text-secondary)] bg-white/10 hover:bg-white/20 transition-colors text-xs">Нет</button>
                </div>
              </div>
            ) : (
              <button onClick={() => setConfirmDeleteId(selectedInstance.id)} className="lg:mt-auto flex items-center gap-2 px-3 py-2.5 rounded-xl text-sm font-bold text-red-500 bg-red-500/10 hover:bg-red-500 hover:text-white transition-colors border border-red-500/20 whitespace-nowrap shrink-0">
                <Trash2 size={16} /> Удалить
              </button>
            )}
          </div>

          <div className="flex-grow bg-jm-card rounded-2xl border border-white/10 shadow-xl p-3 md:p-5 overflow-hidden flex flex-col relative min-h-0">
            {instanceTab === "content" && (
              <div className="flex flex-col h-full">
                <div className="flex justify-between items-center mb-4">
                  <div className="flex gap-2 bg-black/30 p-1 rounded-xl border border-white/5">
                    {[{ id: "mods", label: "Моды" }, { id: "resourcepacks", label: "Ресурспаки" }, { id: "shaderpacks", label: "Шейдеры" }].map(t => (
                      <button key={t.id} onClick={() => { setContentTab(t.id); loadContent(t.id as "mods" | "resourcepacks" | "shaderpacks"); }} className={`px-4 py-2 rounded-lg text-sm font-bold transition-all ${contentTab === t.id ? 'bg-jm-accent text-black shadow-md' : 'text-[var(--text-secondary)] hover:text-white'}`}>{t.label}</button>
                    ))}
                  </div>
                  <button onClick={() => setShowModBrowser(true)} className="bg-jm-accent/20 hover:bg-jm-accent text-jm-accent hover:text-black px-4 py-2 rounded-xl text-sm font-bold flex items-center gap-2 transition-colors border border-jm-accent/30 hover:border-jm-accent">
                    <Plus size={16} /> Добавить
                  </button>
                </div>
                <div className="flex justify-between items-center mb-4">
                  <div className="flex gap-3">
                    <div className="relative">
                      <Search className="absolute left-3 top-2.5 text-[var(--text-secondary)]" size={16} />
                      <input type="text" placeholder="Поиск..." value={modSearch} onChange={e => setModSearch(e.target.value)} className="bg-black/50 border border-white/10 rounded-xl pl-9 pr-4 py-2 text-white outline-none w-44 text-sm focus:border-jm-accent transition-colors" />
                    </div>
                    <div className="w-40">
                      <CustomSelect 
                        value={modFilter} onChange={setModFilter} 
                        options={[{ value: "all", label: "Все" }, { value: "enabled", label: "Включенные" }, { value: "disabled", label: "Отключенные" }]} 
                      />
                    </div>
                  </div>
                  <div className="flex gap-2">
                    {contentTab === "mods" && (
                      <button onClick={checkForUpdates} disabled={isCheckingUpdates} className="bg-white/5 hover:bg-white/10 text-white px-3 py-2 rounded-xl text-xs font-bold flex items-center gap-1.5 transition-colors border border-white/5 disabled:opacity-50">
                        <RefreshCw size={14} className={isCheckingUpdates ? "animate-spin" : ""} /> Обновления
                      </button>
                    )}
                    <button onClick={async () => { showToast("Загрузка метаданных..."); try { await invoke("refresh_mod_metadata", { instanceId: selectedInstance.id }); loadMods(); showToast("Метаданные обновлены!"); } catch (e) { showToast(`Ошибка: ${e}`); } }} className="bg-white/5 hover:bg-white/10 text-white px-3 py-2 rounded-xl text-xs font-bold flex items-center gap-1.5 transition-colors border border-white/5" title="Обновить иконки и названия">
                      <RefreshCw size={14} /> Мета
                    </button>
                  </div>
                </div>

                <div className="flex-grow overflow-y-auto custom-scrollbar pr-2 flex flex-col gap-2">
                  {displayedMods.length === 0 ? (
                    <div className="flex flex-col items-center justify-center h-full text-[var(--text-secondary)]"><Puzzle size={64} className="mb-4 opacity-20" /><p className="text-xl font-bold text-white mb-2">Нет контента</p><p className="text-sm">Нажмите «Добавить», чтобы открыть браузер.</p></div>
                  ) : (
                    displayedMods.map(m => (
                      <div key={m.filename} className={`flex items-center p-3 rounded-xl border transition-colors ${m.enabled ? 'bg-black/30 border-white/5 hover:border-white/10' : 'bg-black/50 border-red-500/20 opacity-60'}`}>
                        <div className="mr-4"><ToggleSwitch checked={m.enabled} onChange={(c) => toggleMod(m.filename, c)} /></div>
                        
                        <div className="cursor-pointer hover:opacity-80 transition-opacity">
                          {m.icon_url ? <img src={m.icon_url} className="w-10 h-10 rounded-lg object-cover bg-black/50 shrink-0" /> : <div className="w-10 h-10 rounded-lg bg-black/50 flex items-center justify-center shrink-0"><Puzzle size={20} className="text-[var(--text-secondary)]"/></div>}
                        </div>
                        
                        <div className="flex-1 ml-4 min-w-0 grid grid-cols-2 items-center" onClick={() => { if (m.project_id) { setOpenModProjectId(m.project_id); setShowModBrowser(true); } }}>
                          <h4 className={`font-bold text-white truncate text-base transition-colors ${m.project_id ? 'cursor-pointer hover:text-jm-accent-light' : ''}`}>{m.title || m.clean_name}</h4>
                          <p className="text-sm text-[var(--text-secondary)] truncate">{m.version_name}</p>
                        </div>

                        <div className="flex items-center gap-2 ml-4 shrink-0">
                          {updates[m.hash] && <button onClick={() => updateMod(m.filename, m.hash)} className="bg-blue-500 hover:bg-blue-400 text-white text-xs px-4 py-2 rounded-lg font-bold transition-colors shadow-md">Обновить</button>}
                          {confirmDeleteMod === m.filename ? (
                            <div className="flex items-center gap-1">
                              <button onClick={() => deleteMod(m.filename)} className="bg-red-500 hover:bg-red-600 text-white text-xs px-3 py-2 rounded-lg font-bold transition-colors">Да</button>
                              <button onClick={() => setConfirmDeleteMod(null)} className="bg-white/10 hover:bg-white/20 text-[var(--text-secondary)] text-xs px-3 py-2 rounded-lg font-bold transition-colors">Нет</button>
                            </div>
                          ) : (
                            <button onClick={() => setConfirmDeleteMod(m.filename)} className="bg-white/5 hover:bg-red-500/20 text-[var(--text-secondary)] hover:text-red-500 transition-colors p-2 rounded-lg"><Trash2 size={18}/></button>
                          )}
                        </div>
                      </div>
                    ))
                  )}
                </div>
              </div>
            )}

            {instanceTab === "worlds" && (
              <div className="flex flex-col h-full gap-6">
                {lastWorldGlobal && (
                  <div>
                    <h3 className="text-lg font-bold text-white mb-3 flex items-center gap-2"><Globe size={20} /> Последний мир</h3>
                    <div className="p-3 rounded-xl bg-jm-accent/10 border border-jm-accent/30 text-white">
                      <div className="font-bold truncate">{lastWorldGlobal.world_name}</div>
                      <div className="text-xs text-[var(--text-secondary)] truncate">Сборка: {lastWorldGlobal.instance_name}</div>
                    </div>
                  </div>
                )}
                <div>
                  <h3 className="text-lg font-bold text-white mb-3 flex items-center gap-2"><Globe size={20} /> Миры</h3>
                  <div className="flex flex-col gap-2 max-h-48 overflow-y-auto custom-scrollbar">
                    {worldsList.length === 0 ? (
                      <p className="text-[var(--text-secondary)] py-4">Нет сохранённых миров</p>
                    ) : (
                      worldsList.map((w) => (
                        <button key={w} onClick={() => launchInstance(selectedInstance, undefined, w)} disabled={isLaunching || isRepairing || isBusy} className="flex items-center gap-4 p-3 rounded-xl bg-black/30 border border-white/5 hover:border-jm-accent/50 hover:bg-jm-accent/10 text-left transition-colors disabled:opacity-50">
                          <Globe size={20} className="text-jm-accent shrink-0" />
                          <span className="font-bold text-white truncate">{w}</span>
                          <Play size={16} className="text-jm-accent ml-auto shrink-0" />
                        </button>
                      ))
                    )}
                  </div>
                </div>
                <div>
                  <h3 className="text-lg font-bold text-white mb-3 flex items-center gap-2"><Server size={20} /> Серверы</h3>
                  <div className="flex flex-col gap-2 max-h-48 overflow-y-auto custom-scrollbar">
                    {recentServers.length === 0 ? (
                      <p className="text-[var(--text-secondary)] py-4">Нет сохранённых серверов</p>
                    ) : (
                      recentServers.map((srv: any, i: number) => (
                        <button key={i} onClick={() => launchInstance(selectedInstance, srv.ip)} disabled={isLaunching || isRepairing || isBusy} className="flex items-center gap-4 p-3 rounded-xl bg-black/30 border border-white/5 hover:border-jm-accent/50 hover:bg-jm-accent/10 text-left transition-colors disabled:opacity-50">
                          <Server size={20} className="text-jm-accent shrink-0" />
                          <div className="min-w-0 flex-1">
                            <div className="font-bold text-white truncate">{srv.name || srv.ip}</div>
                            <div className="text-xs text-[var(--text-secondary)] truncate">{srv.ip}</div>
                          </div>
                          <Play size={16} className="text-jm-accent ml-auto shrink-0" />
                        </button>
                      ))
                    )}
                  </div>
                </div>
              </div>
            )}
            
            {instanceTab === "logs" && (
              <div className="h-full flex flex-col">
                <div className="flex justify-between items-center mb-4"><h3 className="text-xl font-bold text-white">Консоль игры</h3><button onClick={() => setLogs([])} className="text-sm bg-white/10 hover:bg-white/20 px-4 py-2 rounded-lg transition-colors">Очистить</button></div>
                <div className="flex-grow bg-[var(--input-bg)] rounded-xl border border-white/10 p-4 font-mono text-xs text-[var(--text-secondary)] overflow-y-auto custom-scrollbar leading-relaxed">
                  {logs.map((l, i) => <div key={i} className={l.includes("[ERROR]") ? "text-red-400" : ""}>{l}</div>)}
                  <div ref={logsEndRef} />
                </div>
              </div>
            )}

            {instanceTab === "options" && (
              <div className="flex flex-col h-full">
                <div className="flex gap-4 mb-6 border-b border-white/10 pb-4">
                  <button onClick={() => setSettingsSubTab("general")} className={`font-bold transition-colors ${settingsSubTab === "general" ? "text-jm-accent" : "text-[var(--text-secondary)] hover:text-white"}`}>Общие</button>
                  {packSourceInfo && (
                    <button onClick={() => setSettingsSubTab("pack")} className={`font-bold transition-colors ${settingsSubTab === "pack" ? "text-jm-accent" : "text-[var(--text-secondary)] hover:text-white"}`}>Сборка</button>
                  )}
                  <button onClick={() => {
                  setSettingsSubTab("core");
                  if (coreVersions.length === 0) {
                    invoke("fetch_vanilla_versions").then((m: any) => {
                      const vers = (m?.versions || []).map((v: any) => v.id).filter(Boolean);
                      setCoreVersions(vers);
                      if (vers.length > 0 && !coreGameVer) setCoreGameVer(selectedInstance.game_version);
                    });
                    const l = coreLoader || selectedInstance.loader;
                    if (l !== "vanilla") {
                      invoke("get_loader_versions", { loader: l }).then((vers: any) => setCoreVersions(vers || []));
                      invoke("get_specific_loader_versions", { loader: l, gameVersion: coreGameVer || selectedInstance.game_version }).then((vers: any) => setCoreLoaderVersions(vers || []));
                    }
                  }
                }} className={`font-bold transition-colors ${settingsSubTab === "core" ? "text-jm-accent" : "text-[var(--text-secondary)] hover:text-white"}`}>Ядро (Core)</button>
                </div>

                {settingsSubTab === "general" && (
                  <div className="space-y-6">
                    <label className="flex items-center gap-3 cursor-pointer p-4 bg-black/30 rounded-xl border border-white/5">
                      <input type="checkbox" checked={instSettings.override_global} onChange={e => saveInstSettings({...instSettings, override_global: e.target.checked})} className="w-5 h-5 accent-jm-accent cursor-pointer" />
                      <span className="text-white font-bold">Использовать персональные настройки (ОЗУ, Java)</span>
                    </label>
                    <div className={`space-y-6 transition-opacity ${instSettings.override_global ? 'opacity-100' : 'opacity-30 pointer-events-none'}`}>
                      <div>
                        <label className="text-sm text-[var(--text-secondary)] mb-2 block">Выделение ОЗУ: <strong className="text-jm-accent">{instSettings.ram_mb} MB</strong></label>
                        <input type="range" min="1024" max={maxRam} step="512" value={instSettings.ram_mb} onChange={e => saveInstSettings({...instSettings, ram_mb: parseInt(e.target.value)})} className="w-full accent-jm-accent cursor-pointer" />
                      </div>
                      <div>
                        <label className="text-sm text-[var(--text-secondary)] mb-2 block">Аргументы JVM</label>
                        <input type="text" value={instSettings.jvm_args} onChange={e => saveInstSettings({...instSettings, jvm_args: e.target.value})} className="w-full bg-black/50 border border-white/10 rounded-xl px-4 py-3 text-white outline-none focus:border-jm-accent font-mono text-sm transition-colors" />
                      </div>
                    </div>
                    <label className="flex items-center gap-3 cursor-pointer p-4 bg-black/30 rounded-xl border border-white/5">
                      <input type="checkbox" checked={instSettings.use_discrete_gpu ?? false} onChange={e => saveInstSettings({...instSettings, use_discrete_gpu: e.target.checked})} className="w-5 h-5 accent-jm-accent cursor-pointer" />
                      <span className="text-white font-bold">Использовать дискретную видеокарту</span>
                    </label>
                    <p className="text-xs text-[var(--text-secondary)] -mt-2">Linux: __NV_PRIME_RENDER_OFFLOAD. Windows: настройте GPU в параметрах системы.</p>
                  </div>
                )}

                {settingsSubTab === "pack" && packSourceInfo && (
                  <div className="space-y-6">
                    <div className="p-4 bg-black/30 rounded-2xl border border-white/5">
                      <h4 className="text-sm font-bold text-[var(--text-secondary)] mb-3">Название сборки</h4>
                      <input value={renameValue} onChange={e => setRenameValue(e.target.value)} className="w-full bg-black/50 border border-white/10 rounded-xl px-4 py-3 text-white outline-none focus:border-jm-accent" placeholder="Название" />
                      <button onClick={async () => { try { await invoke("rename_instance", { id: selectedInstance.id, new_name: renameValue.trim() }); showToast("Название изменено"); const insts: any = await invoke("get_instances"); const u = (insts || []).find((i: any) => i.id === selectedInstance.id); if (u) setSelectedInstance(u); loadData(); } catch (e) { showToast(`Ошибка: ${e}`); } }} disabled={isRunning} className="mt-2 bg-jm-accent/20 hover:bg-jm-accent text-jm-accent hover:text-black px-4 py-2 rounded-xl font-bold text-sm disabled:opacity-50">Применить</button>
                    </div>
                    {packSourceInfo.source === "modrinth" && packVersions.length > 0 && (
                      <div className="p-4 bg-black/30 rounded-2xl border border-white/5">
                        <h4 className="text-sm font-bold text-[var(--text-secondary)] mb-3">Версия сборки</h4>
                        <div className="flex gap-2 flex-wrap">
                          <select value={selectedPackVersion || packSourceInfo.version_id} onChange={e => setSelectedPackVersion(e.target.value)} className="flex-1 min-w-[200px] bg-black/50 border border-white/10 rounded-xl px-4 py-2 text-white">
                            {packVersions.map((v: any) => (
                              <option key={v.id} value={v.id}>{v.name || v.id}</option>
                            ))}
                          </select>
                          <button onClick={async () => {
                            const ver = selectedPackVersion || packSourceInfo.version_id;
                            const vObj = packVersions.find((v: any) => v.id === ver);
                            const url = vObj?.files?.find((f: any) => f.filename?.endsWith?.(".mrpack"))?.url;
                            if (!url) { showToast("Нет .mrpack для этой версии"); return; }
                            try { setIsUpdatingPack(true); const res = await invoke("update_modpack", { instance_id: selectedInstance.id, update_url: url }); showToast(res as string); loadData(); const insts: any = await invoke("get_instances"); const u = (insts || []).find((i: any) => i.id === selectedInstance.id); if (u) setSelectedInstance(u); setPackSourceInfo(null); invoke("get_pack_source_info", { instance_id: selectedInstance.id }).then((info: any) => setPackSourceInfo(info)); } catch (e) { showToast(`Ошибка: ${e}`); } finally { setIsUpdatingPack(false); } }} disabled={isRunning || isUpdatingPack} className="bg-jm-accent/20 hover:bg-jm-accent text-jm-accent hover:text-black px-4 py-2 rounded-xl font-bold text-sm disabled:opacity-50">{isUpdatingPack ? "..." : "Обновить до версии"}</button>
                        </div>
                      </div>
                    )}
                    <div className="p-4 bg-red-500/10 rounded-2xl border border-red-500/30">
                      <h4 className="text-sm font-bold text-red-400 mb-2">Отвязать от модпака</h4>
                      <p className="text-xs text-[var(--text-secondary)] mb-3">Сборка перестанет показывать обновления. Файлы не удаляются.</p>
                      <button onClick={async () => { try { await invoke("unlink_modpack", { id: selectedInstance.id }); showToast("Сборка отвязана"); setPackSourceInfo(null); setSettingsSubTab("general"); } catch (e) { showToast(`Ошибка: ${e}`); } }} disabled={isRunning} className="bg-red-500/20 hover:bg-red-500 text-red-400 hover:text-white px-4 py-2 rounded-xl font-bold text-sm disabled:opacity-50">Отвязать</button>
                    </div>
                  </div>
                )}

                {settingsSubTab === "core" && (
                  <div className="space-y-6">
                    <div className="flex items-center gap-5 p-5 bg-black/30 rounded-2xl border border-white/5">
                      <div className="w-16 h-16 rounded-2xl bg-black/50 border border-white/20 flex items-center justify-center shrink-0 text-white/70">
                        <LoaderIcon loader={selectedInstance.loader} size={32} />
                      </div>
                      <div>
                        <h4 className="text-lg font-bold text-white capitalize">{selectedInstance.loader === "vanilla" ? "Vanilla" : selectedInstance.loader}</h4>
                        <div className="flex gap-3 mt-1">
                          <span className="text-sm text-[var(--text-secondary)]">Minecraft <strong className="text-white">{selectedInstance.game_version}</strong></span>
                          {selectedInstance.loader_version && <span className="text-sm text-[var(--text-secondary)]">Ядро <strong className="text-jm-accent">{selectedInstance.loader_version}</strong></span>}
                        </div>
                      </div>
                    </div>
                    <div className="space-y-4 p-4 bg-black/20 rounded-2xl border border-white/5">
                      <h4 className="text-sm font-bold text-[var(--text-secondary)] mb-2">Сменить ядро</h4>
                      <div className="grid grid-cols-3 gap-3">
                        <CustomSelect label="Загрузчик" value={coreLoader || selectedInstance.loader} onChange={(v: string) => { setCoreLoader(v); setCoreLoaderVer(""); invoke("get_loader_versions", { loader: v }).then((vers: any) => setCoreVersions(vers || [])); }} options={[{ value: "vanilla", label: "Vanilla" }, { value: "fabric", label: "Fabric" }, { value: "quilt", label: "Quilt" }, { value: "forge", label: "Forge" }, { value: "neoforge", label: "NeoForge" }]} />
                        <CustomSelect label="Версия игры" value={coreGameVer || selectedInstance.game_version} onChange={(v: string) => { setCoreGameVer(v); const l = coreLoader || selectedInstance.loader; if (l !== "vanilla") invoke("get_specific_loader_versions", { loader: l, gameVersion: v }).then((vers: any) => setCoreLoaderVersions(vers || [])); }} options={coreVersions.length > 0 ? coreVersions.map(v => ({ value: v, label: v })) : [{ value: selectedInstance.game_version, label: selectedInstance.game_version }]} />
                        {(coreLoader || selectedInstance.loader) !== "vanilla" && (
                          <CustomSelect label="Версия ядра" value={coreLoaderVer || selectedInstance.loader_version || ""} onChange={setCoreLoaderVer} options={coreLoaderVersions.length > 0 ? coreLoaderVersions.map(v => ({ value: v, label: v })) : selectedInstance.loader_version ? [{ value: selectedInstance.loader_version, label: selectedInstance.loader_version }] : [{ value: "", label: "Нет" }]} />
                        )}
                      </div>
                      <button onClick={async () => {
                        const l = coreLoader || selectedInstance.loader;
                        const gv = coreGameVer || selectedInstance.game_version;
                        const lv = coreLoaderVer || selectedInstance.loader_version || "";
                        try {
                          await invoke("update_instance_core", { id: selectedInstance.id, gameVersion: gv, loader: l, loaderVersion: lv });
                          showToast("Ядро обновлено! Перезапустите игру.");
                          const insts: any = await invoke("get_instances");
                          const updated = (insts || []).find((i: any) => i.id === selectedInstance.id);
                          if (updated) setSelectedInstance(updated);
                          loadData();
                        } catch (e) { showToast(`Ошибка: ${e}`); }
                      }} disabled={isRunning} className="w-full bg-jm-accent/20 hover:bg-jm-accent text-jm-accent hover:text-black py-2.5 rounded-xl font-bold transition-colors border border-jm-accent/30 disabled:opacity-50 text-sm">
                        Применить
                      </button>
                    </div>
                    <div className="flex gap-4 mt-4">
                      <button 
                        onClick={async () => {
                          try {
                            setIsRepairing(true);
                            showToast("Очистка ядра...");
                            const res = await invoke("repair_core", { id: selectedInstance.id });
                            showToast(res as string);
                          } catch (e) {
                            showToast(`Ошибка: ${e}`);
                          } finally {
                            setIsRepairing(false);
                          }
                        }} 
                        disabled={isRunning} 
                        className="bg-red-500/10 hover:bg-red-500 text-red-400 hover:text-white px-6 py-3 rounded-xl font-bold flex items-center gap-2 transition-colors border border-red-500/30 disabled:opacity-50 text-sm"
                      >
                        <RefreshCw size={18} /> Починить ядро
                      </button>
                    </div>
                  </div>
                )}
              </div>
            )}

            <AnimatePresence>
              {showModBrowser && (
                <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="fixed inset-0 z-[100] flex">
                  <div className="hidden sm:block sm:w-[5%] md:w-[10%] lg:w-[20%] bg-black/60 backdrop-blur-sm shrink-0" onClick={() => { setShowModBrowser(false); setOpenModProjectId(undefined); loadMods(); }} />
                  <motion.div initial={{ x: "100%" }} animate={{ x: 0 }} exit={{ x: "100%" }} transition={{ type: "spring", damping: 30, stiffness: 300 }} className="w-full sm:w-[95%] md:w-[90%] lg:w-[80%] h-full bg-jm-bg border-l border-white/10 shadow-2xl flex flex-col">
                    <div className="flex justify-between items-center p-3 bg-jm-card border-b border-white/10 shrink-0">
                      <h3 className="font-bold text-white text-sm md:text-base truncate">Добавление в {selectedInstance.name}</h3>
                      <button onClick={() => { setShowModBrowser(false); setOpenModProjectId(undefined); loadMods(); }} className="text-[var(--text-secondary)] hover:text-white p-2 bg-black/50 rounded-xl transition-colors"><X size={20}/></button>
                    </div>
                    <div className="flex-grow overflow-hidden relative">
                      <SafeBoundary>
                        <DiscoverTab contextInstance={selectedInstance} installedMods={(contentByFolder.mods || []).map((m: any) => (m?.clean_name || "").replace(/\.(jar|zip)$/i, ""))} onModsChanged={loadMods} initialProjectId={openModProjectId} />
                      </SafeBoundary>
                    </div>
                  </motion.div>
                </motion.div>
              )}
            </AnimatePresence>

            <AnimatePresence>
              {showExportModal && (
                <ExportModal
                  instanceId={selectedInstance.id}
                  instanceName={selectedInstance.name}
                  onClose={() => setShowExportModal(false)}
                  showToast={showToast}
                />
              )}
            </AnimatePresence>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col w-full max-w-6xl mx-auto h-full">
      <div className="flex justify-between items-center mb-4 flex-wrap gap-3">
        <h2 className="text-xl md:text-2xl font-bold text-jm-accent-light">Ваши сборки</h2>
        <div className="flex items-center gap-2 flex-wrap">
          <div className="w-36">
            <CustomSelect label="" value={globalFilter} onChange={setGlobalFilter} options={[{ value: "all", label: "Все" }, { value: "vanilla", label: "Vanilla" }, { value: "fabric", label: "Fabric" }, { value: "forge", label: "Forge" }, { value: "neoforge", label: "NeoForge" }, { value: "quilt", label: "Quilt" }]} />
          </div>
          <ImportDropdown onImported={loadData} />
          <button onClick={() => setIsCreating(true)} className="bg-jm-accent hover:bg-jm-accent-light text-black px-3 py-2 rounded-lg font-bold text-xs flex items-center gap-1.5 transition-colors"><Plus size={14}/> Создать</button>
        </div>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 overflow-y-auto custom-scrollbar pb-6">
        {displayedInstances.map((inst, idx) => {
          const isRunning = runningInstances.includes(inst.id);
          return (
            <motion.div key={inst.id} initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: idx * 0.05, duration: 0.3 }} whileHover={{ y: -3, boxShadow: "0 10px 30px rgba(134,168,134,0.15)" }} onClick={() => setSelectedInstance(inst)} className="bg-jm-card border border-white/10 rounded-xl p-3 flex flex-col hover:border-jm-accent transition-all cursor-pointer group relative overflow-hidden">
              <div className="flex items-center gap-3 mb-3 relative z-10">
                {inst.icon ? (
                  <img src={convertFileSrc(inst.icon)} alt="" className="w-10 h-10 rounded-lg object-cover shadow-inner border border-white/10" onError={e => { (e.target as HTMLImageElement).style.display = "none"; (e.target as HTMLImageElement).nextElementSibling?.classList.remove("hidden"); }} />
                ) : null}
                <div className={`w-10 h-10 rounded-lg bg-black/50 border border-white/20 flex items-center justify-center text-sm font-medium text-white/70 shrink-0 ${inst.icon ? "hidden" : ""}`}>{inst.name?.charAt(0)?.toUpperCase() || "?"}</div>
                <div className="min-w-0"><h3 className="text-sm font-bold text-white truncate">{inst.name}</h3><div className="flex gap-1.5"><span className="text-[10px] bg-white/10 px-1.5 py-0.5 rounded text-[var(--text-secondary)] capitalize">{inst.loader}</span><span className="text-[10px] bg-white/10 px-1.5 py-0.5 rounded text-[var(--text-secondary)]">{inst.game_version}</span></div></div>
              </div>
              <div className="mt-auto flex gap-1.5 relative z-10">
                <button onClick={(e) => { e.stopPropagation(); launchInstance(inst); }} disabled={isLaunching || isRepairing || busyInstanceId === inst.id} className={`flex-1 font-bold py-2 rounded-lg text-xs flex items-center justify-center gap-1.5 transition-colors border disabled:opacity-50 disabled:cursor-not-allowed ${isRunning ? 'bg-red-500/20 text-red-500 border-red-500/30 hover:bg-red-500 hover:text-white' : 'bg-jm-accent/10 text-jm-accent border-jm-accent/30 hover:bg-jm-accent hover:text-black'}`}>
                  {busyInstanceId === inst.id ? <><Loader2 size={14} className="animate-spin" /> ...</> : isRunning ? <><Square size={14} fill="currentColor" /> СТОП</> : <><Play size={14} fill="currentColor" /> ИГРАТЬ</>}
                </button>
                <button className="w-9 bg-white/5 hover:bg-white/10 text-[var(--text-secondary)] hover:text-white rounded-lg flex items-center justify-center transition-colors border border-white/10">
                  <Wrench size={14} />
                </button>
              </div>
            </motion.div>
          );
        })}
      </div>

      <AnimatePresence>
        {isCreating && (
          <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
            <motion.div initial={{ scale: 0.95, y: 20 }} animate={{ scale: 1, y: 0 }} exit={{ scale: 0.95, y: 20 }} className="bg-jm-card border border-white/10 p-8 rounded-3xl w-full max-w-md shadow-2xl">
              <h3 className="text-2xl font-bold text-white mb-6">Новая сборка</h3>
              <div className="space-y-4 mb-8">
                <div className="flex items-center gap-4">
                  <button
                    onClick={async () => { try { const p: any = await invoke("pick_image_file"); if (p) setNewIcon(p); } catch {} }}
                    className="w-16 h-16 shrink-0 rounded-xl border-2 border-dashed border-white/20 hover:border-jm-accent/50 flex items-center justify-center bg-black/30 overflow-hidden transition-colors"
                    title="Выбрать иконку"
                  >
                    {newIcon ? (
                      <img src={convertFileSrc(newIcon)} className="w-full h-full object-cover" />
                    ) : (
                      <Plus size={20} className="text-[var(--text-secondary)]" />
                    )}
                  </button>
                  <div className="flex-1"><label className="text-sm text-[var(--text-secondary)] mb-1 block">Название</label><input type="text" placeholder="Например: Выживание с модами" value={newName} onChange={e => setNewName(e.target.value)} className="w-full bg-black/50 border border-white/10 rounded-xl px-4 py-3 text-white outline-none focus:border-jm-accent transition-colors" /></div>
                </div>
                <CustomSelect label="Ядро (Загрузчик)" value={newLoader} onChange={setNewLoader} options={[{ value: "vanilla", label: "Vanilla" }, { value: "fabric", label: "Fabric" }, { value: "quilt", label: "Quilt" }, { value: "forge", label: "Forge" }, { value: "neoforge", label: "NeoForge" }]} />
                <div className="flex gap-4">
                  <div className="flex-1">
                    {isLoadingVersions ? <div className="h-[72px] flex items-end"><div className="w-full bg-black/50 border border-white/10 rounded-xl px-4 py-3 text-[var(--text-secondary)] flex items-center gap-2"><Loader2 className="animate-spin" size={16}/> Загрузка...</div></div> : <CustomSelect label="Версия игры" value={newVersion} onChange={setNewVersion} options={availableVersions.map(v => ({ value: v, label: v }))} />}
                  </div>
                  {newLoader !== "vanilla" && <div className="flex-1">
                    {isLoadingLoaderVersions ? <div className="h-[72px] flex items-end"><div className="w-full bg-black/50 border border-white/10 rounded-xl px-4 py-3 text-[var(--text-secondary)] flex items-center gap-2"><Loader2 className="animate-spin" size={16}/> Загрузка...</div></div> : <CustomSelect label="Версия ядра" value={newLoaderVersion} onChange={setNewLoaderVersion} options={availableLoaderVersions.map(v => ({ value: v, label: v }))} disabled={availableLoaderVersions.length === 0}/>}
                  </div>}
                </div>
              </div>
              <div className="flex gap-4"><button onClick={() => setIsCreating(false)} className="flex-1 py-3 rounded-xl font-bold bg-white/5 hover:bg-white/10 text-white transition-colors">Отмена</button><button onClick={handleCreate} disabled={!newName || isLoadingVersions || isLoadingLoaderVersions} className="flex-1 py-3 rounded-xl font-bold bg-jm-accent text-black hover:bg-jm-accent-light transition-colors disabled:opacity-50">Создать</button></div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}