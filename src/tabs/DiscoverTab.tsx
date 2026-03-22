import { useState, useEffect, useRef } from "react";
import { Search, Download, ChevronLeft, ChevronRight, Loader2, Package, Image as ImageIcon, Sparkles, Layers, Link, X, AlignLeft, List, CheckCircle2, RefreshCw, ArrowDownWideNarrow, ArrowUpWideNarrow } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import DOMPurify from "dompurify";

const showToast = (msg: string) => window.dispatchEvent(new CustomEvent("jm_toast", { detail: msg }));

/** Убирает битые <img> (с невалидным src), санитизирует HTML для безопасного вывода. */
function sanitizeProjectBody(body: string): string {
  if (!body || typeof body !== "string") return "";
  // Удаляем <img> с src не http(s) — битые/битые URL не ломают верстку
  let out = body.replace(/<img\s[^>]*>/gi, (tag) => {
    const m = tag.match(/src\s*=\s*["']([^"']*)["']/i);
    const src = m ? m[1].trim() : "";
    if (!/^https?:\/\//i.test(src)) return "";
    return tag;
  });
  out = DOMPurify.sanitize(out, {
    // Расширяем whitelist: Modrinth/CurseForge описания часто содержат code/pre/blockquote.
    ALLOWED_TAGS: ["p", "br", "ul", "ol", "li", "strong", "em", "b", "i", "a", "h1", "h2", "h3", "h4", "img", "div", "span", "sup", "code", "pre", "blockquote", "hr", "table", "thead", "tbody", "tr", "th", "td"],
    ALLOWED_ATTR: ["href", "src", "target", "rel", "alt"],
    ADD_ATTR: ["target"],
  });
  return out;
}

/** Определяет, что текст — HTML (теги), а не только Markdown. */
function looksLikeHtml(text: string): boolean {
  if (!text || typeof text !== "string") return false;
  return /<\s*[a-z][^>]*>/i.test(text);
}

const CATEGORY_MAP: Record<string, string[]> = {
  mod:["optimization", "magic", "technology", "adventure", "decoration", "worldgen", "storage", "combat", "utility"],
  modpack:["optimization", "adventure", "combat", "multiplayer", "quests", "technology", "vanilla-plus"],
  resourcepack:["16x", "32x", "64x", "128x", "realistic", "stylized", "gui", "animated"],
  shader: ["realistic", "fantasy", "performance", "vanilla-like"],
  datapack: ["worldgen", "utility", "adventure", "combat", "decoration"],
};

function CustomSelect({ label, value, options, onChange, disabled }: any) {
  const [isOpen, setIsOpen] = useState(false);
  const selectRef = useRef<HTMLDivElement>(null);
  const safeOptions = Array.isArray(options) ? options :[];
  const selectedOption = safeOptions.find((o: any) => o.value === value) || { label: "Любое" };

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) { if (selectRef.current && !selectRef.current.contains(e.target as Node)) setIsOpen(false); }
    document.addEventListener("mousedown", handleClickOutside); return () => document.removeEventListener("mousedown", handleClickOutside);
  },[]);

  return (
    <div className={`flex flex-col relative ${disabled ? 'opacity-50 pointer-events-none' : ''}`} ref={selectRef}>
      <div onClick={() => setIsOpen(!isOpen)} className="bg-black/50 border border-white/10 rounded-lg px-3 py-2 text-white cursor-pointer select-none hover:border-jm-accent transition-colors flex justify-between items-center min-w-[110px] text-xs shadow-inner">
        {label && <span className="text-[var(--text-secondary)] mr-1.5">{label}:</span>}<span className="font-bold truncate">{selectedOption?.label}</span>
      </div>
      <AnimatePresence>
        {isOpen && (
          <motion.div initial={{ opacity: 0, y: -8, scale: 0.97 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: -6, scale: 0.98 }} transition={{ type: "spring", stiffness: 520, damping: 32, mass: 0.55 }} className="absolute top-[100%] mt-2 w-full bg-[var(--input-bg)]/95 backdrop-blur-xl border border-white/10 rounded-xl z-[20000] max-h-60 overflow-y-auto custom-scrollbar shadow-[0_10px_40px_rgba(0,0,0,0.8)]">
            {safeOptions.map((o: any) => (
              <div key={o.value} onClick={() => { onChange(o.value); setIsOpen(false); }} className={`px-4 py-2 cursor-pointer transition-colors text-sm ${value === o.value ? 'bg-jm-accent/20 text-jm-accent-light' : 'text-white hover:bg-white/10'}`}>{o.label}</div>
            ))}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

export default function DiscoverTab({ contextInstance, installedMods = [], onModsChanged, initialProjectId }: { contextInstance?: any, installedMods?: string[], onModsChanged?: () => void, initialProjectId?: string }) {
  const [query, setQuery] = useState("");
  const[projectType, setProjectType] = useState("mod");
  const [gameVersion, setGameVersion] = useState(contextInstance?.game_version || "");
  const [loader, setLoader] = useState(contextInstance?.loader === "vanilla" ? "" : (contextInstance?.loader || ""));
  const loaderLocked = !!contextInstance && (projectType === "mod" || projectType === "modpack");
  const [selectedCategories, setSelectedCategories] = useState<string[]>([]);
  const[results, setResults] = useState<any[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [page, setPage] = useState(0);
  const [pageInput, setPageInput] = useState("1");
  const [sortMode, setSortMode] = useState<"relevance" | "popularity" | "updated" | "downloads" | "name" | "author" | "rating">("relevance");
  /** true = по убыванию (популярные сверху), false = по возрастанию */
  const [sortDesc, setSortDesc] = useState(true);
  const [totalHits, setTotalHits] = useState(0);
  /** Просмотр скриншотов: null или { urls, index } */
  const [galleryLightbox, setGalleryLightbox] = useState<{ urls: string[]; index: number } | null>(null);

  const [selectedProject, setSelectedProject] = useState<any>(null);
  const[projectDetails, setProjectDetails] = useState<any>(null);
  const [projectVersions, setProjectVersions] = useState<any[]>([]);
  const [modalTab, setModalTab] = useState("desc");

  const[vFilter, setVFilter] = useState(contextInstance?.game_version || "");
  const [lFilter, setLFilter] = useState(contextInstance?.loader === "vanilla" ? "" : (contextInstance?.loader || ""));

  const[localInstalledMods, setLocalInstalledMods] = useState<string[]>(Array.isArray(installedMods) ? installedMods :[]);
  const [installTarget, setInstallTarget] = useState<any>(null);
  const[instances, setInstances] = useState<any[]>([]);
  const[datapackTarget, setDatapackTarget] = useState<any>(null);
  const[worlds, setWorlds] = useState<string[]>([]);
  const [customPacks, setCustomPacks] = useState<any[]>([]);
  const [customPacksLoading, setCustomPacksLoading] = useState(false);
  const [modProvider, setModProvider] = useState<"modrinth" | "curseforge" | "hybrid">("modrinth");
  const [hybridProviderEnabled, setHybridProviderEnabled] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);

  useEffect(() => {
    invoke("load_settings").then((s: any) => {
      setModProvider((s?.mod_provider === "curseforge" || s?.mod_provider === "hybrid") ? s.mod_provider : "modrinth");
      setHybridProviderEnabled(!!s?.hybrid_provider_enabled);
    }).catch(() => {});
  }, []);

  // Если пользователь открыл вкладку из конкретного instance — автоматически подставляем текущие ограничения.
  useEffect(() => {
    setVFilter(contextInstance?.game_version || "");
    setLFilter(contextInstance?.loader === "vanilla" ? "" : (contextInstance?.loader || ""));
  }, [contextInstance]);

  useEffect(() => { setLocalInstalledMods(Array.isArray(installedMods) ? installedMods : []); }, [installedMods]);

  useEffect(() => {
    if (!galleryLightbox) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setGalleryLightbox(null);
        return;
      }
      if (e.key === "ArrowRight") {
        e.preventDefault();
        setGalleryLightbox((cur) => {
          if (!cur?.urls.length) return cur;
          return { ...cur, index: (cur.index + 1) % cur.urls.length };
        });
      }
      if (e.key === "ArrowLeft") {
        e.preventDefault();
        setGalleryLightbox((cur) => {
          if (!cur?.urls.length) return cur;
          const n = cur.urls.length;
          return { ...cur, index: (cur.index - 1 + n) % n };
        });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [galleryLightbox]);

  useEffect(() => {
    if (projectType !== "custom") return;
    setCustomPacksLoading(true);
    invoke("get_custom_packs").then((data: any) => {
      const arr = Array.isArray(data) ? data : (data?.packs || data?.items || []);
      setCustomPacks(Array.isArray(arr) ? arr : []);
    }).catch(() => setCustomPacks([])).finally(() => setCustomPacksLoading(false));
  }, [projectType]);

  useEffect(() => {
    if (initialProjectId) {
      invoke("get_modrinth_project", { id: initialProjectId }).then((details: any) => {
        if (details) {
          setSelectedProject(details);
          setProjectDetails(details);
          setModalTab("desc");
          invoke("get_modrinth_versions", { id: initialProjectId }).then((versions: any) => {
            setProjectVersions(Array.isArray(versions) ? versions : []);
          });
        }
      }).catch(console.error);
    }
  }, [initialProjectId]);

  const limit = 20;
  const totalPages = Math.max(1, Math.ceil((totalHits || 0) / limit));

  useEffect(() => {
    if (contextInstance && projectType === "modpack") setProjectType("mod");
    if (projectType === "resourcepack" || projectType === "shader" || projectType === "datapack") {
      setLoader("");
    } else if (contextInstance && contextInstance.loader !== "vanilla") {
      setLoader(contextInstance.loader);
    }
    setSelectedCategories([]); setPage(0); setPageInput("1");
  },[projectType, contextInstance]);

  const fetchProjects = async () => {
    if (projectType === "custom") return;
    setIsLoading(true);
    setSearchError(null);
    try {
      const queryTrim = (query || "").trim();
      const params = { query: queryTrim, projectType: projectType || "mod", gameVersion: (gameVersion || "").trim(), loader: loader || "", categories: selectedCategories || [], page: page || 0, sort: sortMode, sortDesc };
      const data: any = modProvider === "hybrid"
        ? await invoke("search_hybrid", params)
        : modProvider === "curseforge"
          ? await invoke("search_curseforge", params)
          : await invoke("search_modrinth", params);
      let hits = data?.hits || [];
      let didLocalFilter = false;
      // CurseForge поиск иногда возвращает "похожие" результаты, поэтому делаем мягкую локальную фильтрацию по title/author.
      if ((modProvider === "curseforge" || modProvider === "hybrid") && queryTrim) {
        const q = queryTrim.toLowerCase();
        hits = hits.filter((p: any) => {
          const t = (p?.title || "").toLowerCase();
          const a = (p?.author || "").toLowerCase();
          return t.includes(q) || a.includes(q);
        });
        didLocalFilter = true;
      }
      setResults(hits);
      setTotalHits(didLocalFilter ? hits.length : (data?.total_hits || 0));
      const err = data?.error;
      if (err === "curseforge_no_api_key") setSearchError("curseforge_no_api_key");
      else if (err === "curseforge_forbidden") setSearchError("curseforge_forbidden");
    } catch (e) { console.error(e); } finally { setIsLoading(false); }
  };

  useEffect(() => {
    if (projectType === "custom") {
      setCustomPacksLoading(true);
      invoke("load_custom_packs_config").then((cfg: any) => {
        const url = cfg?.url || "";
        if (!url?.trim()) { setCustomPacks([]); setCustomPacksLoading(false); return; }
        invoke("fetch_custom_packs", { url }).then((data: any) => {
          const arr = Array.isArray(data) ? data : (data?.packs || data?.items || []);
          setCustomPacks(arr);
        }).catch(() => setCustomPacks([])).finally(() => setCustomPacksLoading(false));
      }).catch(() => setCustomPacksLoading(false));
    }
  }, [projectType]);

  useEffect(() => { if (projectType !== "custom") { const t = setTimeout(() => { setPage(0); setPageInput("1"); fetchProjects(); }, 500); return () => clearTimeout(t); } },[query, projectType, gameVersion, loader, selectedCategories, modProvider, sortMode, sortDesc]);
  useEffect(() => { if (projectType !== "custom" && (page !== 0 || !query)) fetchProjects(); setPageInput((page + 1).toString()); }, [page]);

  const handlePageSubmit = (e: any) => {
    if (e.key === 'Enter') {
      let p = parseInt(pageInput) - 1;
      if (isNaN(p) || p < 0) p = 0;
      if (p >= totalPages) p = totalPages - 1;
      setPage(p);
    }
  };

  const openProject = async (project: any) => {
    setSelectedProject(project); setModalTab("desc"); setProjectDetails(null); setProjectVersions([]);
    try {
      const id = project.project_id;
      const modrinthId = project.modrinth_id ?? id;
      const curseforgeId = project.curseforge_id ?? null;
      if (modProvider === "hybrid") {
        // В hybrid сперва пробуем Modrinth, а если его нет — подтягиваем мету из CurseForge.
        let details: any = null;
        if (modrinthId) {
          details = await invoke("get_modrinth_project", { id: modrinthId });
          // Если Modrinth мета "пустая" (например, нет описания), подгружаем полную с CurseForge.
          const hasDesc = !!(details?.body || details?.description || details?.summary);
          if (!hasDesc && curseforgeId) {
            details = await invoke("get_curseforge_project", { id: curseforgeId });
          }
        } else if (curseforgeId) {
          details = await invoke("get_curseforge_project", { id: curseforgeId });
        }
        setProjectDetails(details);
        const versions: any = await invoke("get_hybrid_versions", { modrinthId: modrinthId || null, curseforgeId });
        setProjectVersions(Array.isArray(versions) ? versions : []);
      } else if (modProvider === "curseforge") {
        const details = await invoke("get_curseforge_project", { id });
        setProjectDetails(details);
        const versions: any = await invoke("get_curseforge_versions", { id });
        setProjectVersions(Array.isArray(versions) ? versions : []);
      } else {
        const details = await invoke("get_modrinth_project", { id: modrinthId });
        setProjectDetails(details);
        const versions: any = await invoke("get_modrinth_versions", { id: modrinthId });
        setProjectVersions(Array.isArray(versions) ? versions : []);
      }
    } catch (e) { console.error(e); }
  };

  const handleDownloadClick = async (version: any) => {
    if (projectType === "datapack") {
      const file = version?.files?.find((f: any) => f.primary) || version?.files?.[0];
      if (!file) return showToast("Файл не найден");
      if (contextInstance) {
        const w: any = await invoke("list_worlds", { instanceId: contextInstance.id }).catch(() => []);
        setWorlds(w || []);
        setDatapackTarget({ url: file.url, filename: file.filename, instanceId: contextInstance.id });
      } else {
        const insts: any = await invoke("get_instances");
        setInstances(insts || []);
        setDatapackTarget({ url: file.url, filename: file.filename, instanceId: null });
      }
      return;
    }
    if (projectType === "modpack") {
      const file = version?.files?.find((f: any) => f.primary) || version?.files?.[0];
      if (!file) return showToast("Файл не найден");
      try {
        showToast(`Установка сборки ${selectedProject?.title || ""}...`);
        await invoke("install_mrpack_from_url", { url: file.url, name: selectedProject?.title || "Modpack" });
        showToast("Сборка успешно установлена!");
      } catch (e) { showToast(`Ошибка: ${e}`); }
      return;
    }
    
    if (!contextInstance) {
      const insts: any = await invoke("get_instances");
      setInstances(insts ||[]);
      const file = version?.files?.find((f: any) => f.primary) || version?.files?.[0];
      if (file) setInstallTarget({ url: file.url, filename: file.filename });
      return;
    }

    const file = version?.files?.find((f: any) => f.primary) || version?.files?.[0];
    if (!file) return showToast("Файл не найден на сервере Modrinth");

    // РП и шейдеры — в resourcepacks/shaderpacks, не через install_mod_with_dependencies
    if (projectType === "resourcepack" || projectType === "shader") {
      try {
        showToast(`Установка ${file.filename}...`);
        await invoke("install_modrinth_file", { instanceId: contextInstance.id, url: file.url, filename: file.filename, projectType });
        await invoke("refresh_mod_metadata", { instanceId: contextInstance.id });
        showToast(`Успешно установлено!`);
        if (onModsChanged) onModsChanged();
      } catch (e) { showToast(`Ошибка: ${e}`); }
      return;
    }

    try {
      showToast(`Установка ${file.filename}...`);
      await invoke("install_mod_with_dependencies", { 
        instanceId: contextInstance.id, 
        versionId: version.id, 
        gameVersion: contextInstance.game_version, 
        loader: contextInstance.loader 
      });
      await invoke("refresh_mod_metadata", { instanceId: contextInstance.id });
      showToast(`Успешно установлено!`);
      
      const cleanName = (file.filename || "").replace(".jar", "").replace(".zip", "");
      if (cleanName) setLocalInstalledMods(prev =>[...prev, cleanName]);
      if (onModsChanged) onModsChanged();
    } catch (e) { showToast(`Ошибка: ${e}`); }
  };

  const installIntoInstance = async (instanceId: string, url: string = installTarget?.url, filename: string = installTarget?.filename) => {
    if (!url || !filename) return;
    try {
      showToast(`Скачивание ${filename}...`);
      await invoke("install_modrinth_file", { instanceId, url, filename, projectType });
      await invoke("refresh_mod_metadata", { instanceId });
      showToast(`Успешно установлено!`);
      setInstallTarget(null);
    } catch (e) { showToast(`Ошибка установки: ${e}`); }
  };

  const noLoaderTypes = ["resourcepack", "shader", "datapack"];
  const types: {id:string, label:string, icon:any}[] = [
    { id: "mod", label: "Моды", icon: <Package size={16} /> },
    { id: "resourcepack", label: "РП", icon: <ImageIcon size={16} /> },
    { id: "shader", label: "Шейдеры", icon: <Sparkles size={16} /> },
    { id: "datapack", label: "Датапаки", icon: <Layers size={16} /> },
  ];
  if (!contextInstance) { types.splice(1, 0, { id: "modpack", label: "Сборки", icon: <Layers size={16} /> }); types.push({ id: "custom", label: "Кастомные", icon: <Link size={16} /> }); }

  const paginationBlock = (
    <div className="flex items-center justify-center gap-4 py-3 shrink-0 bg-jm-bg/80 backdrop-blur-md border border-white/5 rounded-2xl shadow-lg w-fit mx-auto relative z-20">
      <button type="button" onPointerDown={(e) => { e.stopPropagation(); setPage(p => Math.max(0, p - 1)); }} disabled={page === 0} className="p-2 text-jm-accent disabled:opacity-30 hover:bg-white/5 rounded-lg transition-colors select-none cursor-pointer"><ChevronLeft size={20} /></button>
      <div className="flex items-center gap-2 text-sm text-[var(--text-secondary)]">
        Стр.&nbsp;
        <input
          type="text"
          inputMode="numeric"
          value={pageInput}
          onChange={e => { const v = e.target.value.replace(/\D/g, ""); setPageInput(v); }}
          onKeyDown={handlePageSubmit}
          onBlur={() => {
            const n = parseInt(pageInput);
            if (!Number.isFinite(n) || n < 1) { setPageInput((page + 1).toString()); return; }
            let p = n - 1;
            if (p >= totalPages) p = totalPages - 1;
            setPage(p);
          }}
          className="w-14 bg-black border border-white/10 rounded text-center text-white py-1 outline-none focus:border-jm-accent transition-colors"
        />
        из {totalPages}
      </div>
      <button type="button" onPointerDown={(e) => { e.stopPropagation(); setPage(p => Math.min(totalPages - 1, p + 1)); }} disabled={page >= totalPages - 1} className="p-2 text-jm-accent disabled:opacity-30 hover:bg-white/5 rounded-lg transition-colors select-none cursor-pointer"><ChevronRight size={20} /></button>
    </div>
  );

  const providerOptions = [
    { value: "modrinth", label: "Modrinth" },
    { value: "curseforge", label: "CurseForge" },
    ...(hybridProviderEnabled ? [{ value: "hybrid", label: "Гибрид" }] : []),
  ];

  return (
    <div className="flex w-full max-w-7xl mx-auto h-full gap-3 md:gap-4">
      
      {/* ЛЕВАЯ ПАНЕЛЬ: Поставщик + Категории */}
      <div className="hidden md:flex w-48 lg:w-56 shrink-0 bg-jm-card p-3 lg:p-4 rounded-2xl border border-white/10 shadow-2xl flex-col h-full overflow-y-auto custom-scrollbar">
        <h3 className="font-bold text-jm-accent-light mb-2 text-sm lg:text-base">Поставщик</h3>
        <div className="mb-4">
          <CustomSelect
            label=""
            value={modProvider}
            onChange={(v: string) => {
              const val = v as "modrinth" | "curseforge" | "hybrid";
              setModProvider(val);
              invoke("load_settings").then((s: any) => {
                invoke("save_settings", { settings: { ...s, mod_provider: val } }).catch(() => {});
              }).catch(() => {});
            }}
            options={providerOptions}
          />
        </div>
        <h3 className="font-bold text-jm-accent-light mb-3 text-sm lg:text-base">Категории</h3>
        <div className="flex flex-col gap-1.5">
          {(CATEGORY_MAP[projectType] ||[]).map(cat => (
            <button key={cat} onClick={() => { if(selectedCategories.includes(cat)) setSelectedCategories(selectedCategories.filter(c=>c!==cat)); else setSelectedCategories([...selectedCategories, cat]); }} className={`px-3 py-2 rounded-lg text-xs lg:text-sm font-bold transition-all text-left border ${selectedCategories.includes(cat) ? 'bg-jm-accent text-black border-jm-accent' : 'bg-black/30 text-[var(--text-secondary)] border-transparent hover:bg-white/5 hover:text-white'}`}>
              {cat.charAt(0).toUpperCase() + cat.slice(1)}
            </button>
          ))}
        </div>
      </div>

      {/* ПРАВАЯ ПАНЕЛЬ: Поиск */}
      <div className="flex flex-col flex-grow min-w-0 h-full">
        <div className="bg-jm-card p-3 md:p-4 rounded-2xl border border-white/10 shadow-2xl mb-3 flex flex-col gap-3 shrink-0">
          <div className="relative w-full">
            <Search className="absolute left-3 top-2.5 text-[var(--text-secondary)]" size={16} />
            <input type="text" placeholder="Поиск..." value={query} onChange={(e) => setQuery(e.target.value)} className="w-full bg-black/50 border border-white/10 rounded-xl pl-10 pr-3 py-2.5 text-sm text-white outline-none transition-colors focus:border-jm-accent shadow-inner" />
          </div>
          <div className="flex flex-wrap gap-2 items-center">
            <div className="flex gap-1 bg-black/30 p-0.5 rounded-lg border border-white/5 shadow-inner overflow-x-auto [&::-webkit-scrollbar]:hidden">
              {types.map((t) => <button key={t.id} onClick={() => setProjectType(t.id)} className={`flex items-center gap-1 px-2.5 py-1.5 rounded-md text-xs font-bold transition-all whitespace-nowrap ${projectType === t.id ? "bg-jm-accent text-black shadow-md" : "text-[var(--text-secondary)] hover:text-white hover:bg-white/5"}`}>{t.icon} {t.label}</button>)}
            </div>
            <CustomSelect label="Версия" value={gameVersion} onChange={setGameVersion} disabled={!!contextInstance && projectType !== "datapack"} options={[{ value: "", label: "Любая" }, ...["1.21.5","1.21.4","1.21.3","1.21.2","1.21.1","1.21","1.20.6","1.20.4","1.20.2","1.20.1","1.20","1.19.4","1.19.2","1.18.2","1.17.1","1.16.5","1.12.2","1.8.9","1.7.10"].map(v => ({ value: v, label: v }))]} />
            {!noLoaderTypes.includes(projectType) && (
              <CustomSelect label="Ядро" value={loader} onChange={setLoader} disabled={loaderLocked} options={[{ value: "", label: "Любое" }, { value: "fabric", label: "Fabric" }, { value: "forge", label: "Forge" }, { value: "neoforge", label: "NeoForge" }, { value: "quilt", label: "Quilt" }]} />
            )}
            <div className="flex flex-wrap items-end gap-1.5">
              <CustomSelect label="Сортировка" value={sortMode} onChange={setSortMode} options={[
                { value: "relevance", label: "Релевантность" },
                { value: "popularity", label: "Популярность" },
                { value: "downloads", label: "Скачивания" },
                { value: "updated", label: "Обновление" },
                { value: "rating", label: "Рейтинг" },
                { value: "name", label: "Имя" },
                { value: "author", label: "Автор" },
              ]} />
              <motion.button
                type="button"
                whileHover={{ scale: 1.04 }}
                whileTap={{ scale: 0.94 }}
                title={sortDesc ? "По убыванию (больше → меньше). Клик — по возрастанию" : "По возрастанию (меньше → больше). Клик — по убыванию"}
                onClick={() => setSortDesc((d) => !d)}
                className="h-[38px] min-w-[40px] px-2.5 rounded-lg border border-white/10 bg-black/50 text-jm-accent hover:bg-jm-accent/15 hover:border-jm-accent/40 transition-colors flex items-center justify-center shadow-inner"
              >
                {sortDesc ? <ArrowDownWideNarrow size={18} /> : <ArrowUpWideNarrow size={18} />}
              </motion.button>
            </div>
            {/* Provider select for mobile (hidden on md+) */}
            <div className="md:hidden">
              <CustomSelect label="Источник" value={modProvider} onChange={(v: string) => { const val = v as "modrinth" | "curseforge" | "hybrid"; setModProvider(val); invoke("load_settings").then((s: any) => { invoke("save_settings", { settings: { ...s, mod_provider: val } }).catch(() => {}); }).catch(() => {}); }} options={providerOptions} />
            </div>
          </div>
        </div>

        {totalHits > limit && <div className="mb-4">{paginationBlock}</div>}

        <div className="flex-grow overflow-y-auto custom-scrollbar pr-2 relative">
          {(isLoading || (projectType === "custom" && customPacksLoading)) && <div className="absolute inset-0 flex items-center justify-center bg-jm-bg/50 backdrop-blur-sm z-10 rounded-2xl pointer-events-none"><Loader2 className="animate-spin text-jm-accent" size={48} /></div>}
          {projectType === "custom" ? (
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 pb-6">
              {customPacks.length === 0 && !customPacksLoading ? (
                <div className="col-span-full text-center text-[var(--text-secondary)] py-12">Укажите URL в настройках или загрузка не удалась.</div>
              ) : (
                customPacks.map((pack: any, idx: number) => {
                  const title = pack.title || pack.name || "Без названия";
                  const url = pack.url || pack.mrpack_url || pack.download_url;
                  return (
                    <motion.div key={pack.id || idx} whileHover={{ scale: 1.02 }} whileTap={{ scale: 0.98 }} className="bg-jm-card border border-white/5 hover:border-jm-accent/50 rounded-2xl p-4 flex flex-col transition-colors group shadow-xl">
                      <div className="flex items-start gap-4 mb-3">
                        {pack.icon_url ? (
                          <><img src={pack.icon_url} alt="" className="w-16 h-16 rounded-xl object-cover bg-black/50 border border-white/20" onError={e => { (e.target as HTMLImageElement).style.display = "none"; (e.target as HTMLImageElement).nextElementSibling?.classList.remove("hidden"); }} /><div className="hidden w-16 h-16 rounded-xl bg-black/50 border border-white/20 flex items-center justify-center text-sm font-medium text-white/70 shrink-0">{((title || "?") as string).split(/\s+/).map((w: string) => w[0]).join("").slice(0, 2).toUpperCase() || "?"}</div></>
                        ) : (
                          <div className="w-16 h-16 rounded-xl bg-black/50 border border-white/20 flex items-center justify-center text-sm font-medium text-white/70 shrink-0">{((title || "?") as string).split(/\s+/).map((w: string) => w[0]).join("").slice(0, 2).toUpperCase() || "?"}</div>
                        )}
                        <div className="flex-grow min-w-0">
                          <h3 className="font-bold text-white text-lg truncate group-hover:text-jm-accent-light transition-colors">{title}</h3>
                          <p className="text-xs text-jm-accent truncate">от {pack.author || "Неизвестен"}</p>
                        </div>
                      </div>
                      <p className="text-sm text-[var(--text-secondary)] line-clamp-2 mb-3">{pack.description || ""}</p>
                      <button onClick={async () => { if (!url) return showToast("Нет ссылки на сборку"); try { showToast(`Установка ${title}...`); await invoke("install_mrpack_from_url", { url, name: title }); showToast("Сборка установлена!"); } catch (e) { showToast(`Ошибка: ${e}`); } }} disabled={!url} className="mt-auto px-4 py-2 rounded-xl bg-jm-accent/20 hover:bg-jm-accent text-jm-accent hover:text-black font-bold text-sm transition-colors disabled:opacity-50 flex items-center justify-center gap-2">
                        <Download size={16} /> Установить
                      </button>
                    </motion.div>
                  );
                })
              )}
            </div>
          ) : !isLoading && results.length === 0 && searchError && (modProvider === "curseforge" || modProvider === "hybrid") ? (
            <div className="flex flex-col items-center justify-center py-16 px-6 text-center">
              <p className="text-[var(--text-secondary)] text-lg mb-2">
                {searchError === "curseforge_no_api_key"
                  ? "Для CurseForge нужен API ключ"
                  : "CurseForge отклонил запрос (лимит, блокировка IP/VPN или ключ в настройках неверный)"}
              </p>
              <p className="text-sm text-jm-accent/90">Настройки → Экспериментальные → CurseForge API ключ</p>
              <p className="text-xs text-[var(--text-secondary)] mt-2">Если поле ключа заполнено — попробуйте очистить его (будет встроенный ключ). Свой ключ: console.curseforge.com</p>
            </div>
          ) : (
            <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-3 pb-4">
              {results.map((project, ri) => (
                <motion.div
                  initial={{ opacity: 0, y: 12 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ type: "spring", stiffness: 380, damping: 28, delay: Math.min(ri * 0.03, 0.35) }}
                  whileHover={{ y: -4, scale: 1.015, transition: { type: "spring", stiffness: 400, damping: 18 } }}
                  whileTap={{ scale: 0.98 }}
                  key={project.project_id}
                  onClick={() => openProject(project)}
                  className="bg-jm-card border border-white/5 hover:border-jm-accent/50 rounded-xl p-3 flex flex-col transition-shadow cursor-pointer group shadow-xl hover:shadow-[0_12px_40px_rgba(134,168,134,0.12)]"
                >
                  <div className="flex items-start gap-3 mb-2">
                    {project?.icon_url ? (
                      <>
                        <img src={project.icon_url} alt="" className="w-10 h-10 md:w-12 md:h-12 rounded-lg object-cover bg-black/50 border border-white/20 shrink-0" onError={e => { (e.target as HTMLImageElement).style.display = "none"; (e.target as HTMLImageElement).nextElementSibling?.classList.remove("hidden"); }} />
                        <div className="hidden w-10 h-10 md:w-12 md:h-12 rounded-lg bg-black/50 border border-white/20 flex items-center justify-center text-xs font-medium text-white/70 shrink-0">{((project?.title || "?") as string).split(/\s+/).map((w: string) => w[0]).join("").slice(0, 2).toUpperCase() || "?"}</div>
                      </>
                    ) : (
                      <div className="w-10 h-10 md:w-12 md:h-12 rounded-lg bg-black/50 border border-white/20 flex items-center justify-center text-xs font-medium text-white/70 shrink-0">{((project?.title || "?") as string).split(/\s+/).map((w: string) => w[0]).join("").slice(0, 2).toUpperCase() || "?"}</div>
                    )}
                    <div className="flex-grow min-w-0">
                      <h3 className="font-bold text-white text-sm md:text-base truncate group-hover:text-jm-accent-light transition-colors">{project?.title || "Без названия"}</h3>
                      <p className="text-xs text-jm-accent truncate">от {project?.author || "Неизвестен"}</p>
                    </div>
                  </div>
                  <p className="text-xs text-[var(--text-secondary)] line-clamp-2">{project?.description || ""}</p>
                </motion.div>
              ))}
            </div>
          )}
        </div>
        {totalHits > limit && projectType !== "custom" && <div className="mt-2">{paginationBlock}</div>}
      </div>

      <AnimatePresence>
        {selectedProject && (
          <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.2 }} className="fixed inset-0 z-50 flex">
            <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="hidden sm:block sm:w-[10%] md:w-[15%] lg:w-[25%] bg-black/60 backdrop-blur-md shrink-0" onClick={() => setSelectedProject(null)} />
            <motion.div initial={{ x: "104%", opacity: 0.92 }} animate={{ x: 0, opacity: 1 }} exit={{ x: "104%", opacity: 0.9 }} transition={{ type: "spring", damping: 26, stiffness: 320, mass: 0.72 }} className="w-full sm:w-[90%] md:w-[85%] lg:w-[75%] h-full bg-jm-bg border-l border-white/10 shadow-2xl flex flex-col overflow-hidden relative">
              <div className="bg-gradient-to-r from-jm-card to-black/40 p-3 md:p-4 border-b border-white/10 flex gap-3 md:gap-4 items-center shrink-0">
                {selectedProject?.icon_url ? (
                  <><img src={selectedProject.icon_url} alt="" className="w-12 h-12 md:w-16 md:h-16 rounded-xl object-cover bg-black/50 shadow-lg border border-white/20 shrink-0" onError={e => { (e.target as HTMLImageElement).style.display = "none"; (e.target as HTMLImageElement).nextElementSibling?.classList.remove("hidden"); }} /><div className="hidden w-12 h-12 md:w-16 md:h-16 rounded-xl bg-black/50 border border-white/20 flex items-center justify-center text-sm font-medium text-white/70 shrink-0">{((selectedProject?.title || "?") as string).split(/\s+/).map((w: string) => w[0]).join("").slice(0, 2).toUpperCase() || "?"}</div></>
                ) : (
                  <div className="w-12 h-12 md:w-16 md:h-16 rounded-xl bg-black/50 border border-white/20 flex items-center justify-center text-sm font-medium text-white/70 shrink-0">{((selectedProject?.title || "?") as string).split(/\s+/).map((w: string) => w[0]).join("").slice(0, 2).toUpperCase() || "?"}</div>
                )}
                <div className="flex-grow min-w-0">
                  <h2 className="text-lg md:text-xl font-bold text-white mb-1 truncate">{selectedProject?.title}</h2>
                  <div className="flex gap-1 bg-black/30 p-0.5 rounded-lg border border-white/5 w-fit shadow-inner mt-1">
                    <button onClick={() => setModalTab('desc')} className={`px-2.5 py-1.5 rounded-md text-xs font-bold transition-all flex items-center gap-1 ${modalTab === 'desc' ? 'bg-jm-accent text-black shadow-md' : 'text-[var(--text-secondary)] hover:text-white'}`}><AlignLeft size={14} /> <span className="hidden sm:inline">Описание</span></button>
                    <button onClick={() => setModalTab('versions')} className={`px-2.5 py-1.5 rounded-md text-xs font-bold transition-all flex items-center gap-1 ${modalTab === 'versions' ? 'bg-jm-accent text-black shadow-md' : 'text-[var(--text-secondary)] hover:text-white'}`}><List size={14} /> <span className="hidden sm:inline">Версии</span></button>
                    <button onClick={() => setModalTab('screens')} className={`px-2.5 py-1.5 rounded-md text-xs font-bold transition-all flex items-center gap-1 ${modalTab === 'screens' ? 'bg-jm-accent text-black shadow-md' : 'text-[var(--text-secondary)] hover:text-white'}`}><ImageIcon size={14} /> <span className="hidden sm:inline">Скриншоты</span></button>
                  </div>
                </div>
                <button onClick={() => setSelectedProject(null)} className="text-[var(--text-secondary)] hover:text-white bg-black/50 hover:bg-red-500/20 hover:text-red-500 p-1.5 rounded-full transition-colors shrink-0"><X size={20} /></button>
              </div>

              <div className="flex-grow overflow-y-auto p-3 md:p-5 custom-scrollbar relative">
                {modalTab === 'desc' && (
                  projectDetails ? (
                    (() => {
                      const raw = projectDetails.body || projectDetails.description || projectDetails.summary || "Нет описания";
                      if (looksLikeHtml(raw)) {
                        const safe = sanitizeProjectBody(raw);
                        return <div className="text-[var(--text-secondary)] prose prose-invert prose-lg max-w-none text-base leading-relaxed break-words [&_img]:max-w-full [&_img]:h-auto [&_a]:text-jm-accent [&_a]:underline" dangerouslySetInnerHTML={{ __html: safe || "Нет описания" }} />;
                      }
                      return <div className="text-[var(--text-secondary)] prose prose-invert prose-lg max-w-none text-base leading-relaxed"><ReactMarkdown remarkPlugins={[remarkGfm]}>{raw}</ReactMarkdown></div>;
                    })()
                  ) : <div className="flex items-center justify-center h-40"><Loader2 className="animate-spin text-jm-accent" size={48} /></div>
                )}
                
                {modalTab === 'screens' && (
                  projectDetails ? (
                    (() => {
                      const rawGallery = projectDetails?.gallery ?? projectDetails?.screenshots ?? [];
                      const galleryUrls: string[] = Array.isArray(rawGallery)
                        ? rawGallery
                            .map((x: any) => {
                              if (!x) return null;
                              if (typeof x === "string") return x;
                              return x.url || x.thumbnailUrl || x.image_url || null;
                            })
                            .filter((u: any) => typeof u === "string" && u.trim().length > 0)
                        : [];
                      if (galleryUrls.length === 0) {
                        return <div className="text-[var(--text-secondary)] text-center py-10">Скриншотов нет</div>;
                      }
                      return (
                        <div className="columns-1 sm:columns-2 lg:columns-3 gap-3" style={{ columnFill: "balance" as const }}>
                          {galleryUrls.map((u: string, idx: number) => (
                            <motion.button
                              type="button"
                              key={`${u}-${idx}`}
                              initial={{ opacity: 0, scale: 0.96 }}
                              animate={{ opacity: 1, scale: 1 }}
                              transition={{ type: "spring", stiffness: 400, damping: 28, delay: Math.min(idx * 0.04, 0.4) }}
                              whileHover={{ scale: 1.02 }}
                              whileTap={{ scale: 0.98 }}
                              onClick={() => setGalleryLightbox({ urls: galleryUrls, index: idx })}
                              className="mb-3 w-full break-inside-avoid rounded-xl overflow-hidden border border-white/10 bg-white/[0.04] hover:border-jm-accent/45 hover:shadow-lg hover:shadow-jm-accent/10 transition-all text-left block focus:outline-none focus-visible:ring-2 focus-visible:ring-jm-accent/60"
                            >
                              <img
                                src={u}
                                alt=""
                                className="w-full h-auto max-w-full align-bottom block"
                                loading="lazy"
                                decoding="async"
                                onError={(e) => { (e.target as HTMLImageElement).style.display = "none"; }}
                              />
                            </motion.button>
                          ))}
                        </div>
                      );
                    })()
                  ) : (
                    <div className="flex items-center justify-center h-40"><Loader2 className="animate-spin text-jm-accent" size={48} /></div>
                  )
                )}

                {modalTab === 'versions' && (
                  (() => {
                    const pt = (selectedProject?.project_type || projectType || "").toLowerCase().replace(/_/g, "");
                    const isNoLoaderProject = ["resourcepack", "shader", "datapack"].includes(pt);
                    return (
                  <div className="flex flex-col gap-3">
                    <div className="flex gap-4 mb-2 bg-black/30 p-3 rounded-xl border border-white/5 shadow-inner">
                      <CustomSelect label="Версия" value={vFilter} onChange={setVFilter} disabled={false} options={[{ value: "", label: "Любая" }, ...Array.from(new Set(projectVersions.flatMap((v: any) => v?.game_versions || []))).sort((a: any, b: any) => b.localeCompare(a, undefined, { numeric: true })).map((gv: any) => ({ value: gv, label: gv }))]} />
                      {!isNoLoaderProject && <CustomSelect label="Загрузчик" value={lFilter} onChange={setLFilter} disabled={false} options={[{ value: "", label: "Любой" }, { value: "fabric", label: "Fabric" }, { value: "forge", label: "Forge" }, { value: "neoforge", label: "NeoForge" }, { value: "quilt", label: "Quilt" }]} />}
                    </div>
                    
                    {projectVersions.length > 0 ? projectVersions.filter((v: any) => {
                      if (!v) return false;
                      const gv = v.game_versions ||[];
                      const ld = v.loaders ||[];
                      // Эффективные фильтры:
                      // - если пользователь выбрал vFilter/lFilter — используем их
                      // - иначе ограничиваем по contextInstance (если он есть)
                      const wantGV = vFilter;
                      const wantLoader = lFilter;

                      if (wantGV && !gv.includes(wantGV)) return false;
                      if (!isNoLoaderProject && wantLoader && !ld.includes(wantLoader)) return false;
                      return true;
                    }).map((v: any, idx: number) => {
                      const file = v.files?.find((f: any) => f.primary) || v.files?.[0];
                      const cleanName = (file?.filename || "").replace(".jar", "").replace(".zip", "");
                      const isInstalled = localInstalledMods.includes(cleanName);
                      const loadersStr = (v.loaders || []).join(", ");

                      return (
                        <div key={v.id ? `${v.id}-${v._source || "m"}` : `v-${idx}`} className={`p-3 rounded-xl border flex flex-col sm:flex-row sm:justify-between sm:items-center gap-2 transition-colors shadow-md ${isInstalled ? 'bg-jm-accent/5 border-jm-accent/30' : 'bg-jm-card border-white/5'}`}>
                          <div className="min-w-0">
                            <div className="font-bold text-white text-sm md:text-base flex items-center gap-2 flex-wrap">
                              {v.name || "Версия"}
                              {v._source && <span className={`text-[9px] px-1.5 py-0.5 rounded-full border ${v._source === "curseforge" ? "bg-orange-500/20 text-orange-400 border-orange-500/30" : "bg-blue-500/20 text-blue-400 border-blue-500/30"}`}>{v._source === "curseforge" ? "CF" : "MR"}</span>}
                              {isInstalled && <span className="bg-jm-accent/20 text-jm-accent text-[9px] px-1.5 py-0.5 rounded-full border border-jm-accent/30 flex items-center gap-1"><CheckCircle2 size={10} /> Уст.</span>}
                            </div>
                            <div className="text-[11px] text-[var(--text-secondary)] mt-0.5">Игра: {(v.game_versions || []).join(", ")}{isNoLoaderProject ? "" : ` | Ядро: ${loadersStr || "—"}`}</div>
                          </div>
                          <motion.button whileHover={{ scale: 1.05 }} whileTap={{ scale: 0.95 }} onClick={() => handleDownloadClick(v)} className={`px-4 py-1.5 rounded-lg font-bold text-xs transition-colors border flex items-center gap-1.5 shrink-0 self-end sm:self-auto ${isInstalled ? 'bg-blue-500/10 text-blue-400 border-blue-500/30 hover:bg-blue-500 hover:text-white' : 'bg-jm-accent/10 hover:bg-jm-accent text-jm-accent hover:text-black border-jm-accent/30'}`}>
                            {isInstalled ? <><RefreshCw size={14} /> Перест.</> : "Скачать"}
                          </motion.button>
                        </div>
                      );
                    }) : <div className="flex items-center justify-center h-40"><Loader2 className="animate-spin text-jm-accent" size={48} /></div>}
                  </div>
                    );
                  })() )}
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* МОДАЛКА ВЫБОРА СБОРКИ */}
      <AnimatePresence>
        {installTarget && !contextInstance && (
          <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="fixed inset-0 z-[60] bg-black/90 backdrop-blur-md flex items-center justify-center p-4">
            <motion.div initial={{ scale: 0.9, y: 20 }} animate={{ scale: 1, y: 0 }} exit={{ scale: 0.9, y: 20 }} className="bg-jm-card border border-jm-accent rounded-2xl p-5 w-full max-w-md shadow-[0_0_50px_rgba(134,168,134,0.2)]">
              <h3 className="text-xl font-bold text-white mb-2">Выберите сборку</h3>
              <p className="text-[var(--text-secondary)] text-xs mb-4">В какую сборку установить <strong className="text-white">{installTarget.filename}</strong>?</p>
              
              <div className="flex flex-col gap-2 max-h-[300px] overflow-y-auto custom-scrollbar pr-1 mb-4">
                {instances.map(inst => (
                  <motion.div whileHover={{ scale: 1.02 }} whileTap={{ scale: 0.98 }} key={inst.id} onClick={() => installIntoInstance(inst.id)} className="bg-black/50 border border-white/10 p-3 rounded-xl cursor-pointer hover:border-jm-accent hover:bg-jm-accent/10 transition-all flex justify-between items-center group shadow-md">
                    <div>
                      <div className="font-bold text-white text-sm">{inst.name}</div>
                      <div className="text-xs text-[var(--text-secondary)] capitalize">{inst.loader} {inst.game_version}</div>
                    </div>
                    <CheckCircle2 size={18} className="text-jm-accent opacity-0 group-hover:opacity-100 transition-opacity" />
                  </motion.div>
                ))}
                {instances.length === 0 && <div className="text-[var(--text-secondary)] text-center py-4 text-sm">У вас нет сборок. Создайте её во вкладке "Сборки".</div>}
              </div>
              <button onClick={() => setInstallTarget(null)} className="w-full py-2.5 rounded-xl font-bold text-sm bg-white/5 hover:bg-white/10 text-white transition-colors">Отмена</button>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* МОДАЛКА ВЫБОРА МИРА ДЛЯ ДАТАПАКА */}
      <AnimatePresence>
        {datapackTarget && (
          <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="fixed inset-0 z-[60] bg-black/90 backdrop-blur-md flex items-center justify-center p-8">
            <motion.div initial={{ scale: 0.9, y: 20 }} animate={{ scale: 1, y: 0 }} exit={{ scale: 0.9, y: 20 }} className="bg-jm-card border border-jm-accent rounded-3xl p-8 w-[500px] shadow-[0_0_50px_rgba(134,168,134,0.2)]">
              {datapackTarget.instanceId ? (
                <>
                  <h3 className="text-2xl font-bold text-white mb-2">Выберите мир</h3>
                  <p className="text-[var(--text-secondary)] text-sm mb-6">В какой мир установить <strong className="text-white">{datapackTarget.filename}</strong>?</p>
                  <div className="flex flex-col gap-3 max-h-[300px] overflow-y-auto custom-scrollbar pr-2 mb-6">
                    {worlds.map(world => (
                      <motion.div whileHover={{ scale: 1.02 }} whileTap={{ scale: 0.98 }} key={world} onClick={async () => {
                        try {
                          showToast(`Установка в ${world}...`);
                          await invoke("install_datapack", { instanceId: datapackTarget.instanceId, worldName: world, url: datapackTarget.url, filename: datapackTarget.filename });
                          showToast(`Датапак установлен в мир ${world}!`);
                          setDatapackTarget(null);
                        } catch (e) { showToast(`Ошибка: ${e}`); }
                      }} className="bg-black/50 border border-white/10 p-4 rounded-xl cursor-pointer hover:border-jm-accent hover:bg-jm-accent/10 transition-all flex justify-between items-center group shadow-md">
                        <div className="font-bold text-white text-lg">{world}</div>
                        <CheckCircle2 className="text-jm-accent opacity-0 group-hover:opacity-100 transition-opacity" />
                      </motion.div>
                    ))}
                    {worlds.length === 0 && <div className="text-[var(--text-secondary)] text-center py-4">Нет миров. Запустите игру и создайте мир.</div>}
                  </div>
                </>
              ) : (
                <>
                  <h3 className="text-2xl font-bold text-white mb-2">Выберите сборку</h3>
                  <p className="text-[var(--text-secondary)] text-sm mb-6">В какую сборку установить датапак?</p>
                  <div className="flex flex-col gap-3 max-h-[300px] overflow-y-auto custom-scrollbar pr-2 mb-6">
                    {instances.map(inst => (
                      <motion.div whileHover={{ scale: 1.02 }} whileTap={{ scale: 0.98 }} key={inst.id} onClick={async () => {
                        const w: any = await invoke("list_worlds", { instanceId: inst.id }).catch(() => []);
                        setWorlds(w || []);
                        setDatapackTarget({ ...datapackTarget, instanceId: inst.id });
                      }} className="bg-black/50 border border-white/10 p-4 rounded-xl cursor-pointer hover:border-jm-accent hover:bg-jm-accent/10 transition-all flex justify-between items-center group shadow-md">
                        <div>
                          <div className="font-bold text-white text-lg">{inst.name}</div>
                          <div className="text-xs text-[var(--text-secondary)] capitalize">{inst.loader} {inst.game_version}</div>
                        </div>
                        <CheckCircle2 className="text-jm-accent opacity-0 group-hover:opacity-100 transition-opacity" />
                      </motion.div>
                    ))}
                  </div>
                </>
              )}
              <button onClick={() => setDatapackTarget(null)} className="w-full py-3 rounded-xl font-bold bg-white/5 hover:bg-white/10 text-white transition-colors">Отмена</button>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Лайтбокс скриншотов (клик по плитке, стрелки / свайп клавиатуры) */}
      <AnimatePresence>
        {galleryLightbox && galleryLightbox.urls.length > 0 && (
          <motion.div
            role="dialog"
            aria-modal
            aria-label="Просмотр скриншота"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
            className="fixed inset-0 z-[200] flex items-center justify-center bg-black/90 backdrop-blur-xl p-4 sm:p-10"
            onClick={() => setGalleryLightbox(null)}
          >
            <button
              type="button"
              className="absolute top-3 right-3 z-20 p-2.5 rounded-full bg-white/10 hover:bg-red-500/35 text-white transition-colors duration-200"
              onClick={(e) => {
                e.stopPropagation();
                setGalleryLightbox(null);
              }}
              aria-label="Закрыть"
            >
              <X size={22} />
            </button>
            <button
              type="button"
              className="absolute left-1 sm:left-4 top-1/2 -translate-y-1/2 z-20 p-3 rounded-full bg-white/10 hover:bg-jm-accent/25 text-white transition-colors duration-200"
              onClick={(e) => {
                e.stopPropagation();
                setGalleryLightbox((cur) => {
                  if (!cur?.urls.length) return cur;
                  const n = cur.urls.length;
                  return { ...cur, index: (cur.index - 1 + n) % n };
                });
              }}
              aria-label="Предыдущий"
            >
              <ChevronLeft size={28} />
            </button>
            <button
              type="button"
              className="absolute right-1 sm:right-4 top-1/2 -translate-y-1/2 z-20 p-3 rounded-full bg-white/10 hover:bg-jm-accent/25 text-white transition-colors duration-200"
              onClick={(e) => {
                e.stopPropagation();
                setGalleryLightbox((cur) => {
                  if (!cur?.urls.length) return cur;
                  return { ...cur, index: (cur.index + 1) % cur.urls.length };
                });
              }}
              aria-label="Следующий"
            >
              <ChevronRight size={28} />
            </button>
            <div className="max-w-[min(100%,96vw)] max-h-[88vh] flex items-center justify-center pointer-events-none">
              <AnimatePresence mode="wait" initial={false}>
                <motion.img
                  key={`${galleryLightbox.index}-${galleryLightbox.urls[galleryLightbox.index]}`}
                  src={galleryLightbox.urls[galleryLightbox.index]}
                  alt=""
                  initial={{ opacity: 0, scale: 0.94, y: 8 }}
                  animate={{ opacity: 1, scale: 1, y: 0 }}
                  exit={{ opacity: 0, scale: 0.96, y: -6 }}
                  transition={{ type: "spring", stiffness: 420, damping: 34, mass: 0.65 }}
                  className="max-h-[85vh] max-w-full w-auto h-auto object-contain rounded-xl shadow-2xl border border-white/15 pointer-events-auto"
                  onClick={(e) => e.stopPropagation()}
                  draggable={false}
                />
              </AnimatePresence>
            </div>
            <div className="absolute bottom-4 left-1/2 -translate-x-1/2 text-xs font-medium text-white/70 bg-black/55 px-3 py-1.5 rounded-full border border-white/10 tabular-nums">
              {galleryLightbox.index + 1} / {galleryLightbox.urls.length}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}