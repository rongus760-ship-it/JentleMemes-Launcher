<script lang="ts">
  import { fly, fade, scale } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import { onMount, onDestroy, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { showToast } from "../lib/jmEvents";
  import { portal } from "../lib/portalAction";
  import {
    sanitizeProjectBody,
    looksLikeHtml,
    CATEGORY_MAP,
    GAME_VERSION_OPTIONS,
    mergeGameVersionOptionStrings,
    titleInitials,
  } from "../lib/discoverHelpers";
  import DiscoverSelect from "../components/DiscoverSelect.svelte";
  import { marked } from "marked";
  import {
    Search,
    Download,
    ChevronLeft,
    ChevronRight,
    Loader2,
    Package,
    Image as ImageIcon,
    Sparkles,
    Layers,
    Link,
    X,
    AlignLeft,
    List,
    CheckCircle2,
    RefreshCw,
    ArrowDownWideNarrow,
    ArrowUpWideNarrow,
    Trash2,
    FolderDown,
    Info,
  } from "lucide-svelte";

  export let contextInstance: any = undefined;
  export let installedMods: string[] = [];
  /** Установленные моды с project_id (из вкладки сборки) */
  export let installedModSummaries: {
    project_id: string;
    filename: string;
    clean_name: string;
    version_id: string;
  }[] = [];
  /** Все project_id установленного контента (моды, РП, шейдеры) — скрыть «Установить» в браузере сборки */
  export let installedContentProjectIds: string[] = [];
  /** Встроенный браузер из вкладки «Добавить контент» */
  export let libraryEmbedMode = false;
  /** Открыт по клику на мод в списке контента — центральная модалка; иначе боковая панель */
  export let openDiscoverFromContentShortcut = false;
  export let onModsChanged: (() => void) | undefined = undefined;
  export let initialProjectId: string | undefined = undefined;
  /** После открытия проекта по диплинку — сбросить pending в App */
  export let onInitialProjectOpened: (() => void) | undefined = undefined;

  /** Узлы `use:portal` в body — при смене вкладки снимаем принудительно (иначе outro может оставить слой выше навигации). */
  const discoverPortalGroupId = `jm-disco-${Math.random().toString(36).slice(2, 11)}`;

  let query = "";
  let projectType = "mod";
  let gameVersion = "";
  let loader = "";
  let selectedCategories: string[] = [];
  let results: any[] = [];
  let isLoading = false;
  let page = 0;
  let pageInput = "1";
  let sortMode: "relevance" | "popularity" | "updated" | "downloads" | "name" | "author" | "rating" =
    "relevance";
  let sortDesc = true;
  let totalHits = 0;
  let galleryLightbox: { urls: string[]; index: number } | null = null;

  let selectedProject: any = null;
  let projectDetails: any = null;
  let projectVersions: any[] = [];
  /** Установка с карточки поиска без открытия панели */
  let suppressProjectPanel = false;
  let quickCardInstallBusy: string | null = null;
  let modalTab = "desc";

  let vFilter = "";
  let lFilter = "";

  /** Открытая карточка «описание версии» */
  let versionDetailRow: any = null;
  let versionDetailLoading = false;
  let versionDetailPayload: any = null;
  let saveVersionToFolderBusy = false;

  let localInstalledMods: string[] = Array.isArray(installedMods) ? [...installedMods] : [];
  /** Оптимистичный кэш только что установленных project_id — до того, как
   *  refresh_mod_metadata пополнит sidecar и prop подтянет свежие id. */
  let optimisticInstalledIds: string[] = [];
  function markOptimisticInstalled(...ids: (string | null | undefined)[]) {
    const norm = ids.map((v) => String(v || "").trim()).filter(Boolean);
    if (!norm.length) return;
    const set = new Set(optimisticInstalledIds);
    for (const id of norm) set.add(id);
    optimisticInstalledIds = Array.from(set);
  }
  let installTarget: {
    url: string;
    filename: string;
    curseforgeProjectId?: string;
    curseforgeFileId?: string;
    projectTypeForInstall?: string;
  } | null = null;
  let instances: any[] = [];
  let datapackTarget: { url: string; filename: string; instanceId: string | null } | null = null;
  let worlds: string[] = [];
  let customPacks: any[] = [];
  let customPacksLoading = false;
  let modProvider: "modrinth" | "curseforge" | "hybrid" = "modrinth";
  let hybridProviderEnabled = false;
  let searchError: string | null = null;
  let showMcSnapshotVersions = false;
  let showMcAlphaBetaVersions = false;
  let enableAlphaDiscover = false;
  let uiGameVersionsFromApi: string[] = [];

  async function refreshUiGameVersions() {
    try {
      const v = await invoke("get_ui_game_versions", {
        includeSnapshots: showMcSnapshotVersions,
        includeAlphaBeta: showMcAlphaBetaVersions,
      });
      uiGameVersionsFromApi = Array.isArray(v) ? (v as string[]) : [];
    } catch {
      uiGameVersionsFromApi = [];
    }
  }

  function persistDiscoverMcFilters() {
    void invoke("load_settings")
      .then((s: any) =>
        invoke("save_settings", {
          settings: {
            ...s,
            show_mc_snapshot_versions: showMcSnapshotVersions,
            show_mc_alpha_beta_versions: showMcAlphaBetaVersions,
          },
        }),
      )
      .then(() => refreshUiGameVersions())
      .catch(() => {});
  }

  /** Сопоставление с метаданными установленных модов / контента */
  function projectIdCandidates(project: any): string[] {
    if (!project) return [];
    return [project.id, project.project_id, project.modrinth_id, project.curseforge_id]
      .filter((x) => x != null && String(x).trim() !== "")
      .map((x) => String(x));
  }

  /** Версия из списка относится к CurseForge (не к Modrinth API). */
  function isCurseForgeContentVersion(version: any): boolean {
    if (version?._source === "curseforge") return true;
    if (modProvider === "curseforge") return true;
    return false;
  }

  function curseforgeProjectIdForInstall(): string {
    const p = selectedProject;
    return String(p?.curseforge_id ?? p?.project_id ?? p?.id ?? "").trim();
  }

  function isInstalledContentProject(project: any): boolean {
    if (!contextInstance || !project) return false;
    const cand = projectIdCandidates(project);
    if (cand.length === 0) return false;
    return cand.some(
      (c) => installedContentProjectIds.includes(c) || optimisticInstalledIds.includes(c),
    );
  }

  /** Явные Modrinth/CurseForge id для pack_source.json (URL с CDN CF не парсится как Modrinth). */
  function mrpackInstallInvokeArgs(url: string, packTitle: string, project: any, version: any) {
    const vSrc = version?._source;
    const treatAsCf =
      modProvider === "curseforge" ||
      vSrc === "curseforge" ||
      (modProvider === "hybrid" && vSrc === "curseforge");
    if (treatAsCf) {
      const cfPid = String(
        project?.curseforge_id ||
          (vSrc === "curseforge" ? project?.project_id : "") ||
          project?.project_id ||
          "",
      );
      const fid = String(version?.id ?? "");
      return {
        url,
        name: packTitle,
        modrinthProjectId: null,
        modrinthVersionId: null,
        curseforgeProjectId: cfPid.trim() || null,
        curseforgeFileId: fid.trim() || null,
      };
    }
    const mrPid = String(project?.modrinth_id || project?.project_id || project?.id || "");
    const vid = String(version?.id ?? "");
    return {
      url,
      name: packTitle,
      modrinthProjectId: mrPid.trim() || null,
      modrinthVersionId: vid.trim() || null,
      curseforgeProjectId: null,
      curseforgeFileId: null,
    };
  }

  let lastContextId: string | undefined = undefined;
  $: {
    const id = contextInstance?.id;
    if (id !== lastContextId) {
      lastContextId = id;
      gameVersion = contextInstance?.game_version || "";
      loader =
        contextInstance?.loader === "vanilla" ? "" : contextInstance?.loader || "";
    }
  }

  $: {
    contextInstance;
    vFilter = contextInstance?.game_version || "";
    lFilter =
      contextInstance?.loader === "vanilla" ? "" : contextInstance?.loader || "";
  }

  $: localInstalledMods = Array.isArray(installedMods) ? [...installedMods] : [];

  const limit = 20;
  $: totalPages = Math.max(1, Math.ceil((totalHits || 0) / limit));

  $: loaderLocked = !!contextInstance && (projectType === "mod" || projectType === "modpack");

  $: if (contextInstance && projectType === "modpack") {
    projectType = "mod";
  }

  $: {
    projectType;
    contextInstance;
    selectedCategories = [];
    page = 0;
    pageInput = "1";
  }

  $: {
    const pt = projectType;
    const ci = contextInstance;
    if (pt === "resourcepack" || pt === "shader" || pt === "datapack") {
      loader = "";
    } else if (ci && ci.loader !== "vanilla") {
      loader = ci.loader;
    }
  }

  $: noLoaderTypes = ["resourcepack", "shader", "datapack"];
  $: typesList = (() => {
    const base: { id: string; label: string; icon: any }[] = [
      { id: "mod", label: "Моды", icon: Package },
      { id: "resourcepack", label: "РП", icon: ImageIcon },
      { id: "shader", label: "Шейдеры", icon: Sparkles },
      { id: "datapack", label: "Датапаки", icon: Layers },
    ];
    if (!contextInstance) {
      base.splice(1, 0, { id: "modpack", label: "Сборки", icon: Layers });
      base.push({ id: "custom", label: "Кастомные", icon: Link });
    }
    return base;
  })();

  $: providerOptions = [
    { value: "modrinth", label: "Modrinth" },
    { value: "curseforge", label: "CurseForge" },
    ...(hybridProviderEnabled ? [{ value: "hybrid", label: "Гибрид" }] : []),
  ];

  $: embeddedInstanceGameVersion =
    libraryEmbedMode && contextInstance?.game_version
      ? [String(contextInstance.game_version).trim()].filter((s) => s.length > 0)
      : [];
  $: mergedGameVersionStrings = mergeGameVersionOptionStrings(
    mergeGameVersionOptionStrings(GAME_VERSION_OPTIONS, uiGameVersionsFromApi),
    embeddedInstanceGameVersion,
  );

  $: gameVersionSelectOptions = [
    { value: "", label: "Любая" },
    ...mergedGameVersionStrings.map((v) => ({ value: v, label: v })),
  ];

  $: alphaLoaderDiscoverOpts = enableAlphaDiscover
    ? [
        { value: "liteloader", label: "LiteLoader (α)" },
        { value: "modloader", label: "ModLoader (α)" },
      ]
    : [];

  $: loaderSelectOptions = [
    { value: "", label: "Любое" },
    { value: "fabric", label: "Fabric" },
    { value: "forge", label: "Forge" },
    { value: "neoforge", label: "NeoForge" },
    { value: "quilt", label: "Quilt" },
    ...alphaLoaderDiscoverOpts,
  ];

  $: sortSelectOptions = [
    { value: "relevance", label: "Релевантность" },
    { value: "popularity", label: "Популярность" },
    { value: "downloads", label: "Скачивания" },
    { value: "updated", label: "Обновление" },
    { value: "rating", label: "Рейтинг" },
    { value: "name", label: "Имя" },
    { value: "author", label: "Автор" },
  ];

  $: versionTabLoaderOptions = [
    { value: "", label: "Любой" },
    { value: "fabric", label: "Fabric" },
    { value: "forge", label: "Forge" },
    { value: "neoforge", label: "NeoForge" },
    { value: "quilt", label: "Quilt" },
    ...alphaLoaderDiscoverOpts,
  ];

  $: uniqueGameVersionsFromProject = (() => {
    const set = new Set<string>();
    for (const v of projectVersions) {
      for (const gv of v?.game_versions || []) set.add(gv);
    }
    return Array.from(set).sort((a, b) => b.localeCompare(a, undefined, { numeric: true }));
  })();

  $: versionTabGameOptions = [
    { value: "", label: "Любая" },
    ...mergeGameVersionOptionStrings(mergedGameVersionStrings, uniqueGameVersionsFromProject).map(
      (gv) => ({ value: gv, label: gv }),
    ),
  ];

  function renderMarkdown(raw: string): string {
    try {
      const out = marked.parse(raw, { async: false });
      const html = typeof out === "string" ? out : "";
      return sanitizeProjectBody(html);
    } catch {
      return "";
    }
  }

  $: descRaw = projectDetails
    ? projectDetails.body || projectDetails.description || projectDetails.summary || "Нет описания"
    : "";
  $: descIsHtml = looksLikeHtml(descRaw);
  $: descRenderedHtml = descIsHtml ? sanitizeProjectBody(descRaw) : renderMarkdown(descRaw);

  $: galleryUrls = (() => {
    const rawGallery = projectDetails?.gallery ?? projectDetails?.screenshots ?? [];
    if (!Array.isArray(rawGallery)) return [];
    return rawGallery
      .map((x: any) => {
        if (!x) return null;
        if (typeof x === "string") return x;
        return x.url || x.thumbnailUrl || x.image_url || null;
      })
      .filter((u: unknown) => typeof u === "string" && (u as string).trim().length > 0) as string[];
  })();

  $: ptForVersions = (selectedProject?.project_type || projectType || "")
    .toLowerCase()
    .replace(/_/g, "");
  $: isNoLoaderProject = ["resourcepack", "shader", "datapack"].includes(ptForVersions);

  /** В браузере «Добавить контент» — боковая панель; клик по модy из списка контента — модалка */
  $: useModalProjectPanel =
    !!contextInstance && libraryEmbedMode && openDiscoverFromContentShortcut;

  $: portalChromeTopClass = "top-[var(--jm-chrome-stack,5.75rem)]";

  /** В браузере сборки панель выше оболочки (z 10056), иначе модалка оказывается под ней */
  $: projectPortalZClass = libraryEmbedMode ? "z-[10059]" : "z-[10055]";
  $: subModalZClass = libraryEmbedMode ? "z-[10062]" : "z-[10058]";

  $: installedSummary =
    contextInstance && projectType === "mod" && selectedProject
      ? installedModSummaries.find((s) =>
          projectIdCandidates(selectedProject).some((c) => c === s.project_id),
        )
      : undefined;

  let removeModBusy = false;

  function normalizeJarName(name: string): string {
    return (name || "")
      .replace(/\.disabled$/i, "")
      .replace(/\.(jar|zip)$/i, "");
  }

  function isVersionRowInstalled(v: any): boolean {
    const file = primaryOrFirstFile(v.files);
    const cn = normalizeJarName(file?.filename || "");
    if (!cn) return false;
    if (
      installedSummary?.version_id &&
      v?.id &&
      String(installedSummary.version_id) === String(v.id)
    ) {
      return true;
    }
    if (installedSummary?.clean_name) {
      const sn = normalizeJarName(installedSummary.clean_name);
      if (cn === sn) return true;
    }
    return localInstalledMods.includes(cn);
  }

  $: hasThisProjectInstalled =
    !!contextInstance &&
    projectType === "mod" &&
    (!!installedSummary || projectVersions.some((v) => isVersionRowInstalled(v)));

  function goVersionsTab() {
    modalTab = "versions";
  }

  async function removeInstalledModDisk() {
    if (!contextInstance || projectType !== "mod") return;
    removeModBusy = true;
    try {
      let filename = installedSummary?.filename;
      if (!filename) {
        const row = projectVersions.find((v) => isVersionRowInstalled(v));
        const f = primaryOrFirstFile(row?.files);
        const want = normalizeJarName(f?.filename || "");
        if (!want) {
          showToast("Не найдена установленная версия");
          return;
        }
        const mods: any[] = await invoke("get_installed_content", {
          instanceId: contextInstance.id,
          folder: "mods",
          includeFileHashes: false,
        });
        const hit = mods.find(
          (m) =>
            normalizeJarName(m.clean_name) === want ||
            normalizeJarName(m.filename) === want,
        );
        filename = hit?.filename;
      }
      if (!filename) {
        showToast("Не найден файл мода в сборке");
        return;
      }
      await invoke("delete_mod", {
        instanceId: contextInstance.id,
        filename,
        folder: "mods",
      });
      showToast("Мод удалён");
      await invoke("refresh_mod_metadata", { instanceId: contextInstance.id });
      onModsChanged?.();
      selectedProject = null;
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    } finally {
      removeModBusy = false;
    }
  }

  $: isPackPage =
    (selectedProject?.project_type || "").toLowerCase().replace(/_/g, "") === "modpack" ||
    ptForVersions === "modpack";

  $: filteredProjectVersions = projectVersions.filter((v: any) => {
    if (!v) return false;
    const gv = v.game_versions || [];
    const ld = v.loaders || [];
    const wantGV = vFilter;
    const wantLoader = lFilter;
    if (wantGV && !gv.includes(wantGV)) return false;
    if (!isNoLoaderProject && wantLoader && !ld.includes(wantLoader)) return false;
    return true;
  });

  let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  let prevPageTracked: number | undefined = undefined;

  async function fetchProjects() {
    if (projectType === "custom") return;
    isLoading = true;
    searchError = null;
    try {
      const queryTrim = (query || "").trim();
      const params = {
        query: queryTrim,
        projectType: projectType || "mod",
        gameVersion: (gameVersion || "").trim(),
        loader: loader || "",
        categories: selectedCategories || [],
        page: page || 0,
        sort: sortMode,
        sortDesc,
      };
      const data: any =
        modProvider === "hybrid"
          ? await invoke("search_hybrid", params)
          : modProvider === "curseforge"
            ? await invoke("search_curseforge", params)
            : await invoke("search_modrinth", params);
      let hits = data?.hits || [];
      let didLocalFilter = false;
      if ((modProvider === "curseforge" || modProvider === "hybrid") && queryTrim) {
        const q = queryTrim.toLowerCase();
        hits = hits.filter((p: any) => {
          const t = (p?.title || "").toLowerCase();
          const a = (p?.author || "").toLowerCase();
          return t.includes(q) || a.includes(q);
        });
        didLocalFilter = true;
      }
      results = hits;
      totalHits = didLocalFilter ? hits.length : data?.total_hits || 0;
      const err = data?.error;
      if (err === "curseforge_no_api_key") searchError = "curseforge_no_api_key";
      else if (err === "curseforge_forbidden") searchError = "curseforge_forbidden";
      else searchError = null;
    } catch (e) {
      console.error(e);
    } finally {
      isLoading = false;
    }
  }

  function scheduleSearchDebounce() {
    if (projectType === "custom") return;
    if (searchDebounceTimer) clearTimeout(searchDebounceTimer);
    searchDebounceTimer = setTimeout(() => {
      searchDebounceTimer = null;
      page = 0;
      pageInput = "1";
      fetchProjects();
    }, 500);
  }

  $: {
    query;
    projectType;
    gameVersion;
    loader;
    selectedCategories;
    modProvider;
    sortMode;
    sortDesc;
    if (projectType !== "custom") scheduleSearchDebounce();
  }

  $: {
    page;
    projectType;
    const q = query;
    if (projectType === "custom") {
      prevPageTracked = page;
    } else if (prevPageTracked !== page) {
      prevPageTracked = page;
      pageInput = String(page + 1);
      if (page !== 0 || !q.trim()) fetchProjects();
    }
  }

  function handlePageKeydown(e: KeyboardEvent) {
    if (e.key !== "Enter") return;
    let p = parseInt(pageInput, 10) - 1;
    if (isNaN(p) || p < 0) p = 0;
    if (p >= totalPages) p = totalPages - 1;
    page = p;
  }

  function handlePageBlur() {
    const n = parseInt(pageInput, 10);
    if (!Number.isFinite(n) || n < 1) {
      pageInput = String(page + 1);
      return;
    }
    let p = n - 1;
    if (p >= totalPages) p = totalPages - 1;
    page = p;
  }

  function handlePageInput(e: Event) {
    const el = e.currentTarget;
    if (!(el instanceof HTMLInputElement)) return;
    const v = el.value.replace(/\D/g, "");
    pageInput = v;
  }

  async function openProject(project: any) {
    selectedProject = project;
    modalTab = "desc";
    projectDetails = null;
    projectVersions = [];
    try {
      const id = project.project_id;
      const modrinthId = project.modrinth_id ?? id;
      const curseforgeId = project.curseforge_id ?? null;
      if (modProvider === "hybrid") {
        let details: any = null;
        if (modrinthId) {
          details = await invoke("get_modrinth_project", { id: modrinthId });
          const hasDesc = !!(details?.body || details?.description || details?.summary);
          if (!hasDesc && curseforgeId) {
            details = await invoke("get_curseforge_project", { id: curseforgeId });
          }
        } else if (curseforgeId) {
          details = await invoke("get_curseforge_project", { id: curseforgeId });
        }
        projectDetails = details;
        const versions: any = await invoke("get_hybrid_versions", {
          modrinthId: modrinthId || null,
          curseforgeId,
        });
        projectVersions = Array.isArray(versions) ? versions : [];
      } else if (modProvider === "curseforge") {
        const details = await invoke("get_curseforge_project", { id });
        projectDetails = details;
        const versions: any = await invoke("get_curseforge_versions", { id });
        projectVersions = Array.isArray(versions) ? versions : [];
      } else {
        const details = await invoke("get_modrinth_project", { id: modrinthId });
        projectDetails = details;
        const versions: any = await invoke("get_modrinth_versions", { id: modrinthId });
        projectVersions = Array.isArray(versions) ? versions : [];
      }
    } catch (e) {
      console.error(e);
    }
  }

  async function fetchVersionsOnly(project: any): Promise<any[]> {
    const id = project.project_id;
    const modrinthId = project.modrinth_id ?? id;
    const curseforgeId = project.curseforge_id ?? null;
    try {
      if (modProvider === "hybrid") {
        const versions: any = await invoke("get_hybrid_versions", {
          modrinthId: modrinthId || null,
          curseforgeId,
        });
        return Array.isArray(versions) ? versions : [];
      }
      if (modProvider === "curseforge") {
        const versions: any = await invoke("get_curseforge_versions", { id });
        return Array.isArray(versions) ? versions : [];
      }
      const versions: any = await invoke("get_modrinth_versions", { id: modrinthId });
      return Array.isArray(versions) ? versions : [];
    } catch (e) {
      console.error(e);
      return [];
    }
  }

  async function quickInstallFromResultCard(project: any) {
    const pid = project?.project_id;
    if (!pid) {
      showToast("Нет id проекта");
      return;
    }
    if (projectType === "datapack") {
      showToast("Откройте карточку и выберите мир для датапака");
      return;
    }
    if (projectType === "modpack" && contextInstance) {
      showToast("Сборки ставятся отдельно — откройте карточку");
      return;
    }
    if (projectType === "custom") {
      showToast("Откройте карточку для установки");
      return;
    }
    quickCardInstallBusy = String(pid);
    try {
      const versions = await fetchVersionsOnly(project);
      if (versions.length === 0) {
        showToast("Нет версий — откройте карточку");
        return;
      }
      suppressProjectPanel = true;
      selectedProject = project;
      projectVersions = versions;
      await tick();
      const list =
        filteredProjectVersions.length > 0 ? filteredProjectVersions : projectVersions;
      const v = list[0];
      if (!v) {
        showToast("Нет версии под фильтры сборки — откройте карточку");
        selectedProject = null;
        projectVersions = [];
        suppressProjectPanel = false;
        return;
      }
      await handleDownloadClick(v);
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    } finally {
      selectedProject = null;
      projectVersions = [];
      suppressProjectPanel = false;
      quickCardInstallBusy = null;
    }
  }

  async function handleDownloadClick(version: any) {
    if (projectType === "datapack") {
      const file = version?.files?.find((f: any) => f.primary) || version?.files?.[0];
      if (!file) return showToast("Файл не найден");
      if (contextInstance) {
        const w: any = await invoke("list_worlds", { instanceId: contextInstance.id }).catch(() => []);
        worlds = w || [];
        datapackTarget = {
          url: file.url,
          filename: file.filename,
          instanceId: contextInstance.id,
        };
      } else {
        const insts: any = await invoke("get_instances");
        instances = insts || [];
        datapackTarget = { url: file.url, filename: file.filename, instanceId: null };
      }
      return;
    }
    if (projectType === "modpack") {
      const file = version?.files?.find((f: any) => f.primary) || version?.files?.[0];
      if (!file) return showToast("Файл не найден");
      try {
        const title = selectedProject?.title || "Modpack";
        showToast(`Установка сборки ${title}...`);
        await invoke(
          "install_mrpack_from_url",
          mrpackInstallInvokeArgs(file.url ?? "", title, selectedProject, version),
        );
        showToast("Сборка успешно установлена!");
      } catch (e) {
        showToast(`Ошибка: ${e}`);
      }
      return;
    }

    if (!contextInstance) {
      const insts: any = await invoke("get_instances");
      instances = insts || [];
      const file = version?.files?.find((f: any) => f.primary) || version?.files?.[0];
      if (!file) return showToast("Файл не найден");
      const cfPid = String(selectedProject?.curseforge_id ?? selectedProject?.project_id ?? "").trim();
      if (
        isCurseForgeContentVersion(version) &&
        (projectType === "mod" || projectType === "resourcepack" || projectType === "shader") &&
        cfPid
      ) {
        installTarget = {
          url: file.url || "",
          filename: file.filename,
          curseforgeProjectId: cfPid,
          curseforgeFileId: String(version.id ?? ""),
          projectTypeForInstall: projectType,
        };
      } else {
        installTarget = { url: file.url || "", filename: file.filename };
      }
      return;
    }

    const file = version?.files?.find((f: any) => f.primary) || version?.files?.[0];
    if (!file) return showToast("Файл не найден на сервере");

    if (projectType === "resourcepack" || projectType === "shader") {
      try {
        showToast(`Установка ${file.filename}...`);
        if (isCurseForgeContentVersion(version)) {
          const cfPid = curseforgeProjectIdForInstall();
          if (!cfPid) return showToast("Нет id проекта CurseForge");
          await invoke("install_curseforge_mod_file", {
            instanceId: contextInstance.id,
            curseforgeProjectId: cfPid,
            curseforgeFileId: String(version.id),
            filename: file.filename || null,
            projectType,
          });
        } else {
          await invoke("install_modrinth_file", {
            instanceId: contextInstance.id,
            url: file.url,
            filename: file.filename,
            projectType,
          });
        }
        await invoke("refresh_mod_metadata", { instanceId: contextInstance.id });
        showToast(`Успешно установлено!`);
        markOptimisticInstalled(...projectIdCandidates(selectedProject));
        onModsChanged?.();
      } catch (e) {
        showToast(`Ошибка: ${e}`);
      }
      return;
    }

    try {
      showToast(`Установка ${file.filename}...`);
      if (isCurseForgeContentVersion(version)) {
        const cfPid = curseforgeProjectIdForInstall();
        if (!cfPid) return showToast("Нет id проекта CurseForge");
        await invoke("install_curseforge_mod_file", {
          instanceId: contextInstance.id,
          curseforgeProjectId: cfPid,
          curseforgeFileId: String(version.id),
          filename: file.filename || null,
          projectType: "mod",
        });
      } else {
        await invoke("install_mod_with_dependencies", {
          instanceId: contextInstance.id,
          versionId: version.id,
          gameVersion: contextInstance.game_version,
          loader: contextInstance.loader,
        });
      }
      await invoke("refresh_mod_metadata", { instanceId: contextInstance.id });
      showToast(`Успешно установлено!`);

      const cleanName = (file.filename || "").replace(".jar", "").replace(".zip", "");
      if (cleanName) localInstalledMods = [...localInstalledMods, cleanName];
      markOptimisticInstalled(...projectIdCandidates(selectedProject));
      onModsChanged?.();
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  async function onDatapackChooseInstanceForWorlds(inst: any) {
    const w = await invoke("list_worlds", { instanceId: inst.id }).catch(() => []);
    worlds = Array.isArray(w) ? w : [];
    if (datapackTarget) datapackTarget = { ...datapackTarget, instanceId: inst.id };
  }

  async function installDatapackIntoWorld(world: string) {
    const dt = datapackTarget;
    if (!dt?.instanceId) return;
    try {
      showToast(`Установка в ${world}...`);
      await invoke("install_datapack", {
        instanceId: dt.instanceId,
        worldName: world,
        url: dt.url,
        filename: dt.filename,
      });
      showToast(`Датапак установлен в мир ${world}!`);
      datapackTarget = null;
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  let quickDownloadBusy = false;

  /** Быстрая загрузка: лучшая подходящая версия из списка (фильтр учитывается) */
  async function quickDownloadFirstVersion() {
    const list =
      filteredProjectVersions.length > 0 ? filteredProjectVersions : projectVersions;
    const v = list[0];
    if (!v) {
      showToast("Нет версий — откройте вкладку «Версии»");
      modalTab = "versions";
      return;
    }
    quickDownloadBusy = true;
    try {
      await handleDownloadClick(v);
    } finally {
      quickDownloadBusy = false;
    }
  }

  async function installIntoInstance(
    instanceId: string,
    url: string = installTarget?.url || "",
    filename: string = installTarget?.filename || ""
  ) {
    const t = installTarget;
    const useCf =
      t?.curseforgeProjectId &&
      t?.curseforgeFileId &&
      (!url || !String(url).trim());
    if (!filename && !useCf) return;
    try {
      showToast(`Скачивание ${filename || "файла"}...`);
      if (useCf) {
        await invoke("install_curseforge_mod_file", {
          instanceId,
          curseforgeProjectId: t!.curseforgeProjectId!,
          curseforgeFileId: t!.curseforgeFileId!,
          filename: filename || null,
          projectType: t?.projectTypeForInstall || projectType,
        });
      } else {
        if (!url || !filename) return;
        await invoke("install_modrinth_file", { instanceId, url, filename, projectType });
      }
      await invoke("refresh_mod_metadata", { instanceId });
      showToast(`Успешно установлено!`);
      const cleanName = (filename || "").replace(/\.(jar|zip)$/i, "");
      if (cleanName && !localInstalledMods.includes(cleanName)) {
        localInstalledMods = [...localInstalledMods, cleanName];
      }
      if (installTarget?.curseforgeProjectId) markOptimisticInstalled(installTarget.curseforgeProjectId);
      if (selectedProject) markOptimisticInstalled(...projectIdCandidates(selectedProject));
      onModsChanged?.();
      installTarget = null;
    } catch (e) {
      showToast(`Ошибка установки: ${e}`);
    }
  }

  async function saveModProvider(val: "modrinth" | "curseforge" | "hybrid") {
    modProvider = val;
    try {
      const s: any = await invoke("load_settings");
      await invoke("save_settings", { settings: { ...s, mod_provider: val } }).catch(() => {});
    } catch {
      /* ignore */
    }
  }

  function handleModProviderChange(v: string) {
    if (v === "modrinth" || v === "curseforge" || v === "hybrid") {
      saveModProvider(v);
    }
  }

  function versionRowKey(v: any, idx: number): string {
    return v?.id ? `${v.id}-${v._source || "m"}` : `v-${idx}`;
  }

  function primaryOrFirstFile(files: any[] | undefined) {
    return files?.find((f: { primary?: boolean }) => f.primary) || files?.[0];
  }

  function canShowVersionDetails(): boolean {
    return projectType === "mod" || projectType === "resourcepack" || projectType === "shader";
  }

  async function openVersionDetails(v: any) {
    if (!canShowVersionDetails() || !v?.id) return;
    versionDetailRow = v;
    versionDetailPayload = null;
    versionDetailLoading = true;
    try {
      if (isCurseForgeContentVersion(v)) {
        const mid = curseforgeProjectIdForInstall();
        if (!mid) {
          versionDetailPayload = { error: "Нет id проекта CurseForge" };
          return;
        }
        versionDetailPayload = await invoke("get_curseforge_file_details", {
          modId: mid,
          fileId: String(v.id),
        });
      } else {
        versionDetailPayload = await invoke("get_modrinth_version_details", {
          versionId: String(v.id),
        });
      }
    } catch (e) {
      versionDetailPayload = { error: String(e) };
    } finally {
      versionDetailLoading = false;
    }
  }

  function closeVersionDetails() {
    versionDetailRow = null;
    versionDetailPayload = null;
    versionDetailLoading = false;
  }

  async function saveOpenedVersionToFolder() {
    const v = versionDetailRow;
    if (!v) return;
    saveVersionToFolderBusy = true;
    try {
      const file = primaryOrFirstFile(v.files);
      const cfPid = curseforgeProjectIdForInstall();
      const isCf = isCurseForgeContentVersion(v);
      const path: string = await invoke("download_mod_to_user_folder", {
        downloadUrl: file?.url?.trim() ? file.url : null,
        modrinthVersionId: isCf ? null : String(v.id),
        curseforgeProjectId: isCf && cfPid ? cfPid : null,
        curseforgeFileId: isCf ? String(v.id) : null,
        filenameHint: file?.filename || null,
      });
      showToast(`Сохранено: ${path}`);
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    } finally {
      saveVersionToFolderBusy = false;
    }
  }

  function handleSortModeChange(v: string) {
    if (
      v === "relevance" ||
      v === "popularity" ||
      v === "updated" ||
      v === "downloads" ||
      v === "name" ||
      v === "author" ||
      v === "rating"
    ) {
      sortMode = v;
    }
  }

  function toggleCategory(cat: string) {
    if (selectedCategories.includes(cat)) {
      selectedCategories = selectedCategories.filter((c) => c !== cat);
    } else {
      selectedCategories = [...selectedCategories, cat];
    }
  }

  function galleryPrev() {
    if (!galleryLightbox?.urls.length) return;
    const n = galleryLightbox.urls.length;
    galleryLightbox = { ...galleryLightbox, index: (galleryLightbox.index - 1 + n) % n };
  }

  function galleryNext() {
    if (!galleryLightbox?.urls.length) return;
    galleryLightbox = {
      ...galleryLightbox,
      index: (galleryLightbox.index + 1) % galleryLightbox.urls.length,
    };
  }

  function onGalleryKeydown(e: KeyboardEvent) {
    if (!galleryLightbox) return;
    if (e.key === "Escape") {
      galleryLightbox = null;
      return;
    }
    if (e.key === "ArrowRight") {
      e.preventDefault();
      galleryNext();
    }
    if (e.key === "ArrowLeft") {
      e.preventDefault();
      galleryPrev();
    }
  }

  function onIconError(e: Event) {
    const t = e.currentTarget;
    if (!(t instanceof HTMLImageElement)) return;
    t.style.display = "none";
    t.nextElementSibling?.classList.remove("hidden");
  }

  function onShotError(e: Event) {
    const t = e.currentTarget;
    if (!(t instanceof HTMLImageElement)) return;
    t.style.display = "none";
  }

  let lastOpenedInitialProjectId: string | undefined = undefined;

  /** id из меты сборки: Modrinth slug/uuid или числовой CurseForge project id */
  async function openFromInitialProjectId(id: string) {
    const tid = String(id || "").trim();
    if (!tid) {
      return;
    }
    const looksLikeCurseForgeId = /^\d+$/.test(tid);
    try {
      if (looksLikeCurseForgeId) {
        const cf: any = await invoke("get_curseforge_project", { id: tid }).catch(() => null);
        if (cf && (cf.title || cf.project_id)) {
          selectedProject = {
            ...cf,
            project_id: String(cf.project_id ?? tid),
            curseforge_id: String(cf.project_id ?? tid),
          };
          projectDetails = cf;
          modalTab = "desc";
          const versions: any = await invoke("get_curseforge_versions", { id: tid });
          projectVersions = Array.isArray(versions) ? versions : [];
          return;
        }
      }

      let details: any = await invoke("get_modrinth_project", { id: tid }).catch(() => null);
      if (details && (details.title || details.project_id || details.id)) {
        const pid = String(details.project_id || details.id || tid);
        selectedProject = { ...details, project_id: pid, modrinth_id: pid };
        projectDetails = details;
        modalTab = "desc";
        const versions: any = await invoke("get_modrinth_versions", { id: pid });
        projectVersions = Array.isArray(versions) ? versions : [];
        return;
      }

      const cfFallback: any = await invoke("get_curseforge_project", { id: tid }).catch(() => null);
      if (cfFallback && (cfFallback.title || cfFallback.project_id)) {
        selectedProject = {
          ...cfFallback,
          project_id: String(cfFallback.project_id ?? tid),
          curseforge_id: String(cfFallback.project_id ?? tid),
        };
        projectDetails = cfFallback;
        modalTab = "desc";
        const versions: any = await invoke("get_curseforge_versions", { id: tid });
        projectVersions = Array.isArray(versions) ? versions : [];
      }
    } catch (e) {
      console.error(e);
    } finally {
      onInitialProjectOpened?.();
    }
  }

  $: if (!initialProjectId) lastOpenedInitialProjectId = undefined;

  $: if (initialProjectId && initialProjectId !== lastOpenedInitialProjectId) {
    lastOpenedInitialProjectId = initialProjectId;
    void openFromInitialProjectId(initialProjectId);
  }

  let prevProjectTypeForCustom = "";
  function runCustomPackLoads() {
    customPacksLoading = true;
    invoke("load_custom_packs_config")
      .then((cfg: any) => {
        // Встроенный список в custom_packs.json (массив или { packs/items } без url)
        if (Array.isArray(cfg)) {
          customPacks = cfg;
          customPacksLoading = false;
          return;
        }
        const url = typeof cfg?.url === "string" ? cfg.url.trim() : "";
        const embedded = cfg?.packs ?? cfg?.items;
        if (Array.isArray(embedded) && embedded.length > 0 && !url) {
          customPacks = embedded;
          customPacksLoading = false;
          return;
        }

        return invoke("get_custom_packs")
          .then((data: any) => {
            const arr = Array.isArray(data) ? data : data?.packs || data?.items || [];
            customPacks = Array.isArray(arr) ? arr : [];
          })
          .catch(() => {
            customPacks = [];
          })
          .then(() => {
            if (!url) {
              customPacksLoading = false;
              return;
            }
            return invoke("fetch_custom_packs", { url })
              .then((data: any) => {
                const arr = Array.isArray(data) ? data : data?.packs || data?.items || [];
                customPacks = Array.isArray(arr) ? arr : [];
              })
              .catch(() => {
                customPacks = [];
              })
              .finally(() => {
                customPacksLoading = false;
              });
          });
      })
      .catch(() => {
        customPacksLoading = false;
      });
  }

  $: {
    const pt = projectType;
    if (pt === "custom" && prevProjectTypeForCustom !== "custom") {
      prevProjectTypeForCustom = "custom";
      runCustomPackLoads();
    } else if (pt !== "custom") {
      prevProjectTypeForCustom = pt;
    }
  }

  onMount(() => {
    invoke("load_settings")
      .then((s: any) => {
        modProvider =
          s?.mod_provider === "curseforge" || s?.mod_provider === "hybrid"
            ? s.mod_provider
            : "modrinth";
        hybridProviderEnabled = !!s?.hybrid_provider_enabled;
        showMcSnapshotVersions = !!s?.show_mc_snapshot_versions;
        showMcAlphaBetaVersions = !!s?.show_mc_alpha_beta_versions;
        enableAlphaDiscover = !!s?.enable_alpha_loaders;
      })
      .then(() => refreshUiGameVersions())
      .catch(() => {});

    prevPageTracked = undefined;
    window.addEventListener("keydown", onGalleryKeydown);
  });

  onDestroy(() => {
    if (searchDebounceTimer) clearTimeout(searchDebounceTimer);
    window.removeEventListener("keydown", onGalleryKeydown);
    document
      .querySelectorAll(`[data-jm-discover-portal="${discoverPortalGroupId}"]`)
      .forEach((n) => n.parentNode?.removeChild(n));
  });
</script>

<div class="jm-container flex h-full">
  <div class="flex flex-col flex-grow min-w-0 h-full">
    <div class="ui-card !p-3 mb-2.5 flex flex-col gap-2.5 shrink-0">
      <!-- Верхний ряд: поиск + источник -->
      <div class="flex items-center gap-2">
        <div class="relative flex-1 min-w-0">
          <Search class="absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none text-[var(--text-secondary)]" size={15} strokeWidth={2} />
          <input
            type="text"
            placeholder="Поиск модов, ресурспаков, шейдеров…"
            bind:value={query}
            class="ui-input pl-9"
          />
        </div>
        <div class="shrink-0 w-[9.5rem]">
          <DiscoverSelect
            label="Источник"
            value={modProvider}
            options={providerOptions}
            onChange={handleModProviderChange}
          />
        </div>
      </div>

      <!-- Типы контента — крупные pill-tabs -->
      <div class="ui-seg w-full overflow-x-auto flex [&::-webkit-scrollbar]:hidden">
        {#each typesList as t (t.id)}
          <button
            type="button"
            on:click={() => (projectType = t.id)}
            class="ui-seg-item flex items-center gap-1.5 whitespace-nowrap"
            class:is-active={projectType === t.id}
          >
            <svelte:component this={t.icon} size={13} strokeWidth={2.2} />
            {t.label}
          </button>
        {/each}
      </div>

      <!-- Фильтры (версия / ядро / сортировка) -->
      <div class="flex flex-wrap gap-2 items-end">
        <DiscoverSelect
          label="Версия"
          value={gameVersion}
          disabled={!!contextInstance && projectType !== "datapack"}
          options={gameVersionSelectOptions}
          onChange={(v) => (gameVersion = v)}
        />
        {#if !noLoaderTypes.includes(projectType)}
          <DiscoverSelect
            label="Ядро"
            value={loader}
            disabled={loaderLocked}
            options={loaderSelectOptions}
            onChange={(v) => (loader = v)}
          />
        {/if}
        <DiscoverSelect
          label="Сортировка"
          value={sortMode}
          options={sortSelectOptions}
          onChange={handleSortModeChange}
        />
        <button
          type="button"
          title={sortDesc
            ? "По убыванию. Клик — по возрастанию"
            : "По возрастанию. Клик — по убыванию"}
          on:click={() => (sortDesc = !sortDesc)}
          class="ui-btn ui-btn-subtle ui-btn-icon"
          aria-label="Порядок сортировки"
        >
          {#if sortDesc}
            <ArrowDownWideNarrow size={14} strokeWidth={2.2} />
          {:else}
            <ArrowUpWideNarrow size={14} strokeWidth={2.2} />
          {/if}
        </button>
        <label class="flex items-center gap-1.5 cursor-pointer text-[11px] ml-auto" style:color="var(--text-secondary)">
          <input
            type="checkbox"
            class="ui-toggle"
            bind:checked={showMcSnapshotVersions}
            on:change={persistDiscoverMcFilters}
          />
          Снапшоты
        </label>
        <label class="flex items-center gap-1.5 cursor-pointer text-[11px]" style:color="var(--text-secondary)">
          <input
            type="checkbox"
            class="ui-toggle"
            bind:checked={showMcAlphaBetaVersions}
            on:change={persistDiscoverMcFilters}
          />
          Альфа / бета
        </label>
        {#if totalHits > 0}
          <span class="text-[11px]" style:color="var(--text-secondary)">
            Найдено: <strong class="font-semibold" style:color="var(--text)">{totalHits.toLocaleString("ru-RU")}</strong>
          </span>
        {/if}
      </div>

      <!-- Категории как горизонтальные чипы (вместо левого сайдбара) -->
      {#if (CATEGORY_MAP[projectType] || []).length > 0}
        <div class="pt-2 border-t border-[var(--border)] -mx-3 px-3 -mb-3 pb-3">
          <div class="flex flex-wrap gap-1.5">
            {#each CATEGORY_MAP[projectType] || [] as cat (cat)}
              {@const active = selectedCategories.includes(cat)}
              <button
                type="button"
                on:click={() => toggleCategory(cat)}
                class="px-2.5 py-1 rounded-full border text-[11px] leading-none transition-all whitespace-nowrap {active
                  ? 'bg-[var(--accent-soft)] border-[var(--accent)]/60 text-[var(--accent-light)] font-semibold'
                  : 'bg-transparent border-[var(--border)] text-[var(--text-secondary)] hover:border-[var(--border-strong)] hover:text-[var(--text)]'}"
              >
                {cat.charAt(0).toUpperCase() + cat.slice(1)}
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </div>

    {#if totalHits > limit}
      <div class="mb-3 flex justify-center">
        <div class="ui-card flex items-center gap-1 px-2 py-1.5">
          <button
            type="button"
            on:pointerdown|stopPropagation={() => (page = Math.max(0, page - 1))}
            disabled={page === 0}
            class="ui-btn ui-btn-ghost ui-btn-icon"
            aria-label="Предыдущая страница"
          >
            <ChevronLeft size={14} strokeWidth={2.2} />
          </button>
          <div class="flex items-center gap-1.5 text-xs px-2" style:color="var(--text-secondary)">
            <span>Стр.</span>
            <input
              type="text"
              inputmode="numeric"
              value={pageInput}
              on:input={handlePageInput}
              on:keydown={handlePageKeydown}
              on:blur={handlePageBlur}
              class="ui-input w-12 text-center px-1 py-1 text-xs"
            />
            <span>из {totalPages}</span>
          </div>
          <button
            type="button"
            on:pointerdown|stopPropagation={() => (page = Math.min(totalPages - 1, page + 1))}
            disabled={page >= totalPages - 1}
            class="ui-btn ui-btn-ghost ui-btn-icon"
            aria-label="Следующая страница"
          >
            <ChevronRight size={14} strokeWidth={2.2} />
          </button>
        </div>
      </div>
    {/if}

    <div class="flex-grow overflow-y-auto custom-scrollbar pr-2 relative">
      {#if isLoading || (projectType === "custom" && customPacksLoading)}
        <div
          class="absolute inset-0 flex items-center justify-center backdrop-blur-sm z-10 rounded-[var(--radius-lg)] pointer-events-none"
          style:background="color-mix(in srgb, var(--bg) 55%, transparent)"
        >
          <Loader2 class="animate-spin text-[var(--accent-light)]" size={32} strokeWidth={2} />
        </div>
      {/if}
      {#if projectType === "custom"}
        <div
          class="grid gap-2.5 pb-6"
          style="grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));"
        >
          {#if customPacks.length === 0 && !customPacksLoading}
            <div class="col-span-full text-center py-12">
              <p class="text-sm" style:color="var(--text-secondary)">Укажите URL в настройках или загрузка не удалась.</p>
            </div>
          {:else}
            {#each customPacks as pack, idx (pack.id || idx)}
              {@const title = pack.title || pack.name || "Без названия"}
              {@const url = pack.url || pack.mrpack_url || pack.download_url}
              <div class="ui-card p-4 flex flex-col gap-3 group">
                <div class="flex items-start gap-3">
                  {#if pack.icon_url}
                    <img
                      src={pack.icon_url}
                      alt=""
                      class="w-14 h-14 rounded-[var(--radius)] object-cover border border-[var(--border)] shrink-0"
                      style:background="var(--surface-1)"
                      on:error={onIconError}
                    />
                    <div
                      class="hidden w-14 h-14 rounded-[var(--radius)] border border-[var(--border)] flex items-center justify-center text-sm font-medium shrink-0"
                      style:background="var(--surface-1)"
                      style:color="var(--text-secondary)"
                    >
                      {titleInitials(title)}
                    </div>
                  {:else}
                    <div
                      class="w-14 h-14 rounded-[var(--radius)] border border-[var(--border)] flex items-center justify-center text-sm font-medium shrink-0"
                      style:background="var(--surface-1)"
                      style:color="var(--text-secondary)"
                    >
                      {titleInitials(title)}
                    </div>
                  {/if}
                  <div class="flex-grow min-w-0">
                    <h3 class="font-semibold text-sm truncate group-hover:text-[var(--accent-light)] transition-colors">
                      {title}
                    </h3>
                    <p class="text-[11px] truncate mt-0.5" style:color="var(--text-secondary)">
                      от {pack.author || "Неизвестен"}
                    </p>
                  </div>
                </div>
                <p class="text-xs line-clamp-2" style:color="var(--text-secondary)">
                  {pack.description || ""}
                </p>
                <button
                  type="button"
                  on:click={async () => {
                    if (!url) return showToast("Нет ссылки на сборку");
                    try {
                      showToast(`Установка ${title}...`);
                      await invoke("install_mrpack_from_url", {
                        url,
                        name: title,
                        modrinthProjectId: null,
                        modrinthVersionId: null,
                        curseforgeProjectId: null,
                        curseforgeFileId: null,
                      });
                      showToast("Сборка установлена!");
                    } catch (e) {
                      showToast(`Ошибка: ${e}`);
                    }
                  }}
                  disabled={!url}
                  class="ui-btn ui-btn-primary ui-btn-sm w-full mt-auto"
                >
                  <Download size={13} strokeWidth={2.2} /> Установить
                </button>
              </div>
            {/each}
          {/if}
        </div>
      {:else if !isLoading && results.length === 0 && searchError && (modProvider === "curseforge" || modProvider === "hybrid")}
        <div class="flex flex-col items-center justify-center py-16 px-6 text-center gap-2">
          <p class="text-sm" style:color="var(--text-secondary)">
            {searchError === "curseforge_no_api_key"
              ? "Для CurseForge нужен API ключ"
              : "CurseForge отклонил запрос (лимит, блокировка IP/VPN или ключ в настройках неверный)"}
          </p>
          <p class="text-xs" style:color="var(--accent-light)">Настройки → Расширенные → CurseForge API key</p>
          <p class="text-[11px]" style:color="var(--text-secondary)">
            Если поле ключа заполнено — попробуйте очистить его (будет встроенный ключ). Свой ключ:
            <code class="font-mono">console.curseforge.com</code>
          </p>
        </div>
      {:else if !isLoading && results.length === 0}
        <div class="flex flex-col items-center justify-center py-16 px-6 text-center gap-2">
          <Search size={28} class="opacity-30" strokeWidth={1.6} />
          <p class="text-sm" style:color="var(--text-secondary)">Ничего не найдено. Попробуйте изменить фильтры или запрос.</p>
        </div>
      {:else}
        <div
          class="grid gap-2.5 pb-4"
          style="grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));"
        >
          {#each results as project (project.project_id)}
            <div class="ui-card !p-2.5 flex flex-col group transition-all hover:border-[var(--border-strong)]">
              <button
                type="button"
                class="text-left flex flex-col flex-1 min-h-0 outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)]"
                on:click={() => openProject(project)}
              >
                <div class="flex items-start gap-3 mb-2">
                  {#if project?.icon_url}
                    <img
                      src={project.icon_url}
                      alt=""
                      class="w-11 h-11 md:w-12 md:h-12 rounded-[var(--radius)] object-cover border border-[var(--border)] shrink-0"
                      style:background="var(--surface-1)"
                      on:error={onIconError}
                    />
                    <div
                      class="hidden w-11 h-11 md:w-12 md:h-12 rounded-[var(--radius)] border border-[var(--border)] items-center justify-center text-xs font-medium shrink-0"
                      style:background="var(--surface-1)"
                      style:color="var(--text-secondary)"
                    >
                      {titleInitials(project?.title || "?")}
                    </div>
                  {:else}
                    <div
                      class="w-11 h-11 md:w-12 md:h-12 rounded-[var(--radius)] border border-[var(--border)] flex items-center justify-center text-xs font-medium shrink-0"
                      style:background="var(--surface-1)"
                      style:color="var(--text-secondary)"
                    >
                      {titleInitials(project?.title || "?")}
                    </div>
                  {/if}
                  <div class="flex-grow min-w-0">
                    <h3 class="font-semibold text-sm md:text-[14px] truncate transition-colors group-hover:text-[var(--accent-light)]">
                      {project?.title || "Без названия"}
                    </h3>
                    <p class="text-[11px] truncate mt-0.5" style:color="var(--text-secondary)">
                      от {project?.author || "Неизвестен"}
                    </p>
                  </div>
                </div>
                <p class="text-xs line-clamp-2" style:color="var(--text-secondary)">{project?.description || ""}</p>
              </button>
              {#if projectType !== "custom" && !(projectType === "modpack" && contextInstance) && projectType !== "datapack" && !(contextInstance && ["mod", "resourcepack", "shader"].includes(projectType) && isInstalledContentProject(project))}
                <button
                  type="button"
                  disabled={quickCardInstallBusy === String(project.project_id)}
                  on:click|stopPropagation={() => quickInstallFromResultCard(project)}
                  class="ui-btn ui-btn-subtle ui-btn-sm w-full mt-3"
                >
                  {#if quickCardInstallBusy === String(project.project_id)}
                    <Loader2 size={13} class="animate-spin" strokeWidth={2.2} />
                    <span>Установка…</span>
                  {:else}
                    <Download size={13} strokeWidth={2.2} />
                    Установить
                  {/if}
                </button>
              {:else if contextInstance && ["mod", "resourcepack", "shader"].includes(projectType) && isInstalledContentProject(project)}
                <div class="mt-3 flex items-center justify-center gap-1.5 text-[11px] py-2 rounded-[var(--radius)]" style:background="color-mix(in srgb, #22c55e 12%, transparent)" style:color="#86efac">
                  <CheckCircle2 size={12} strokeWidth={2.5} /> Уже установлено
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>

    {#if totalHits > limit && projectType !== "custom"}
      <div class="mt-3 flex justify-center">
        <div class="ui-card flex items-center gap-1 px-2 py-1.5">
          <button
            type="button"
            on:pointerdown|stopPropagation={() => (page = Math.max(0, page - 1))}
            disabled={page === 0}
            class="ui-btn ui-btn-ghost ui-btn-icon"
            aria-label="Предыдущая страница"
          >
            <ChevronLeft size={14} strokeWidth={2.2} />
          </button>
          <div class="flex items-center gap-1.5 text-xs px-2" style:color="var(--text-secondary)">
            <span>Стр.</span>
            <input
              type="text"
              inputmode="numeric"
              value={pageInput}
              on:input={handlePageInput}
              on:keydown={handlePageKeydown}
              on:blur={handlePageBlur}
              class="ui-input w-12 text-center px-1 py-1 text-xs"
            />
            <span>из {totalPages}</span>
          </div>
          <button
            type="button"
            on:pointerdown|stopPropagation={() => (page = Math.min(totalPages - 1, page + 1))}
            disabled={page >= totalPages - 1}
            class="ui-btn ui-btn-ghost ui-btn-icon"
            aria-label="Следующая страница"
          >
            <ChevronRight size={14} strokeWidth={2.2} />
          </button>
        </div>
      </div>
    {/if}
  </div>

  {#if selectedProject && !suppressProjectPanel}
    <div
      use:portal
      data-jm-discover-portal={discoverPortalGroupId}
      class="fixed left-0 right-0 bottom-0 {projectPortalZClass} flex {portalChromeTopClass} {useModalProjectPanel
        ? 'items-center justify-center p-3 md:p-4 bg-black/80 backdrop-blur-lg'
        : ''}"
      transition:fade={{ duration: 260 }}
      role="presentation"
      on:click={() => useModalProjectPanel && (selectedProject = null)}
    >
      {#if !useModalProjectPanel}
        <button
          type="button"
          class="hidden sm:block flex-1 min-w-0 bg-transparent shrink border-0 p-0 cursor-pointer"
          aria-label="Закрыть"
          on:click={() => (selectedProject = null)}
        />
      {/if}
      <div
        role="dialog"
        aria-modal="true"
        class="{useModalProjectPanel
          ? 'w-full max-w-2xl max-h-[min(88vh,calc(100vh-var(--jm-chrome-stack,5.75rem)-1.5rem))] rounded-[var(--radius-lg)] border border-[var(--border-strong)] shadow-[0_24px_90px_rgba(0,0,0,0.5)] flex flex-col overflow-hidden bg-jm-bg/98 relative'
          : 'w-full sm:w-[620px] md:w-[720px] lg:w-[820px] xl:w-[920px] 2xl:w-[1040px] h-full bg-jm-bg border-l border-[var(--border)] shadow-[-12px_0_32px_-16px_rgba(0,0,0,0.45)] flex flex-col overflow-hidden relative shrink-0'}"
        on:click|stopPropagation
        in:fly={{
          x: useModalProjectPanel ? 0 : 380,
          y: useModalProjectPanel ? 14 : 0,
          duration: useModalProjectPanel ? 260 : 280,
          easing: quintOut,
        }}
        out:fly={{
          x: useModalProjectPanel ? 0 : 380,
          y: useModalProjectPanel ? 14 : 0,
          duration: useModalProjectPanel ? 200 : 220,
          easing: quintOut,
        }}
      >
        <div
          class="p-4 md:p-5 border-b border-[var(--border)] flex flex-col gap-3 shrink-0 relative bg-[var(--surface-1)]"
        >
          <div class="flex flex-col sm:flex-row items-start sm:items-center gap-3 md:gap-4">
            {#if selectedProject?.icon_url}
              <img
                src={selectedProject.icon_url}
                alt=""
                class="w-14 h-14 md:w-16 md:h-16 rounded-[var(--radius)] object-cover border border-[var(--border)] shrink-0"
                style:background="var(--surface-1)"
                on:error={onIconError}
              />
              <div
                class="hidden w-14 h-14 md:w-16 md:h-16 rounded-[var(--radius)] border border-[var(--border)] items-center justify-center text-sm font-medium shrink-0"
                style:background="var(--surface-1)"
                style:color="var(--text-secondary)"
              >
                {titleInitials(selectedProject?.title || "?")}
              </div>
            {:else}
              <div
                class="w-14 h-14 md:w-16 md:h-16 rounded-[var(--radius)] border border-[var(--border)] flex items-center justify-center text-sm font-medium shrink-0"
                style:background="var(--surface-1)"
                style:color="var(--text-secondary)"
              >
                {titleInitials(selectedProject?.title || "?")}
              </div>
            {/if}
            <div class="flex-grow min-w-0">
              <h2 class="text-lg md:text-xl font-semibold truncate leading-tight">{selectedProject?.title}</h2>
              {#if selectedProject?.author}
                <p class="text-xs truncate mt-0.5" style:color="var(--text-secondary)">
                  от {selectedProject.author}
                </p>
              {/if}
            </div>
            <div class="flex flex-row items-center gap-2 sm:ml-auto shrink-0 flex-wrap justify-end">
              {#if !isPackPage}
                {#if contextInstance && projectType === "mod" && hasThisProjectInstalled && !useModalProjectPanel}
                  <button
                    type="button"
                    disabled={removeModBusy}
                    on:click={removeInstalledModDisk}
                    class="ui-btn ui-btn-danger ui-btn-sm"
                    title="Удалить мод из сборки"
                  >
                    {#if removeModBusy}
                      <Loader2 size={13} class="animate-spin" strokeWidth={2.2} />
                    {:else}
                      <Trash2 size={13} strokeWidth={2.2} />
                    {/if}
                    <span class="hidden sm:inline">Удалить</span>
                  </button>
                  <button
                    type="button"
                    on:click={goVersionsTab}
                    class="ui-btn ui-btn-primary ui-btn-sm"
                    title="Выбрать другую версию на вкладке «Версии»"
                  >
                    <RefreshCw size={13} strokeWidth={2.2} />
                    <span class="hidden sm:inline">Заменить</span>
                  </button>
                {:else}
                  <button
                    type="button"
                    disabled={quickDownloadBusy || projectVersions.length === 0}
                    on:click={quickDownloadFirstVersion}
                    class="ui-btn ui-btn-primary"
                    title="Скачать первую подходящую версию (учитываются фильтры на вкладке «Версии»)"
                  >
                    {#if quickDownloadBusy}
                      <Loader2 size={14} class="animate-spin" strokeWidth={2.2} />
                      <span>Загрузка…</span>
                    {:else}
                      <Download size={14} strokeWidth={2.2} />
                      <span>Загрузить</span>
                    {/if}
                  </button>
                {/if}
              {/if}
              <button
                type="button"
                on:click={() => (selectedProject = null)}
                class="ui-btn ui-btn-ghost ui-btn-icon"
                aria-label="Закрыть"
              >
                <X size={14} strokeWidth={2.2} />
              </button>
            </div>
          </div>
          <div class="ui-seg w-fit">
            <button
              type="button"
              on:click={() => (modalTab = "desc")}
              class="ui-seg-item flex items-center gap-1.5"
              class:is-active={modalTab === "desc"}
            >
              <AlignLeft size={12} strokeWidth={2.2} /> <span class="hidden sm:inline">Описание</span>
            </button>
            <button
              type="button"
              on:click={() => (modalTab = "versions")}
              class="ui-seg-item flex items-center gap-1.5"
              class:is-active={modalTab === "versions"}
            >
              <List size={12} strokeWidth={2.2} /> <span class="hidden sm:inline">Версии</span>
            </button>
            <button
              type="button"
              on:click={() => (modalTab = "screens")}
              class="ui-seg-item flex items-center gap-1.5"
              class:is-active={modalTab === "screens"}
            >
              <ImageIcon size={12} strokeWidth={2.2} /> <span class="hidden sm:inline">Скриншоты</span>
            </button>
          </div>
        </div>

        <div class="flex-1 min-h-0 overflow-y-auto p-3 md:p-5 custom-scrollbar relative flex flex-col">
          {#if modalTab === "desc"}
            {#if projectDetails}
              <div
                class="discover-md text-[var(--text-secondary)] prose prose-invert prose-lg max-w-none text-base leading-relaxed break-words [&_img]:max-w-full [&_img]:h-auto [&_a]:text-jm-accent [&_a]:underline"
              >
                {@html descRenderedHtml || "Нет описания"}
              </div>
              {#if isPackPage}
                <div class="mt-6 pt-4 border-t border-[var(--border)] flex flex-col gap-2">
                  <p class="text-[10px] font-semibold text-[var(--text-secondary)] uppercase tracking-wider">
                    Установка сборки
                  </p>
                  <button
                    type="button"
                    disabled={quickDownloadBusy || projectVersions.length === 0}
                    on:click={quickDownloadFirstVersion}
                    class="ui-btn ui-btn-primary w-full"
                    title="Скачать и установить .mrpack (первая подходящая версия)"
                  >
                    {#if quickDownloadBusy}
                      <Loader2 size={14} class="animate-spin shrink-0" strokeWidth={2.2} />
                      <span>Загрузка…</span>
                    {:else}
                      <Download size={14} class="shrink-0" strokeWidth={2.2} />
                      <span>Загрузить сборку</span>
                    {/if}
                  </button>
                </div>
              {:else if contextInstance && projectType === "mod"}
                <div class="mt-6 pt-4 border-t border-[var(--border)] flex flex-col gap-2">
                  <p class="text-[10px] font-semibold text-[var(--text-secondary)] uppercase tracking-wider">
                    {hasThisProjectInstalled ? "Установленный мод" : "Установка мода"}
                  </p>
                  {#if hasThisProjectInstalled}
                    <p class="text-xs text-[var(--text-secondary)] leading-relaxed">
                      Чтобы поставить другую версию, откройте вкладку «Версии» и нажмите «Заменить» у нужной
                      строки.
                    </p>
                    <div class="flex flex-col sm:flex-row gap-2">
                      {#if useModalProjectPanel}
                        <button
                          type="button"
                          disabled={removeModBusy}
                          on:click={removeInstalledModDisk}
                          class="ui-btn ui-btn-danger flex-1"
                        >
                          {#if removeModBusy}
                            <Loader2 size={14} class="animate-spin shrink-0" strokeWidth={2.2} />
                          {:else}
                            <Trash2 size={14} class="shrink-0" strokeWidth={2.2} />
                          {/if}
                          <span>Удалить из сборки</span>
                        </button>
                      {/if}
                      <button
                        type="button"
                        on:click={goVersionsTab}
                        class="ui-btn ui-btn-subtle flex-1"
                      >
                        <List size={14} class="shrink-0" strokeWidth={2.2} />
                        <span>К версиям</span>
                      </button>
                    </div>
                  {:else}
                    <button
                      type="button"
                      disabled={quickDownloadBusy || projectVersions.length === 0}
                      on:click={quickDownloadFirstVersion}
                      class="ui-btn ui-btn-primary w-full"
                      title="Скачать первую подходящую версию"
                    >
                      {#if quickDownloadBusy}
                        <Loader2 size={14} class="animate-spin shrink-0" strokeWidth={2.2} />
                        <span>Загрузка…</span>
                      {:else}
                        <Download size={14} class="shrink-0" strokeWidth={2.2} />
                        <span>Загрузить мод</span>
                      {/if}
                    </button>
                  {/if}
                </div>
              {/if}
            {:else}
              <div class="flex items-center justify-center h-40">
                <Loader2 class="animate-spin text-jm-accent" size={48} />
              </div>
            {/if}
          {:else if modalTab === "screens"}
            {#if projectDetails}
              {#if galleryUrls.length === 0}
                <div class="text-[var(--text-secondary)] text-center py-10">Скриншотов нет</div>
              {:else}
                <div class="columns-1 sm:columns-2 lg:columns-3 gap-3" style="column-fill: balance">
                  {#each galleryUrls as u, idx (u + "-" + idx)}
                    <button
                      type="button"
                      on:click={() => (galleryLightbox = { urls: galleryUrls, index: idx })}
                      class="mb-3 w-full break-inside-avoid rounded-xl overflow-hidden border border-white/10 bg-white/[0.04] hover:border-jm-accent/45 hover:shadow-lg hover:shadow-jm-accent/10 transition-all text-left block focus:outline-none focus-visible:ring-2 focus-visible:ring-jm-accent/60 jm-tap-scale"
                      transition:scale={{ duration: 220, start: 0.96, easing: quintOut }}
                    >
                      <img
                        src={u}
                        alt=""
                        class="w-full h-auto max-w-full align-bottom block"
                        loading="lazy"
                        decoding="async"
                        on:error={onShotError}
                      />
                    </button>
                  {/each}
                </div>
              {/if}
            {:else}
              <div class="flex items-center justify-center h-40">
                <Loader2 class="animate-spin text-jm-accent" size={48} />
              </div>
            {/if}
          {:else if modalTab === "versions"}
            <div class="flex flex-col gap-3 min-h-0 flex-1">
              <div
                class="shrink-0 flex gap-3 flex-wrap items-end bg-[var(--surface-1)] p-3 rounded-[var(--radius)] border border-[var(--border)]"
              >
                <DiscoverSelect
                  label="Версия"
                  value={vFilter}
                  disabled={false}
                  options={versionTabGameOptions}
                  onChange={(v) => (vFilter = v)}
                />
                {#if !isNoLoaderProject}
                  <DiscoverSelect
                    label="Загрузчик"
                    value={lFilter}
                    disabled={false}
                    options={versionTabLoaderOptions}
                    onChange={(v) => (lFilter = v)}
                  />
                {/if}
              </div>

              {#if projectVersions.length > 0}
                <div
                  class="min-h-0 flex-1 overflow-y-auto custom-scrollbar pr-1 -mr-0.5"
                >
                  <div
                    class="grid grid-cols-1 2xl:grid-cols-2 gap-2.5 [grid-auto-rows:1fr] content-start pb-1"
                  >
                {#each filteredProjectVersions as v, idx (versionRowKey(v, idx))}
                  {@const rowInstalled = isVersionRowInstalled(v)}
                  {@const loadersStr = (v.loaders || []).join(", ")}
                  <div
                    class="p-3 rounded-[var(--radius)] border flex flex-col min-h-[120px] h-full gap-2 justify-between transition-colors {rowInstalled
                      ? 'bg-[var(--accent-softer)] border-[var(--accent)]/35'
                      : 'bg-[var(--surface-2)] border-[var(--border)]'}"
                  >
                    {#if canShowVersionDetails()}
                      <!-- svelte-ignore a11y_click_events_have_key_events -->
                      <button
                        type="button"
                        class="min-w-0 flex-1 text-left rounded-[var(--radius-sm)] -m-1 p-1 hover:bg-[var(--surface-hover)] transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)]/50"
                        on:click={() => openVersionDetails(v)}
                        title="Описание версии, зависимости, скачать в папку"
                      >
                        <div
                          class="font-semibold text-[var(--text)] text-sm flex items-center gap-1.5 flex-wrap"
                        >
                          <Info size={12} class="text-[var(--accent)] shrink-0 opacity-80" strokeWidth={2.2} />
                          <span class="truncate">{v.name || "Версия"}</span>
                          {#if v._source}
                            <span
                              class="text-[9px] px-1.5 py-0.5 rounded-[var(--radius-sm)] border font-semibold leading-none {v._source === 'curseforge'
                                ? 'bg-orange-500/15 text-orange-300 border-orange-500/30'
                                : 'bg-blue-500/15 text-blue-300 border-blue-500/30'}"
                            >
                              {v._source === "curseforge" ? "CF" : "MR"}
                            </span>
                          {/if}
                          {#if rowInstalled}
                            <span
                              class="bg-[var(--accent-soft)] text-[var(--accent-light)] text-[9px] px-1.5 py-0.5 rounded-[var(--radius-sm)] border border-[var(--accent)]/35 flex items-center gap-1 font-semibold leading-none"
                            >
                              <CheckCircle2 size={10} /> В сборке
                            </span>
                          {/if}
                        </div>
                        <div class="text-[11px] text-[var(--text-secondary)] mt-1">
                          Игра: {(v.game_versions || []).join(", ")}{isNoLoaderProject
                            ? ""
                            : ` · Ядро: ${loadersStr || "—"}`}
                        </div>
                      </button>
                    {:else}
                      <div class="min-w-0 flex-1">
                        <div
                          class="font-semibold text-[var(--text)] text-sm flex items-center gap-1.5 flex-wrap"
                        >
                          <span class="truncate">{v.name || "Версия"}</span>
                          {#if v._source}
                            <span
                              class="text-[9px] px-1.5 py-0.5 rounded-[var(--radius-sm)] border font-semibold leading-none {v._source === 'curseforge'
                                ? 'bg-orange-500/15 text-orange-300 border-orange-500/30'
                                : 'bg-blue-500/15 text-blue-300 border-blue-500/30'}"
                            >
                              {v._source === "curseforge" ? "CF" : "MR"}
                            </span>
                          {/if}
                          {#if rowInstalled}
                            <span
                              class="bg-[var(--accent-soft)] text-[var(--accent-light)] text-[9px] px-1.5 py-0.5 rounded-[var(--radius-sm)] border border-[var(--accent)]/35 flex items-center gap-1 font-semibold leading-none"
                            >
                              <CheckCircle2 size={10} /> В сборке
                            </span>
                          {/if}
                        </div>
                        <div class="text-[11px] text-[var(--text-secondary)] mt-1">
                          Игра: {(v.game_versions || []).join(", ")}{isNoLoaderProject
                            ? ""
                            : ` · Ядро: ${loadersStr || "—"}`}
                        </div>
                      </div>
                    {/if}
                    <div class="flex flex-wrap gap-2 shrink-0 justify-end mt-auto pt-2 border-t border-[var(--border)]">
                      {#if contextInstance && projectType === "mod" && rowInstalled}
                        <button
                          type="button"
                          disabled={removeModBusy}
                          on:click={removeInstalledModDisk}
                          class="ui-btn ui-btn-danger ui-btn-sm"
                        >
                          {#if removeModBusy}
                            <Loader2 size={12} class="animate-spin" strokeWidth={2.2} />
                          {:else}
                            <Trash2 size={12} strokeWidth={2.2} />
                          {/if}
                          <span>Удалить</span>
                        </button>
                      {:else if contextInstance && projectType === "mod" && hasThisProjectInstalled}
                        <button
                          type="button"
                          on:click={() => handleDownloadClick(v)}
                          class="ui-btn ui-btn-subtle ui-btn-sm"
                        >
                          <RefreshCw size={12} strokeWidth={2.2} /> <span>Заменить</span>
                        </button>
                      {:else}
                        <button
                          type="button"
                          on:click={() => handleDownloadClick(v)}
                          class="ui-btn ui-btn-primary ui-btn-sm"
                        >
                          <Download size={12} strokeWidth={2.2} /> <span>Скачать</span>
                        </button>
                      {/if}
                    </div>
                  </div>
                {/each}
                  </div>
                </div>
              {:else}
                <div class="flex items-center justify-center h-40 shrink-0">
                  <Loader2 class="animate-spin text-jm-accent" size={48} />
                </div>
              {/if}
            </div>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  {#if versionDetailRow && canShowVersionDetails()}
    <div
      use:portal
      data-jm-discover-portal={discoverPortalGroupId}
      class="fixed left-0 right-0 bottom-0 z-[10064] bg-black/90 backdrop-blur-md flex items-center justify-center p-4 {portalChromeTopClass}"
      transition:fade={{ duration: 200 }}
      role="presentation"
      on:click={closeVersionDetails}
    >
      <div
        class="bg-jm-card border border-jm-accent rounded-2xl p-5 w-full max-w-2xl max-h-[min(85vh,720px)] overflow-y-auto custom-scrollbar shadow-[0_0_50px_rgba(134,168,134,0.2)]"
        in:scale={{ duration: 220, start: 0.96, easing: quintOut }}
        on:click|stopPropagation
        role="dialog"
        aria-modal="true"
      >
        <div class="flex items-start justify-between gap-3 mb-4">
          <div class="min-w-0">
            <h3 class="text-lg font-bold text-white leading-tight">
              {versionDetailRow.name || "Версия"}
            </h3>
            <p class="text-xs text-[var(--text-secondary)] mt-1">
              {#if isCurseForgeContentVersion(versionDetailRow)}
                CurseForge · файл {versionDetailRow.id}
              {:else}
                Modrinth · версия {versionDetailRow.id}
              {/if}
            </p>
          </div>
          <button
            type="button"
            on:click={closeVersionDetails}
            class="shrink-0 p-2 rounded-lg hover:bg-white/10 text-[var(--text-secondary)]"
            aria-label="Закрыть"
          >
            <X size={20} />
          </button>
        </div>

        {#if versionDetailLoading}
          <div class="flex justify-center py-16">
            <Loader2 class="animate-spin text-jm-accent" size={40} />
          </div>
        {:else if versionDetailPayload?.error}
          <p class="text-sm text-red-300">{versionDetailPayload.error}</p>
        {:else if versionDetailPayload}
          <div class="space-y-5 text-sm">
            {#if versionDetailPayload.changelog_html}
              <div>
                <h4 class="text-xs font-bold text-jm-accent uppercase tracking-wide mb-2">Изменения / описание</h4>
                <div class="text-[var(--text-secondary)] leading-relaxed version-changelog-html">
                  {@html sanitizeProjectBody(String(versionDetailPayload.changelog_html || ""))}
                </div>
              </div>
            {:else if versionDetailPayload.changelog}
              <div>
                <h4 class="text-xs font-bold text-jm-accent uppercase tracking-wide mb-2">Changelog</h4>
                {#if looksLikeHtml(versionDetailPayload.changelog)}
                  <div class="text-[var(--text-secondary)] leading-relaxed version-changelog-html">
                    {@html sanitizeProjectBody(String(versionDetailPayload.changelog))}
                  </div>
                {:else}
                  <div
                    class="text-[var(--text-secondary)] leading-relaxed prose prose-invert prose-sm max-w-none version-changelog-md"
                  >
                    {@html sanitizeProjectBody(
                      String(marked.parse(String(versionDetailPayload.changelog), { async: false })),
                    )}
                  </div>
                {/if}
              </div>
            {:else}
              <p class="text-xs text-[var(--text-secondary)]">Текст изменений у этой версии не указан.</p>
            {/if}

            {#if versionDetailPayload.required_dependencies?.length}
              <div>
                <h4 class="text-xs font-bold text-amber-200/90 uppercase tracking-wide mb-2">
                  Обязательные зависимости
                </h4>
                <ul class="list-disc list-inside space-y-1 text-[var(--text-secondary)] text-xs">
                  {#each versionDetailPayload.required_dependencies as dep (dep.title + (dep.mod_id ?? dep.project_id ?? ""))}
                    <li>{dep.title}{dep.mod_id != null ? ` (CF ${dep.mod_id})` : ""}{dep.project_id ? ` · ${dep.project_id}` : ""}</li>
                  {/each}
                </ul>
              </div>
            {/if}

            {#if versionDetailPayload.optional_dependencies?.length}
              <div>
                <h4 class="text-xs font-bold text-sky-300/90 uppercase tracking-wide mb-2">
                  Опциональные зависимости
                </h4>
                <ul class="list-disc list-inside space-y-1 text-[var(--text-secondary)] text-xs">
                  {#each versionDetailPayload.optional_dependencies as dep (dep.title + (dep.mod_id ?? dep.project_id ?? ""))}
                    <li>{dep.title}{dep.mod_id != null ? ` (CF ${dep.mod_id})` : ""}{dep.project_id ? ` · ${dep.project_id}` : ""}</li>
                  {/each}
                </ul>
              </div>
            {/if}

            {#if versionDetailPayload.other_dependencies?.length}
              <div>
                <h4 class="text-xs font-bold text-[var(--text-secondary)] uppercase tracking-wide mb-2">
                  Прочие связи
                </h4>
                <ul class="list-disc list-inside space-y-1 text-[var(--text-secondary)] text-xs">
                  {#each versionDetailPayload.other_dependencies as dep (dep.title + String(dep.relation_type ?? ""))}
                    <li>{dep.title} (тип {dep.relation_type})</li>
                  {/each}
                </ul>
              </div>
            {/if}
          </div>
        {/if}

        <div class="flex flex-col sm:flex-row gap-2 mt-6 pt-4 border-t border-white/10">
          <button
            type="button"
            disabled={saveVersionToFolderBusy || versionDetailLoading}
            on:click={saveOpenedVersionToFolder}
            class="flex-1 min-h-[44px] px-4 py-2.5 rounded-xl font-bold text-sm flex items-center justify-center gap-2 bg-jm-accent/15 hover:bg-jm-accent text-jm-accent hover:text-black border border-jm-accent/40 transition-colors disabled:opacity-45 jm-tap-scale"
          >
            {#if saveVersionToFolderBusy}
              <Loader2 size={18} class="animate-spin shrink-0" />
            {:else}
              <FolderDown size={18} class="shrink-0" />
            {/if}
            Скачать в папку на ПК…
          </button>
          <button
            type="button"
            on:click={closeVersionDetails}
            class="sm:w-auto w-full min-h-[44px] px-4 py-2.5 rounded-xl font-bold text-sm bg-white/5 hover:bg-white/10 text-white transition-colors"
          >
            Закрыть
          </button>
        </div>
      </div>
    </div>
  {/if}

  {#if installTarget && !contextInstance}
    <div
      use:portal
      data-jm-discover-portal={discoverPortalGroupId}
      class="fixed left-0 right-0 bottom-0 {subModalZClass} bg-black/90 backdrop-blur-md flex items-center justify-center p-4 {portalChromeTopClass}"
      transition:fade={{ duration: 200 }}
      role="presentation"
    >
      <div
        class="bg-jm-card border border-jm-accent rounded-2xl p-5 w-full max-w-md shadow-[0_0_50px_rgba(134,168,134,0.2)]"
        in:scale={{ duration: 220, start: 0.9, easing: quintOut }}
        out:scale={{ duration: 180, start: 0.92, easing: quintOut }}
      >
        <h3 class="text-xl font-bold text-white mb-2">Выберите сборку</h3>
        <p class="text-[var(--text-secondary)] text-xs mb-4">
          В какую сборку установить <strong class="text-white">{installTarget.filename}</strong>?
        </p>
        <div class="flex flex-col gap-2 max-h-[300px] overflow-y-auto custom-scrollbar pr-1 mb-4">
          {#each instances as inst (inst.id)}
            <button
              type="button"
              on:click={() => installIntoInstance(inst.id)}
              class="bg-black/50 border border-white/10 p-3 rounded-xl cursor-pointer hover:border-jm-accent hover:bg-jm-accent/10 transition-all flex justify-between items-center group shadow-md text-left w-full jm-tap-scale"
            >
              <div>
                <div class="font-bold text-white text-sm">{inst.name}</div>
                <div class="text-xs text-[var(--text-secondary)] capitalize">
                  {inst.loader}
                  {inst.game_version}
                </div>
              </div>
              <CheckCircle2
                size={18}
                class="text-jm-accent opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
              />
            </button>
          {/each}
          {#if instances.length === 0}
            <div class="text-[var(--text-secondary)] text-center py-4 text-sm">
              У вас нет сборок. Создайте её во вкладке "Сборки".
            </div>
          {/if}
        </div>
        <button
          type="button"
          on:click={() => (installTarget = null)}
          class="w-full py-2.5 rounded-xl font-bold text-sm bg-white/5 hover:bg-white/10 text-white transition-colors"
        >
          Отмена
        </button>
      </div>
    </div>
  {/if}

  {#if datapackTarget}
    <div
      use:portal
      data-jm-discover-portal={discoverPortalGroupId}
      class="fixed left-0 right-0 bottom-0 {subModalZClass} bg-black/90 backdrop-blur-md flex items-center justify-center p-8 {portalChromeTopClass}"
      transition:fade={{ duration: 200 }}
      role="presentation"
    >
      <div
        class="bg-jm-card border border-jm-accent rounded-3xl p-8 w-[500px] shadow-[0_0_50px_rgba(134,168,134,0.2)] max-w-full"
        in:scale={{ duration: 220, start: 0.9, easing: quintOut }}
        out:scale={{ duration: 180, start: 0.92, easing: quintOut }}
      >
        {#if datapackTarget.instanceId}
          <h3 class="text-2xl font-bold text-white mb-2">Выберите мир</h3>
          <p class="text-[var(--text-secondary)] text-sm mb-6">
            В какой мир установить <strong class="text-white">{datapackTarget.filename}</strong>?
          </p>
          <div class="flex flex-col gap-3 max-h-[300px] overflow-y-auto custom-scrollbar pr-2 mb-6">
            {#each worlds as world (world)}
              <button
                type="button"
                on:click={() => installDatapackIntoWorld(world)}
                class="bg-black/50 border border-white/10 p-4 rounded-xl cursor-pointer hover:border-jm-accent hover:bg-jm-accent/10 transition-all flex justify-between items-center group shadow-md text-left w-full jm-tap-scale"
              >
                <div class="font-bold text-white text-lg">{world}</div>
                <CheckCircle2
                  size={22}
                  class="text-jm-accent opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
                />
              </button>
            {/each}
            {#if worlds.length === 0}
              <div class="text-[var(--text-secondary)] text-center py-4">
                Нет миров. Запустите игру и создайте мир.
              </div>
            {/if}
          </div>
        {:else}
          <h3 class="text-2xl font-bold text-white mb-2">Выберите сборку</h3>
          <p class="text-[var(--text-secondary)] text-sm mb-6">В какую сборку установить датапак?</p>
          <div class="flex flex-col gap-3 max-h-[300px] overflow-y-auto custom-scrollbar pr-2 mb-6">
            {#each instances as inst (inst.id)}
              <button
                type="button"
                on:click={() => onDatapackChooseInstanceForWorlds(inst)}
                class="bg-black/50 border border-white/10 p-4 rounded-xl cursor-pointer hover:border-jm-accent hover:bg-jm-accent/10 transition-all flex justify-between items-center group shadow-md text-left w-full jm-tap-scale"
              >
                <div>
                  <div class="font-bold text-white text-lg">{inst.name}</div>
                  <div class="text-xs text-[var(--text-secondary)] capitalize">
                    {inst.loader}
                    {inst.game_version}
                  </div>
                </div>
                <CheckCircle2
                  size={22}
                  class="text-jm-accent opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
                />
              </button>
            {/each}
          </div>
        {/if}
        <button
          type="button"
          on:click={() => (datapackTarget = null)}
          class="w-full py-3 rounded-xl font-bold bg-white/5 hover:bg-white/10 text-white transition-colors"
        >
          Отмена
        </button>
      </div>
    </div>
  {/if}

  {#if galleryLightbox && galleryLightbox.urls.length > 0}
    <div
      use:portal
      data-jm-discover-portal={discoverPortalGroupId}
      role="dialog"
      aria-modal="true"
      aria-label="Просмотр скриншота"
      class="fixed left-0 right-0 bottom-0 {subModalZClass} flex items-center justify-center bg-black/90 backdrop-blur-xl p-4 sm:p-10 {portalChromeTopClass}"
      transition:fade={{ duration: 220 }}
      on:click={() => (galleryLightbox = null)}
    >
      <button
        type="button"
        class="absolute top-3 right-3 z-20 p-2.5 rounded-full bg-white/10 hover:bg-red-500/35 text-white transition-colors duration-200"
        on:click|stopPropagation={() => (galleryLightbox = null)}
        aria-label="Закрыть"
      >
        <X size={22} />
      </button>
      <button
        type="button"
        class="absolute left-1 sm:left-4 top-1/2 -translate-y-1/2 z-20 p-3 rounded-full bg-white/10 hover:bg-jm-accent/25 text-white transition-colors duration-200"
        on:click|stopPropagation={galleryPrev}
        aria-label="Предыдущий"
      >
        <ChevronLeft size={28} />
      </button>
      <button
        type="button"
        class="absolute right-1 sm:right-4 top-1/2 -translate-y-1/2 z-20 p-3 rounded-full bg-white/10 hover:bg-jm-accent/25 text-white transition-colors duration-200"
        on:click|stopPropagation={galleryNext}
        aria-label="Следующий"
      >
        <ChevronRight size={28} />
      </button>
      <div class="max-w-[min(100%,96vw)] max-h-[88vh] flex items-center justify-center pointer-events-none">
        {#key galleryLightbox.index + "-" + galleryLightbox.urls[galleryLightbox.index]}
          <img
            src={galleryLightbox.urls[galleryLightbox.index]}
            alt=""
            in:scale={{ duration: 280, start: 0.94, easing: quintOut }}
            out:fade={{ duration: 160 }}
            class="max-h-[85vh] max-w-full w-auto h-auto object-contain rounded-xl shadow-2xl border border-white/15 pointer-events-auto"
            on:click|stopPropagation={() => {}}
            draggable="false"
          />
        {/key}
      </div>
      <div
        class="absolute bottom-4 left-1/2 -translate-x-1/2 text-xs font-medium text-white/70 bg-black/55 px-3 py-1.5 rounded-full border border-white/10 tabular-nums"
      >
        {galleryLightbox.index + 1} / {galleryLightbox.urls.length}
      </div>
    </div>
  {/if}
</div>
