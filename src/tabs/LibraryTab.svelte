<script lang="ts">
  import { formatPlaytimeSeconds } from "../lib/libraryUtils";
  import { sortMcVersionsDesc } from "../lib/mcVersionSort";
  import { showToast } from "../lib/jmEvents";
  import { instanceIconSrc } from "../utils/instanceIcon";
  import FileBrowserModal from "../components/FileBrowserModal.svelte";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { listen } from "@tauri-apps/api/event";
  import LibrarySelect from "../components/library/LibrarySelect.svelte";
  import PackVersionList from "../components/library/PackVersionList.svelte";
  import ExportModal from "../components/library/ExportModal.svelte";
  import LoaderIcon from "../components/LoaderIcon.svelte";
  import DiscoverTab from "./DiscoverTab.svelte";
  import { portal } from "../lib/portalAction";
  import {
    Play,
    Plus,
    Trash2,
    PackageOpen,
    Archive,
    Loader2,
    Settings,
    Terminal,
    Puzzle,
    ArrowLeft,
    Square,
    Search,
    FolderOpen,
    RefreshCw,
    X,
    Wrench,
    Download,
    Globe,
    Server,
    ListTree,
    Sliders,
    Coffee,
    Copy,
    TextSelect,
    Pencil,
    LayoutGrid,
    List as ListIcon,
    Image as ImageIcon,
    Sparkles,
    AlertCircle,
  } from "lucide-svelte";
  import { fly, fade, scale } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import { onMount, onDestroy, tick } from "svelte";
  import { filterMojangManifestVersions } from "../lib/discoverHelpers";

  type ProgressPayload = {
    task_name: string;
    downloaded: number;
    total: number;
    instance_id?: string;
  };

  export let initialInstanceId: string | undefined = undefined;
  export let initialServerIp: string | undefined = undefined;
  export let initialWorldName: string | undefined = undefined;
  export let onInstanceOpened: (() => void) | undefined = undefined;
  export let onServerLaunchConsumed: (() => void) | undefined = undefined;
  export let onWorldLaunchConsumed: (() => void) | undefined = undefined;
  export let busyInstanceId: string | null = null;
  export let progress: ProgressPayload = { task_name: "", downloaded: 0, total: 0 };

  let instances: any[] = [];
  let activeAccount: any = null;

  let isCreating = false;
  let newName = "";
  let newLoader = "fabric";
  let newVersion = "1.20.1";
  let newLoaderVersion = "";
  let newIcon = "";

  let availableVersions: string[] = [];
  let availableLoaderVersions: string[] = [];
  let isLoadingVersions = false;
  let isLoadingLoaderVersions = false;
  /** Синхронизируются с настройками (`show_mc_*`) и влияют на списки версий везде в лаунчере. */
  let showMcSnapshotVersions = false;
  let showMcAlphaBetaVersions = false;

  function persistMcVersionFilters() {
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
      .catch(() => {});
  }

  let selectedInstance: any = null;
  let instanceTab = "content";
  let settingsSubTab = "general";

  let runningInstances: string[] = [];

  async function syncRunningInstancesFromRust() {
    try {
      const ids = (await invoke("get_running_instance_ids")) as string[];
      runningInstances = Array.isArray(ids) ? ids : [];
    } catch {
      /* ignore */
    }
  }
  let logs: string[] = [];
  /** Сброс лога при смене сборки (иначе строки от прошлой игры «липнут» к новой вкладке). */
  let lastLogInstanceId = "";
  let packUpdateInfo: any = null;
  let isCheckingPackUpdate = false;
  let isUpdatingPack = false;
  let logsEndEl: HTMLDivElement | undefined;
  let logsScrollEl: HTMLDivElement | undefined;

  function logsAsPlainText(): string {
    return logs.join("\n");
  }

  async function copyAllLogs() {
    const t = logsAsPlainText();
    if (!t) {
      showToast("Лог пуст");
      return;
    }
    try {
      await navigator.clipboard.writeText(t);
      showToast("Весь лог скопирован");
    } catch (e) {
      showToast(`Не удалось скопировать: ${e}`);
    }
  }

  async function copySelectedLogs() {
    const fragment = window.getSelection()?.toString() ?? "";
    if (fragment.trim()) {
      try {
        await navigator.clipboard.writeText(fragment);
        showToast("Выделенный фрагмент скопирован");
      } catch (e) {
        showToast(`Не удалось скопировать: ${e}`);
      }
      return;
    }
    await copyAllLogs();
  }

  function selectAllLogs() {
    const root = logsScrollEl;
    if (!root) return;
    const range = document.createRange();
    range.selectNodeContents(root);
    const sel = window.getSelection();
    if (!sel) return;
    sel.removeAllRanges();
    sel.addRange(range);
    root.focus({ preventScroll: true });
  }

  let contentByFolder: { mods: any[]; resourcepacks: any[]; shaderpacks: any[] } = {
    mods: [],
    resourcepacks: [],
    shaderpacks: [],
  };
  let modSearch = "";
  let modFilter = "all";
  let updates: Record<string, any> = {};
  let isCheckingUpdates = false;
  let showModBrowser = false;
  /** true — клик по моду в списке контента (модалка проекта); false — кнопка «Добавить» (боковая панель) */
  let discoverFromContentRow = false;
  let openModProjectId: string | undefined = undefined;
  let corePackVersionModalOpen = false;

  const defaultInstSettings = {
    override_global: false,
    ram_mb: 4096,
    jvm_args: "",
    use_discrete_gpu: false,
    custom_java_path: "",
  };

  let instSettings = { ...defaultInstSettings };

  let instanceAdvancedCounts: {
    mods: number;
    resourcepacks: number;
    shaderpacks: number;
  } | null = null;
  let javaRuntimesForInstance: { path: string; major: number; label: string }[] = [];
  let javaPickMenuOpen = false;
  let downloadJavaMajorStr = "17";
  let isDownloadingJavaForInstance = false;
  let isBackingUpInstance = false;
  let dndHover = false;
  let dndUnlisten: (() => void) | undefined;
  let maxRam = 8192;
  let globalFilter = "all";
  let instanceSearchQuery = "";
  let experimentalFileBrowser = false;
  let enableAlphaLoaders = false;
  let fileBrowserOpen = false;
  let fileBrowserInitialPath = "";
  let confirmDeleteId: string | null = null;
  let confirmDeleteMod: string | null = null;
  let showExportModal = false;
  let contentTab: "mods" | "resourcepacks" | "shaderpacks" = "mods";
  let contentViewMode: "list" | "grid" = "list";
  /** Понятная подпись для источника мода: modrinth (xxxxxxxx, 8 символов a-z0-9) vs curseforge (число). */
  function detectModSource(project_id: unknown): "modrinth" | "curseforge" | "unknown" {
    const id = String(project_id ?? "").trim();
    if (!id) return "unknown";
    if (/^\d+$/.test(id)) return "curseforge";
    if (/^[a-zA-Z0-9]{8}$/.test(id)) return "modrinth";
    return "unknown";
  }
  function contentTabIcon(id: string) {
    if (id === "mods") return Puzzle;
    if (id === "resourcepacks") return ImageIcon;
    if (id === "shaderpacks") return Sparkles;
    return Puzzle;
  }
  let isLaunching = false;
  let isRepairing = false;
  let coreLoader = "";
  let coreGameVer = "";
  let coreLoaderVer = "";
  let coreVersions: string[] = [];
  let coreLoaderVersions: string[] = [];
  let packSourceInfo: any = null;
  let packVersions: any[] = [];
  let selectedPackVersion: any = null;
  let renameValue = "";
  /** Чтобы $: не затирал поле переименования при каждом тике — синк только при смене сборки */
  let renameDraftForInstanceId: string | null = null;
  let serverLaunchHandled = false;
  let worldLaunchHandled = false;
  let worldsList: string[] = [];
  let recentServers: any[] = [];
  let lastWorldGlobal: { instance_id: string; instance_name: string; world_name: string } | null = null;

  let saveSettingsTimer: ReturnType<typeof setTimeout> | null = null;
  let logUnlisten: (() => void) | undefined;
  /** Защита от множественных listen(log_*) при частом обновлении selectedInstance (гонка с .then). */
  let logListenGen = 0;
  let packLoadSeq = 0;

  $: busyPercent = progress.total > 0 ? Math.round((progress.downloaded / progress.total) * 100) : 0;

  $: safeModsList = (() => {
    const tab = contentTab;
    const arr =
      tab === "mods"
        ? contentByFolder.mods
        : tab === "resourcepacks"
          ? contentByFolder.resourcepacks
          : contentByFolder.shaderpacks;
    return Array.isArray(arr) ? arr : [];
  })();

  $: displayedMods = safeModsList.filter((m) => {
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

  $: contentCounts = {
    mods: (contentByFolder.mods || []).length,
    resourcepacks: (contentByFolder.resourcepacks || []).length,
    shaderpacks: (contentByFolder.shaderpacks || []).length,
  };
  $: enabledCurrentCount = safeModsList.filter((m) => m?.enabled).length;
  $: pendingUpdatesCount = safeModsList.filter(
    (m) => m && updates[m.hash],
  ).length;

  $: displayedInstances = (() => {
    let list =
      globalFilter === "all" ? instances : instances.filter((i) => i.loader === globalFilter);
    const q = instanceSearchQuery.trim().toLowerCase();
    if (q) list = list.filter((i) => (i.name || "").toLowerCase().includes(q));
    return list;
  })();

  $: {
    const id = selectedInstance?.id ?? "";
    if (id !== lastLogInstanceId) {
      lastLogInstanceId = id;
      logs = [];
    }
  }

  /** Список имён jar для DiscoverTab (без TS в разметке) */
  $: discoverInstalledModSlugs = (contentByFolder.mods || []).map((m) =>
    String(m?.clean_name || "").replace(/\.(jar|zip)$/i, ""),
  );

  /** Метаданные установленных модов для карточки в Discover (project_id / версия на диске) */
  $: installedModSummariesForDiscover = (contentByFolder.mods || [])
    .filter((m: any) => m?.project_id)
    .map((m: any) => ({
      project_id: String(m.project_id),
      filename: m.filename,
      clean_name: String(m.clean_name || "").replace(/\.disabled$/i, ""),
      version_id: String(m.version_id || ""),
    }));

  /** project_id всех типов контента — скрыть «Установить» в Discover для уже добавленного */
  $: installedDiscoverContentProjectIds = [
    ...(contentByFolder.mods || []),
    ...(contentByFolder.resourcepacks || []),
    ...(contentByFolder.shaderpacks || []),
  ]
    .map((m: any) => m?.project_id)
    .filter(Boolean)
    .map(String);

  async function loadAllContentForInstance(expectedId: string) {
    try {
      const [mods, rp, sp] = await Promise.all([
        invoke("get_installed_content", {
          instanceId: expectedId,
          folder: "mods",
          includeFileHashes: false,
        }),
        invoke("get_installed_content", {
          instanceId: expectedId,
          folder: "resourcepacks",
          includeFileHashes: false,
        }),
        invoke("get_installed_content", {
          instanceId: expectedId,
          folder: "shaderpacks",
          includeFileHashes: false,
        }),
      ]);
      if (selectedInstance?.id !== expectedId) return;
      contentByFolder = {
        mods: Array.isArray(mods) ? mods : [],
        resourcepacks: Array.isArray(rp) ? rp : [],
        shaderpacks: Array.isArray(sp) ? sp : [],
      };
    } catch {
      if (selectedInstance?.id !== expectedId) return;
      contentByFolder = { mods: [], resourcepacks: [], shaderpacks: [] };
    }
  }

  async function loadContent(folder: "mods" | "resourcepacks" | "shaderpacks") {
    const id = selectedInstance?.id;
    if (!id) return;
    try {
      const m: any = await invoke("get_installed_content", {
        instanceId: id,
        folder,
        includeFileHashes: false,
      });
      if (selectedInstance?.id !== id) return;
      contentByFolder = { ...contentByFolder, [folder]: m || [] };
    } catch {
      if (selectedInstance?.id !== id) return;
      contentByFolder = { ...contentByFolder, [folder]: [] };
    }
  }

  function reloadActiveContentTab() {
    loadContent(contentTab);
  }

  function reloadAllInstanceContent() {
    if (selectedInstance?.id) void loadAllContentForInstance(selectedInstance.id);
  }

  type ContentFolderTab = "mods" | "resourcepacks" | "shaderpacks";
  const contentTabDefs: { id: ContentFolderTab; label: string }[] = [
    { id: "mods", label: "Моды" },
    { id: "resourcepacks", label: "Ресурспаки" },
    { id: "shaderpacks", label: "Шейдеры" },
  ];

  function selectContentFolder(id: ContentFolderTab) {
    contentTab = id;
    loadContent(id);
  }

  async function loadData() {
    let instanceList: any[] | null = null;
    try {
      const insts = await invoke("get_instances");
      instanceList = Array.isArray(insts) ? insts : [];
    } catch (e) {
      console.error(e);
    }
    let acc = activeAccount;
    try {
      const profs: any = await invoke("load_profiles");
      acc =
        profs?.accounts?.find((a: any) => a.id === profs?.active_account_id) || null;
      activeAccount = acc;
    } catch (e) {
      console.error(e);
    }
    if (instanceList === null) {
      return;
    }
    try {
      instances = instanceList;
      invoke("get_system_ram").then((r: any) => (maxRam = r));

      if (initialServerIp && initialInstanceId && !serverLaunchHandled) {
        const inst = instanceList.find((i: any) => i.id === initialInstanceId);
        if (inst) {
          serverLaunchHandled = true;
          selectedInstance = inst;
          onServerLaunchConsumed?.();
          onInstanceOpened?.();
          invoke("update_server_last_played", { ip: initialServerIp, name: "", instanceId: inst.id }).catch(() => {});
          launchInstance(inst, initialServerIp, undefined, acc);
        }
      }
      if (initialWorldName && initialInstanceId && !initialServerIp && !worldLaunchHandled) {
        const inst = instanceList.find((i: any) => i.id === initialInstanceId);
        if (inst) {
          worldLaunchHandled = true;
          selectedInstance = inst;
          onWorldLaunchConsumed?.();
          onInstanceOpened?.();
          launchInstance(inst, undefined, initialWorldName, acc);
        }
      }
      if (
        initialInstanceId &&
        !initialServerIp &&
        !initialWorldName &&
        !serverLaunchHandled &&
        !worldLaunchHandled
      ) {
        const inst = instanceList.find((i: any) => i.id === initialInstanceId);
        if (inst) {
          selectedInstance = inst;
          await tick();
          onInstanceOpened?.();
        }
      }
    } catch (e) {
      console.error(e);
    }
  }

  async function loadPackSourceForSelection(id: string) {
    const seq = ++packLoadSeq;
    try {
      const info: any = await invoke("get_pack_source_info", { instanceId: id });
      if (seq !== packLoadSeq || selectedInstance?.id !== id) return;
      packSourceInfo = info || null;
      if (info?.project_id && (info.source === "curseforge" || info.source === "modrinth")) {
        const vers: any =
          info.source === "curseforge"
            ? await invoke("get_curseforge_versions", { id: info.project_id })
            : await invoke("get_modrinth_versions", { id: info.project_id });
        if (seq !== packLoadSeq || selectedInstance?.id !== id) return;
        packVersions = Array.isArray(vers) ? vers : [];
        selectedPackVersion = info.version_id || null;
      } else {
        packVersions = [];
      }
    } catch {
      if (seq === packLoadSeq && selectedInstance?.id === id) packSourceInfo = null;
    }
  }

  $: if (selectedInstance) {
    instSettings = { ...defaultInstSettings, ...(selectedInstance.settings || {}) };
    packUpdateInfo = null;
    const rid = selectedInstance.id;
    if (renameDraftForInstanceId !== rid) {
      renameDraftForInstanceId = rid;
      renameValue = selectedInstance.name || "";
      coreLoader = "";
      coreGameVer = "";
      coreLoaderVer = "";
      coreVersions = [];
      coreLoaderVersions = [];
    }
    void loadPackSourceForSelection(rid);
  } else {
    renameDraftForInstanceId = null;
  }

  $: if (instanceTab === "options" && settingsSubTab === "core" && selectedInstance) {
    const loader = selectedInstance.loader;
    void invoke("fetch_vanilla_versions")
      .then((manifest: any) => {
        const ids = filterMojangManifestVersions(
          manifest?.versions || [],
          showMcSnapshotVersions,
          showMcAlphaBetaVersions,
        );
        const vers = sortMcVersionsDesc(ids);
        coreVersions = vers.length > 0 ? vers : [selectedInstance.game_version];
      })
      .catch(() => {
        coreVersions = [selectedInstance.game_version];
      });
    if (loader !== "vanilla") {
      void invoke("get_loader_versions", {
        loader,
        includeSnapshots: showMcSnapshotVersions,
        includeAlphaBeta: showMcAlphaBetaVersions,
      }).then((vers: any) => {
        if (vers?.length) coreVersions = sortMcVersionsDesc(vers);
      });
      void invoke("get_specific_loader_versions", {
        loader,
        gameVersion: selectedInstance.game_version,
      }).then((vers: any) => {
        coreLoaderVersions = vers || [];
      });
    }
  }

  $: if (isCreating) {
    void invoke("load_settings").then((s: any) => {
      showMcSnapshotVersions = !!s?.show_mc_snapshot_versions;
      showMcAlphaBetaVersions = !!s?.show_mc_alpha_beta_versions;
    });
    isLoadingVersions = true;
    void invoke("get_loader_versions", {
      loader: newLoader,
      includeSnapshots: showMcSnapshotVersions,
      includeAlphaBeta: showMcAlphaBetaVersions,
    })
      .then((vers: any) => {
        const v = sortMcVersionsDesc(vers || []);
        availableVersions = v;
        if (v.length > 0 && !v.includes(newVersion)) newVersion = v[0];
      })
      .catch(() => {
        availableVersions = ["1.20.1"];
        newVersion = "1.20.1";
      })
      .finally(() => {
        isLoadingVersions = false;
      });
  }

  function getDndContext() {
    return { selectedInstance, instanceTab, contentTab };
  }

  async function refreshInstanceAdvancedPanel() {
    if (!selectedInstance) return;
    try {
      instanceAdvancedCounts = (await invoke("get_instance_content_counts", {
        instanceId: selectedInstance.id,
      })) as typeof instanceAdvancedCounts;
    } catch {
      instanceAdvancedCounts = null;
    }
    try {
      javaRuntimesForInstance = (await invoke("list_detected_java_runtimes")) as typeof javaRuntimesForInstance;
    } catch {
      javaRuntimesForInstance = [];
    }
  }

  async function downloadJavaForInstanceMajor() {
    const maj = parseInt(downloadJavaMajorStr, 10);
    if (!Number.isFinite(maj) || maj < 8) {
      showToast("Укажите мажорную версию Java (8+)");
      return;
    }
    isDownloadingJavaForInstance = true;
    try {
      await invoke("download_java_major", { major: maj });
      showToast(`Java ${maj} загружена`);
      await refreshInstanceAdvancedPanel();
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    } finally {
      isDownloadingJavaForInstance = false;
    }
  }

  async function backupCurrentInstance() {
    if (!selectedInstance) return;
    isBackingUpInstance = true;
    try {
      const p = await invoke("backup_instance_zip", { instanceId: selectedInstance.id });
      showToast(`Резервная копия: ${p}`);
    } catch (e) {
      const msg = String(e);
      if (!msg.includes("Отменено")) showToast(`Ошибка: ${e}`);
    } finally {
      isBackingUpInstance = false;
    }
  }

  $: if (instanceTab === "options" && settingsSubTab === "advanced" && selectedInstance?.id) {
    void refreshInstanceAdvancedPanel();
  }

  $: if (isCreating && newVersion && newLoader !== "vanilla") {
    isLoadingLoaderVersions = true;
    void invoke("get_specific_loader_versions", { loader: newLoader, gameVersion: newVersion })
      .then((vers: any) => {
        availableLoaderVersions = vers || [];
        if (vers && vers.length > 0) newLoaderVersion = vers[0];
        else newLoaderVersion = "";
      })
      .catch(() => {
        availableLoaderVersions = [];
      })
      .finally(() => {
        isLoadingLoaderVersions = false;
      });
  }

  $: if (selectedInstance) {
    const id = selectedInstance.id;
    const gen = ++logListenGen;
    void loadAllContentForInstance(id);
    logUnlisten?.();
    logUnlisten = undefined;
    void listen(`log_${id}`, (e: any) => {
      if (gen !== logListenGen) return;
      if (selectedInstance?.id !== id) return;
      logs = [...logs.slice(-100), e.payload];
      setTimeout(() => logsEndEl?.scrollIntoView({ behavior: "smooth" }), 50);
    }).then((f) => {
      if (gen !== logListenGen) {
        f();
        return;
      }
      logUnlisten = f;
    });
  } else {
    logListenGen++;
    contentByFolder = { mods: [], resourcepacks: [], shaderpacks: [] };
    logUnlisten?.();
    logUnlisten = undefined;
  }

  let downloadProgressUnlisten: (() => void) | undefined;
  let instancesChangedUnlisten: (() => void) | undefined;
  let instancesReloadDebounce: ReturnType<typeof setTimeout> | null = null;

  onMount(() => {
    void invoke("load_settings").then((s: any) => {
      experimentalFileBrowser = !!s?.internal_file_browser;
      enableAlphaLoaders = !!s?.enable_alpha_loaders;
      showMcSnapshotVersions = !!s?.show_mc_snapshot_versions;
      showMcAlphaBetaVersions = !!s?.show_mc_alpha_beta_versions;
    });
    void loadData();
    void syncRunningInstancesFromRust();
    const onVis = () => {
      if (document.visibilityState === "visible") void syncRunningInstancesFromRust();
    };
    document.addEventListener("visibilitychange", onVis);
    runningVisCleanup = () => document.removeEventListener("visibilitychange", onVis);
    void listen<string>("exit_", (e) => {
      const id = e.payload;
      if (id) {
        runningInstances = runningInstances.filter((x) => x !== id);
        showToast("Игра закрыта");
        void (async () => {
          try {
            const insts = ((await invoke("get_instances")) as any[]) || [];
            instances = insts;
            if (selectedInstance) {
              const u = insts.find((i: any) => i.id === selectedInstance.id);
              if (u) selectedInstance = u;
            }
          } catch {
            /* ignore */
          }
        })();
      }
    }).then((f) => {
      exitUnlisten = f;
    });

    void listen<any>("download_progress", (e) => {
      const p = e.payload;
      if (p?.silent) return;
      if (p?.total > 0 && p?.downloaded >= p?.total) {
        if (instancesReloadDebounce) clearTimeout(instancesReloadDebounce);
        instancesReloadDebounce = setTimeout(() => {
          instancesReloadDebounce = null;
          void loadData();
        }, 400);
      }
    }).then((f) => {
      downloadProgressUnlisten = f;
    });

    void listen("instances_changed", () => {
      void loadData();
    }).then((f) => {
      instancesChangedUnlisten = f;
    });

    void getCurrentWebview()
      .onDragDropEvent((e) => {
        const p = e.payload;
        const { selectedInstance: sel, instanceTab: itab, contentTab: ct } = getDndContext();
        if (p.type === "enter" || p.type === "over") {
          dndHover = !!(sel && itab === "content");
        } else if (p.type === "leave") {
          dndHover = false;
        } else if (p.type === "drop") {
          dndHover = false;
          if (!sel || itab !== "content") return;
          const id = sel.id;
          void (async () => {
            let ok = 0;
            for (const path of p.paths) {
              const lower = path.toLowerCase();
              let folder: "mods" | "resourcepacks" | "shaderpacks";
              if (lower.endsWith(".jar") || lower.endsWith(".jar.disabled")) {
                folder = "mods";
              } else if (lower.endsWith(".zip")) {
                folder = ct;
              } else {
                showToast(`Пропуск: не .jar и не .zip — ${path.split(/[/\\]/).pop() || path}`);
                continue;
              }
              try {
                await invoke("install_dropped_content_file", {
                  instanceId: id,
                  sourcePath: path,
                  folder,
                });
                ok++;
              } catch (err) {
                showToast(`Ошибка установки: ${err}`);
              }
            }
            if (ok > 0) {
              showToast(ok === 1 ? "Файл добавлен в сборку" : `Добавлено файлов: ${ok}`);
              reloadActiveContentTab();
            }
          })();
        }
      })
      .then((u) => {
        dndUnlisten = u;
      });
  });

  let exitUnlisten: (() => void) | undefined;
  let runningVisCleanup: (() => void) | undefined;

  onDestroy(() => {
    logUnlisten?.();
    exitUnlisten?.();
    downloadProgressUnlisten?.();
    instancesChangedUnlisten?.();
    dndUnlisten?.();
    runningVisCleanup?.();
    if (instancesReloadDebounce) clearTimeout(instancesReloadDebounce);
    if (saveSettingsTimer) clearTimeout(saveSettingsTimer);
  });

  async function pickNewInstanceIcon() {
    try {
      const p = await invoke("pick_image_file");
      if (p) newIcon = String(p);
    } catch {
      /* ignore */
    }
  }

  async function handleCreate() {
    if (!newName.trim() || !newVersion) return;
    const lv =
      newLoader === "vanilla"
        ? ""
        : newLoader === "modloader" && !String(newLoaderVersion || "").trim()
          ? "manual"
          : newLoaderVersion;
    await invoke("create_instance", {
      name: newName,
      gameVersion: newVersion,
      loader: newLoader,
      loaderVersion: lv,
      icon: newIcon || null,
    });
    isCreating = false;
    newName = "";
    newIcon = "";
    await loadData();
    showToast("Сборка создана!");
  }

  async function saveInstSettings(newSettings: typeof instSettings, skipDebounce = false) {
    if (!selectedInstance) return;
    instSettings = newSettings;
    if (skipDebounce) {
      if (saveSettingsTimer) {
        clearTimeout(saveSettingsTimer);
        saveSettingsTimer = null;
      }
      await invoke("save_instance_settings", { id: selectedInstance.id, settings: newSettings });
      await loadData();
      showToast("Настройки сохранены");
      return;
    }
    if (saveSettingsTimer) clearTimeout(saveSettingsTimer);
    saveSettingsTimer = setTimeout(async () => {
      saveSettingsTimer = null;
      if (!selectedInstance) return;
      await invoke("save_instance_settings", { id: selectedInstance.id, settings: newSettings });
      await loadData();
      showToast("Настройки сохранены");
    }, 600);
  }

  async function handleDelete(id: string) {
    try {
      showToast("Удаление сборки...");
      await invoke("delete_instance", { id });
      confirmDeleteId = null;
      if (selectedInstance?.id === id) selectedInstance = null;
      await loadData();
      showToast("Сборка успешно удалена");
    } catch (e) {
      showToast(`Ошибка удаления: ${e}`);
    }
  }

  async function checkPackUpdate() {
    if (!selectedInstance) return;
    isCheckingPackUpdate = true;
    packUpdateInfo = null;
    try {
      const info: any = await invoke("check_modpack_update", { instanceId: selectedInstance.id });
      packUpdateInfo = info;
      if (info?.has_update) {
        showToast(`Доступна новая версия: ${info.latest_version || ""}`);
      } else if (info?.reason) {
        showToast(info.reason);
      } else {
        showToast("Сборка актуальна");
      }
    } catch (e) {
      showToast(`Ошибка проверки: ${e}`);
      packUpdateInfo = null;
    } finally {
      isCheckingPackUpdate = false;
    }
  }

  async function applyPackUpdate() {
    if (!selectedInstance || !packUpdateInfo?.has_update || !packUpdateInfo?.update_url) return;
    isUpdatingPack = true;
    try {
      await invoke("update_modpack", {
        instanceId: selectedInstance.id,
        updateUrl: packUpdateInfo.update_url,
        newPackVersionRef: packUpdateInfo.latest_version_id || null,
      });
      showToast("Сборка обновлена!");
      packUpdateInfo = null;
      reloadAllInstanceContent();
      await loadData();
    } catch (e) {
      showToast(`Ошибка обновления: ${e}`);
    } finally {
      isUpdatingPack = false;
    }
  }

  async function launchInstance(inst: any, serverIp?: string, worldName?: string, accountOverride?: any) {
    const account = accountOverride || activeAccount;
    if (!account) {
      try {
        const profs: any = await invoke("load_profiles");
        const acc = profs?.accounts?.find((a: any) => a.id === profs.active_account_id) || null;
        if (acc) {
          activeAccount = acc;
          return launchInstance(inst, serverIp, worldName, acc);
        }
      } catch {
        /* ignore */
      }
      showToast("Выберите аккаунт в профиле!");
      return;
    }
    if (runningInstances.includes(inst.id)) {
      await invoke("stop_instance", { instanceId: inst.id });
      runningInstances = runningInstances.filter((x) => x !== inst.id);
      return;
    }

    try {
      isLaunching = true;
      showToast("Подготовка к запуску...");
      instanceTab = "logs";
      logs = [];

      const playName = account.username;

      // Параллелим подготовку файлов и обновление MS-токена: они НЕ зависят друг от друга.
      // - `prepare_launch`: на warm-path ~1-3 мс (sentinel install_state.json + early-exit).
      // - `refresh_account_for_launch`: на warm-path (токен не истёк) ~1 мс, без сети;
      //   на cold-path (истёк) 8-12 с сетью. Раньше это блокировало запуск.
      const isMs = account.acc_type === "microsoft" || account.id?.startsWith("ms-");
      const prepP = invoke("prepare_launch", {
        instanceId: inst.id,
        gameVersion: inst.game_version,
        loader: inst.loader ?? "vanilla",
        loaderVersion: inst.loader_version?.trim() ? inst.loader_version : null,
      }) as Promise<string>;
      const refreshP = isMs
        ? (invoke("refresh_account_for_launch", { accountId: account.id }) as Promise<any>)
        : Promise.resolve(account);

      let launchVersion: string;
      let launchAccount: any;
      try {
        const settled = await Promise.allSettled([prepP, refreshP]);
        if (settled[1].status === "rejected" && isMs) {
          showToast(`Сессия Microsoft устарела: ${settled[1].reason}`);
          runningInstances = runningInstances.filter((id) => id !== inst.id);
          return;
        }
        if (settled[0].status === "rejected") {
          throw settled[0].reason;
        }
        launchVersion = settled[0].value;
        launchAccount = settled[1].status === "fulfilled" ? settled[1].value : account;
        if (isMs) activeAccount = launchAccount;
      } catch (e) {
        showToast(`Ошибка: ${e}`);
        runningInstances = runningInstances.filter((id) => id !== inst.id);
        return;
      }

      runningInstances = [...runningInstances, inst.id];

      if (serverIp)
        invoke("update_server_last_played", { ip: serverIp, name: "", instanceId: inst.id }).catch(() => {});

      try {
        await invoke("fluxcore_launch", {
          instanceId: inst.id,
          versionId: launchVersion,
          username: playName,
          uuid: launchAccount.uuid || "00000000-0000-0000-0000-000000000000",
          token: launchAccount.token || "0",
          accType: launchAccount.acc_type || "offline",
          serverIp: serverIp || null,
          worldName: worldName || null,
        });
      } catch {
        await invoke("launch_game", {
          instanceId: inst.id,
          versionId: launchVersion,
          username: playName,
          uuid: launchAccount.uuid || "00000000-0000-0000-0000-000000000000",
          token: launchAccount.token || "0",
          accType: launchAccount.acc_type || "offline",
          serverIp: serverIp || "",
          worldName: worldName || null,
        });
      }
    } catch (e) {
      showToast(`Ошибка: ${e}`);
      runningInstances = runningInstances.filter((id) => id !== inst.id);
    } finally {
      isLaunching = false;
    }
  }

  async function toggleMod(filename: string, enable: boolean) {
    if (!selectedInstance) return;
    await invoke("toggle_mod", { instanceId: selectedInstance.id, filename, enable, folder: contentTab });
    reloadActiveContentTab();
  }

  async function deleteMod(filename: string) {
    if (!selectedInstance) return;
    try {
      await invoke("delete_mod", { instanceId: selectedInstance.id, filename, folder: contentTab });
      confirmDeleteMod = null;
      reloadActiveContentTab();
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  async function checkForUpdates() {
    if (!selectedInstance) return;
    isCheckingUpdates = true;
    try {
      const id = selectedInstance.id;
      const modsWithHash: any = await invoke("get_installed_content", {
        instanceId: id,
        folder: "mods",
        includeFileHashes: true,
      });
      const list = Array.isArray(modsWithHash) ? modsWithHash : [];
      if (selectedInstance?.id !== id) return;
      contentByFolder = { ...contentByFolder, mods: list };

      const hashes = list.map((m: any) => m.hash).filter((h: string) => h.length > 0);
      if (hashes.length === 0) {
        showToast("Нет jar для проверки (или мода только что добавили — обновите список)");
        return;
      }
      const res: any = await invoke("check_mod_updates", {
        hashes,
        loader: selectedInstance.loader,
        gameVersion: selectedInstance.game_version,
      });

      const actualUpdates: Record<string, any> = {};
      let count = 0;
      for (const hash in res) {
        const mod = list.find((m: any) => m.hash === hash);
        if (mod && res[hash].id !== mod.version_id) {
          actualUpdates[hash] = res[hash];
          count++;
        }
      }

      updates = actualUpdates;
      if (count > 0) showToast(`Найдено обновлений: ${count}`);
      else showToast("Все моды обновлены!");
    } catch {
      showToast("Ошибка проверки");
    } finally {
      isCheckingUpdates = false;
    }
  }

  async function updateMod(oldFilename: string, oldHash: string) {
    if (!selectedInstance) return;
    const updateInfo = updates[oldHash];
    if (!updateInfo) return;
    showToast("Обновление мода...");
    try {
      await invoke("delete_mod", { instanceId: selectedInstance.id, filename: oldFilename, folder: contentTab });
      await invoke("install_mod_with_dependencies", {
        instanceId: selectedInstance.id,
        versionId: updateInfo.id,
        gameVersion: selectedInstance.game_version,
        loader: selectedInstance.loader,
      });
      showToast("Мод обновлен!");
      reloadActiveContentTab();
      const nu = { ...updates };
      delete nu[oldHash];
      updates = nu;
    } catch {
      showToast("Ошибка обновления");
    }
  }

  function onInstanceIconError(e: Event) {
    const el = e.currentTarget;
    if (el instanceof HTMLImageElement) {
      el.style.display = "none";
      el.nextElementSibling?.classList.remove("hidden");
    }
  }

  function onListCardIconError(e: Event) {
    const el = e.currentTarget;
    if (el instanceof HTMLImageElement) {
      el.style.display = "none";
      el.nextElementSibling?.classList.remove("hidden");
    }
  }

  async function importInstance() {
    try {
      const res = (await invoke("import_instance")) as { message: string; instance_id: string };
      showToast(res.message);
      await loadData();
    } catch (e) {
      showToast(`Ошибка импорта: ${e}`);
    }
  }

  function onWorldsTabOpen() {
    if (!selectedInstance) return;
    void invoke("list_worlds", { instanceId: selectedInstance.id }).then((w: any) => (worldsList = w || []));
    void invoke("load_servers", { instanceId: selectedInstance.id }).then((s: any) =>
      (recentServers = Array.isArray(s) ? s : []),
    );
    void invoke("get_last_world", { instanceId: selectedInstance.id })
      .then((lw: any) => (lastWorldGlobal = lw && lw.instance_id ? lw : null))
      .catch(() => (lastWorldGlobal = null));
  }

  function onCoreSettingsTabClick() {
    settingsSubTab = "core";
    void invoke("load_settings").then((s: any) => {
      showMcSnapshotVersions = !!s?.show_mc_snapshot_versions;
      showMcAlphaBetaVersions = !!s?.show_mc_alpha_beta_versions;
    });
    if (selectedInstance) {
      if (!coreLoader) coreLoader = selectedInstance.loader;
      if (!coreGameVer) coreGameVer = selectedInstance.game_version;
      if (!coreLoaderVer) coreLoaderVer = selectedInstance.loader_version || "";

      const l = coreLoader;
      const gv = coreGameVer;
      if (l === "vanilla") {
        void invoke("fetch_vanilla_versions").then((m: any) => {
          const ids = filterMojangManifestVersions(
            m?.versions || [],
            showMcSnapshotVersions,
            showMcAlphaBetaVersions,
          );
          coreVersions = sortMcVersionsDesc(ids);
        });
      } else {
        void invoke("get_loader_versions", {
          loader: l,
          includeSnapshots: showMcSnapshotVersions,
          includeAlphaBeta: showMcAlphaBetaVersions,
        }).then((vers: any) => (coreVersions = vers || []));
        void invoke("get_specific_loader_versions", {
          loader: l,
          gameVersion: gv,
        }).then((vers: any) => (coreLoaderVersions = vers || []));
      }
    }
  }

  async function applyRename() {
    if (!selectedInstance) return;
    const next = renameValue.trim();
    if (!next) {
      showToast("Введите название сборки");
      return;
    }
    try {
      await invoke("rename_instance", { id: selectedInstance.id, newName: next });
      showToast("Название изменено");
      const insts: any = await invoke("get_instances");
      const u = (insts || []).find((i: any) => i.id === selectedInstance.id);
      if (u) selectedInstance = u;
      await loadData();
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  async function applyPackVersionUpdate(): Promise<boolean> {
    if (!selectedInstance || !packSourceInfo) return false;
    const ver = selectedPackVersion || packSourceInfo.version_id;
    const vObj = packVersions.find((v: any) => v.id === ver);
    const url = vObj?.files?.find((f: any) => f.filename?.endsWith?.(".mrpack"))?.url;
    if (!url) {
      showToast("Нет .mrpack для этой версии");
      return false;
    }
    isUpdatingPack = true;
    try {
      const res = await invoke("update_modpack", {
        instanceId: selectedInstance.id,
        updateUrl: url,
        newPackVersionRef: ver,
      });
      showToast(String(res));
      await loadData();
      const insts: any = await invoke("get_instances");
      const u = (insts || []).find((i: any) => i.id === selectedInstance.id);
      if (u) selectedInstance = u;
      packSourceInfo = null;
      const info: any = await invoke("get_pack_source_info", { instanceId: selectedInstance.id });
      packSourceInfo = info;
      return true;
    } catch (e) {
      showToast(`Ошибка: ${e}`);
      return false;
    } finally {
      isUpdatingPack = false;
    }
  }

  async function applyPackVersionFromCoreModal() {
    const ok = await applyPackVersionUpdate();
    if (ok) corePackVersionModalOpen = false;
  }

  async function unlinkPack() {
    if (!selectedInstance) return;
    try {
      await invoke("unlink_modpack", { id: selectedInstance.id });
      showToast("Сборка отвязана");
      packSourceInfo = null;
      if (settingsSubTab === "pack") settingsSubTab = "general";
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  async function applyCoreUpdate() {
    if (!selectedInstance) return;
    const l = coreLoader || selectedInstance.loader;
    const gv = coreGameVer || selectedInstance.game_version;
    const lv = coreLoaderVer || selectedInstance.loader_version || "";
    try {
      await invoke("update_instance_core", {
        id: selectedInstance.id,
        gameVersion: gv,
        loader: l,
        loaderVersion: lv,
      });
      showToast("Ядро обновлено! Перезапустите игру.");
      const insts: any = await invoke("get_instances");
      const updated = (insts || []).find((i: any) => i.id === selectedInstance.id);
      if (updated) selectedInstance = updated;
      await loadData();
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  async function repairCore() {
    if (!selectedInstance) return;
    try {
      isRepairing = true;
      showToast("Очистка ядра...");
      const res = await invoke("repair_core", { id: selectedInstance.id });
      showToast(String(res));
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    } finally {
      isRepairing = false;
    }
  }

  async function refreshModMetadata() {
    if (!selectedInstance) return;
    showToast("Загрузка метаданных...");
    try {
      await invoke("refresh_mod_metadata", { instanceId: selectedInstance.id });
      reloadAllInstanceContent();
      showToast("Метаданные обновлены!");
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    }
  }

  function closeModBrowser() {
    showModBrowser = false;
    discoverFromContentRow = false;
    openModProjectId = undefined;
    reloadAllInstanceContent();
  }

  function setCoreLoaderChoice(v: string) {
    coreLoader = v;
    coreLoaderVer = "";
    coreGameVer = "";
    coreLoaderVersions = [];
    if (v === "vanilla") {
      void invoke("fetch_vanilla_versions").then((m: any) => {
        const ids = filterMojangManifestVersions(
          m?.versions || [],
          showMcSnapshotVersions,
          showMcAlphaBetaVersions,
        );
        coreVersions = sortMcVersionsDesc(ids);
      });
    } else {
      void invoke("get_loader_versions", {
        loader: v,
        includeSnapshots: showMcSnapshotVersions,
        includeAlphaBeta: showMcAlphaBetaVersions,
      }).then((vers: any) => (coreVersions = vers || []));
    }
  }

  function setCoreGameVerChoice(v: string) {
    coreGameVer = v;
    coreLoaderVer = "";
    coreLoaderVersions = [];
    const l = coreLoader || selectedInstance?.loader;
    if (l && l !== "vanilla")
      void invoke("get_specific_loader_versions", { loader: l, gameVersion: v }).then(
        (vers: any) => (coreLoaderVersions = vers || []),
      );
  }

  const baseLoaderOptions = [
    { value: "vanilla", label: "Vanilla" },
    { value: "fabric", label: "Fabric" },
    { value: "quilt", label: "Quilt" },
    { value: "forge", label: "Forge" },
    { value: "neoforge", label: "NeoForge" },
  ];
  $: alphaLoaderOptions = enableAlphaLoaders
    ? [
        { value: "liteloader", label: "LiteLoader (α)" },
        { value: "modloader", label: "ModLoader / Risugami (α)" },
      ]
    : [];
  $: loaderOptions = [...baseLoaderOptions, ...alphaLoaderOptions];
  $: newLoaderOptions = loaderOptions;

  $: coreGameOptions =
    coreVersions.length > 0 && selectedInstance
      ? coreVersions.map((v) => ({ value: v, label: v }))
      : selectedInstance
        ? [{ value: selectedInstance.game_version, label: selectedInstance.game_version }]
        : [];

  $: coreLoaderVerOptions =
    selectedInstance && coreLoaderVersions.length > 0
      ? [{ value: "", label: "Выберите версию загрузчика" }, ...coreLoaderVersions.map((v) => ({ value: v, label: v }))]
      : selectedInstance?.loader_version && !coreLoader
        ? [{ value: selectedInstance.loader_version, label: selectedInstance.loader_version }]
        : [{ value: "", label: "Выберите версию загрузчика" }];
</script>

{#if selectedInstance}
  {@const si = selectedInstance}
  {@const isRunning = runningInstances.includes(si.id)}
  {@const isBusy = busyInstanceId === si.id}
  {#key si.id}
    <div
      class="jm-container flex flex-col h-full"
      in:fly={{ x: 36, duration: 380, opacity: 0.9, easing: quintOut }}
    >
    <div
      class="flex flex-col md:flex-row md:items-center md:justify-between gap-3 mb-4 bg-jm-card p-3 md:p-4 rounded-2xl border border-white/10 shadow-xl shrink-0"
    >
      <div class="flex items-center gap-3">
        <button
          type="button"
          on:click={() => (selectedInstance = null)}
          class="p-2 bg-black/50 hover:bg-jm-accent hover:text-black text-white rounded-lg transition-colors shrink-0"
        >
          <ArrowLeft size={18} />
        </button>
        <div
          class="w-12 h-12 rounded-xl overflow-hidden shrink-0 bg-black/50 flex items-center justify-center border border-white/20"
        >
          {#if instanceIconSrc(si.icon || packSourceInfo?.icon_url)}
            <img
              src={instanceIconSrc(si.icon || packSourceInfo?.icon_url) || ""}
              alt=""
              class="w-full h-full object-cover"
              on:error={onInstanceIconError}
            />
            <span
              class="hidden w-full h-full flex items-center justify-center text-sm font-semibold text-white/70"
              >{si.name?.charAt(0)?.toUpperCase() || "?"}</span
            >
          {:else}
            <span class="w-full h-full flex items-center justify-center text-sm font-semibold text-white/70"
              >{si.name?.charAt(0)?.toUpperCase() || "?"}</span
            >
          {/if}
        </div>
        <div class="min-w-0 flex items-start gap-2">
          <div class="min-w-0 flex-1">
            <h2 class="text-lg md:text-xl font-bold text-white truncate">{si.name}</h2>
            <div class="flex gap-1.5 text-xs mt-1">
            <span class="bg-white/10 px-2 py-0.5 rounded-md text-[var(--text-secondary)] capitalize"
              >{si.loader}</span
            >
            <span class="bg-white/10 px-2 py-0.5 rounded-md text-[var(--text-secondary)]"
              >{si.game_version}</span
            >
            {#if si.loader_version}
              <span
                class="bg-jm-accent/20 text-jm-accent-light px-2 py-0.5 rounded-md border border-jm-accent/30"
                >{si.loader_version}</span
              >
            {/if}
            </div>
          </div>
          <button
            type="button"
            on:click={() => {
              instanceTab = "options";
              settingsSubTab = "general";
            }}
            class="shrink-0 p-2 rounded-lg bg-white/10 hover:bg-white/20 text-white transition-colors self-start"
            title="Переименовать сборку"
          >
            <Pencil size={16} />
          </button>
        </div>
      </div>

      <div class="flex gap-2 flex-wrap">
        <button
          type="button"
          on:click={checkPackUpdate}
          disabled={isCheckingPackUpdate}
          class="bg-white/10 hover:bg-white/20 text-white px-3 py-2 rounded-lg font-bold text-xs transition-colors flex items-center gap-1.5 disabled:opacity-50"
          title="Проверить обновления сборки"
        >
          <RefreshCw size={14} class={isCheckingPackUpdate ? "animate-spin" : ""} />
          <span class="hidden sm:inline">Обновления</span>
        </button>
        {#if packUpdateInfo?.has_update && packUpdateInfo?.update_url}
          <button
            type="button"
            on:click={applyPackUpdate}
            disabled={isUpdatingPack}
            class="bg-blue-500 hover:bg-blue-400 text-white px-3 py-2 rounded-lg font-bold text-xs transition-colors flex items-center gap-1.5 disabled:opacity-50"
          >
            {#if isUpdatingPack}
              <Loader2 size={14} class="animate-spin" />
            {:else}
              <Download size={14} />
            {/if}
            Обновить
          </button>
        {/if}
        <button
          type="button"
          on:click={() => (showExportModal = true)}
          class="bg-white/10 hover:bg-white/20 text-white px-3 py-2 rounded-lg font-bold text-xs transition-colors flex items-center gap-1.5"
          ><Archive size={14} />
          <span class="hidden sm:inline">Экспорт</span></button
        >
        <button
          type="button"
          on:click={() => launchInstance(si)}
          disabled={isLaunching || isRepairing || isBusy}
          class="{isRunning
            ? 'bg-red-500 hover:bg-red-600 text-white'
            : 'bg-jm-accent hover:bg-jm-accent-light text-black'} px-5 py-2 rounded-lg font-bold text-xs transition-transform hover:scale-105 shadow-lg flex items-center gap-1.5 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100"
        >
          {#if isBusy}
            <Loader2 size={14} class="animate-spin" />
            {busyPercent}%
          {:else if isLaunching || isRepairing}
            <Loader2 size={14} class="animate-spin" />
            ...
          {:else if isRunning}
            <Square size={14} fill="currentColor" />
            СТОП
          {:else}
            <Play size={14} fill="currentColor" />
            ИГРАТЬ
          {/if}
        </button>
        <button
          type="button"
          on:click={() => invoke("open_folder", { id: si.id })}
          class="w-9 h-9 bg-white/10 hover:bg-white/20 rounded-lg flex items-center justify-center transition-colors shrink-0"
          title="Открыть папку сборки"
        >
          <FolderOpen size={14} class="text-white" />
        </button>
        {#if experimentalFileBrowser}
          <button
            type="button"
            on:click={() => {
              fileBrowserInitialPath = `instances/${si.id}`;
              fileBrowserOpen = true;
            }}
            class="w-9 h-9 bg-white/10 hover:bg-jm-accent/25 rounded-lg flex items-center justify-center transition-colors shrink-0 border border-white/10"
            title="Обзор файлов (внутри лаунчера)"
          >
            <ListTree size={14} class="text-jm-accent-light" />
          </button>
        {/if}
      </div>
    </div>

    <div class="flex flex-col lg:flex-row gap-3 h-full min-h-0">
      <div
        class="lg:w-48 shrink-0 flex lg:flex-col gap-1.5 overflow-x-auto lg:overflow-x-visible [&::-webkit-scrollbar]:hidden"
      >
        {#each [{ id: "content", label: "Контент", Icon: Puzzle }, { id: "worlds", label: "Миры", Icon: Globe }, { id: "logs", label: "Логи", Icon: Terminal }, { id: "options", label: "Настройки", Icon: Settings }] as tab (tab.id)}
          <button
            type="button"
            on:click={() => {
              instanceTab = tab.id;
              if (tab.id === "worlds") onWorldsTabOpen();
            }}
            class="flex items-center gap-2 px-3 py-2.5 rounded-xl text-sm font-bold transition-all whitespace-nowrap shrink-0 {instanceTab ===
            tab.id
              ? 'bg-jm-accent text-black shadow-md'
              : 'bg-jm-card text-[var(--text-secondary)] hover:text-white border border-white/5 hover:border-white/20'}"
          >
            <svelte:component this={tab.Icon} size={16} />
            {tab.label}
          </button>
        {/each}
        {#if confirmDeleteId === si.id}
          <div class="lg:mt-auto flex flex-col gap-1.5 p-3 rounded-xl bg-red-500/10 border border-red-500/30">
            <p class="text-xs text-red-400 font-bold text-center">Удалить?</p>
            <div class="flex gap-1.5">
              <button
                type="button"
                on:click={() => handleDelete(si.id)}
                class="flex-1 py-1.5 rounded-lg font-bold text-white bg-red-500 hover:bg-red-600 transition-colors text-xs"
                >Да</button
              >
              <button
                type="button"
                on:click={() => (confirmDeleteId = null)}
                class="flex-1 py-1.5 rounded-lg font-bold text-[var(--text-secondary)] bg-white/10 hover:bg-white/20 transition-colors text-xs"
                >Нет</button
              >
            </div>
          </div>
        {:else}
          <button
            type="button"
            on:click={() => (confirmDeleteId = si.id)}
            class="lg:mt-auto flex items-center gap-2 px-3 py-2.5 rounded-xl text-sm font-bold text-red-500 bg-red-500/10 hover:bg-red-500 hover:text-white transition-colors border border-red-500/20 whitespace-nowrap shrink-0"
          >
            <Trash2 size={16} />
            Удалить
          </button>
        {/if}
      </div>

      <div
        class="flex-grow ui-card p-3 md:p-5 overflow-hidden flex flex-col relative min-h-0"
      >
        {#if instanceTab === "content"}
          <div class="relative flex flex-col h-full min-h-0">
            {#if dndHover}
              <div
                class="absolute inset-0 z-20 rounded-[var(--radius-lg)] border-2 border-dashed border-[var(--accent)] bg-[var(--accent-soft)] backdrop-blur-[2px] flex flex-col items-center justify-center gap-2 pointer-events-none"
                aria-hidden="true"
              >
                <PackageOpen size={40} class="text-[var(--accent)]" />
                <p class="text-sm font-semibold text-[var(--text)] text-center px-4">
                  Отпустите файлы — .jar в «Моды», .zip в текущую вкладку ({contentTabDefs.find((t) => t.id === contentTab)
                    ?.label || contentTab})
                </p>
              </div>
            {/if}

            <!-- Большие вкладки-счётчики: сразу видно, сколько всего контента разного типа. -->
            <div class="grid grid-cols-3 gap-2 mb-4">
              {#each contentTabDefs as t (t.id)}
                {@const Icn = contentTabIcon(t.id)}
                {@const isActive = contentTab === t.id}
                <button
                  type="button"
                  on:click={() => selectContentFolder(t.id)}
                  class="group flex items-center gap-3 px-3 py-3 rounded-[var(--radius-lg)] border text-left transition-all duration-200
                    {isActive
                      ? 'bg-[var(--accent-softer)] border-[var(--accent)] shadow-[0_4px_16px_var(--accent-soft)]'
                      : 'bg-[var(--surface-1)] border-[var(--border)] hover:border-[var(--border-strong)] hover:bg-[var(--surface-hover)]'}"
                >
                  <div class="w-10 h-10 rounded-lg flex items-center justify-center shrink-0 {isActive ? 'bg-[var(--accent)] text-[var(--accent-contrast,#000)]' : 'bg-[var(--surface-2)] text-[var(--text-secondary)]'}">
                    <svelte:component this={Icn} size={20} strokeWidth={2.2} />
                  </div>
                  <div class="min-w-0">
                    <div class="text-sm font-semibold text-[var(--text)] truncate">{t.label}</div>
                    <div class="text-[11px] text-[var(--text-secondary)] flex items-center gap-1.5">
                      <span class="font-bold text-[var(--text)]">{contentCounts[t.id] || 0}</span>
                      <span>шт.</span>
                      {#if t.id === "mods" && pendingUpdatesCount > 0 && isActive}
                        <span class="px-1.5 py-0.5 rounded-full text-[10px] font-semibold bg-[var(--accent)] text-[var(--accent-contrast,#000)]">
                          +{pendingUpdatesCount} обн.
                        </span>
                      {/if}
                    </div>
                  </div>
                </button>
              {/each}
            </div>

            <!-- Панель действий: поиск + фильтр + переключатель вида + «Добавить». -->
            <div class="flex flex-wrap items-center gap-2 mb-3">
              <div class="relative flex-1 min-w-[12rem] max-w-md">
                <Search class="absolute left-3 top-1/2 -translate-y-1/2 text-[var(--text-secondary)] pointer-events-none" size={16} />
                <input
                  type="text"
                  placeholder="Поиск по названию…"
                  bind:value={modSearch}
                  class="ui-input pl-9 w-full"
                />
              </div>
              <div class="w-40 shrink-0">
                <LibrarySelect
                  label=""
                  value={modFilter}
                  onChange={(v) => (modFilter = v)}
                  options={[
                    { value: "all", label: `Все · ${safeModsList.length}` },
                    { value: "enabled", label: `Включ. · ${enabledCurrentCount}` },
                    { value: "disabled", label: `Откл. · ${safeModsList.length - enabledCurrentCount}` },
                  ]}
                />
              </div>
              <div class="ui-seg shrink-0" role="group" aria-label="Вид отображения">
                <button
                  type="button"
                  class="ui-seg-item"
                  class:is-active={contentViewMode === "list"}
                  on:click={() => (contentViewMode = "list")}
                  title="Список"
                  aria-label="Список"
                ><ListIcon size={14} strokeWidth={2.2} /></button>
                <button
                  type="button"
                  class="ui-seg-item"
                  class:is-active={contentViewMode === "grid"}
                  on:click={() => (contentViewMode = "grid")}
                  title="Плитка"
                  aria-label="Плитка"
                ><LayoutGrid size={14} strokeWidth={2.2} /></button>
              </div>
              <div class="ml-auto flex items-center gap-2">
                {#if contentTab === "mods"}
                  <button
                    type="button"
                    on:click={checkForUpdates}
                    disabled={isCheckingUpdates}
                    class="ui-btn ui-btn-subtle ui-btn-sm"
                    title="Проверить обновления"
                  >
                    <RefreshCw size={14} class={isCheckingUpdates ? "animate-spin" : ""} />
                    {pendingUpdatesCount > 0 ? `Обновить (${pendingUpdatesCount})` : "Проверить"}
                  </button>
                {/if}
                <button
                  type="button"
                  on:click={refreshModMetadata}
                  class="ui-btn ui-btn-ghost ui-btn-icon"
                  title="Обновить иконки и названия"
                  aria-label="Обновить метаданные"
                >
                  <RefreshCw size={14} />
                </button>
                <button
                  type="button"
                  on:click={() => {
                    discoverFromContentRow = false;
                    showModBrowser = true;
                  }}
                  class="ui-btn ui-btn-primary ui-btn-sm shadow-[0_6px_18px_var(--accent-soft)]"
                  title="Открыть каталог модов, ресурспаков и шейдеров"
                >
                  <Plus size={16} />
                  Добавить
                </button>
              </div>
            </div>

            {#if pendingUpdatesCount > 0 && contentTab === "mods"}
              <div
                class="mb-3 flex items-center gap-3 px-4 py-2.5 rounded-[var(--radius)] border border-[var(--accent)]/40 bg-[var(--accent-softer)]"
              >
                <div class="w-8 h-8 rounded-full bg-[var(--accent)] text-[var(--accent-contrast,#000)] flex items-center justify-center shrink-0">
                  <AlertCircle size={16} strokeWidth={2.4} />
                </div>
                <div class="min-w-0 flex-1">
                  <p class="text-sm font-semibold text-[var(--text)]">
                    Доступно обновлений: {pendingUpdatesCount}
                  </p>
                  <p class="text-[11px] text-[var(--text-secondary)]">Нажмите «Обновить» рядом с модом, чтобы применить.</p>
                </div>
              </div>
            {/if}

            <div class="flex-grow overflow-y-auto custom-scrollbar pr-1 min-h-0">
              {#if displayedMods.length === 0}
                <div class="flex flex-col items-center justify-center h-full gap-4 px-4 py-6 text-center">
                  <div
                    class="w-24 h-24 rounded-full bg-[var(--accent-softer)] border border-[var(--accent)]/30 flex items-center justify-center text-[var(--accent)]"
                  >
                    <svelte:component this={contentTabIcon(contentTab)} size={44} strokeWidth={1.8} />
                  </div>
                  <div class="space-y-1">
                    <p class="text-xl font-bold text-[var(--text)]">
                      {modSearch ? "Ничего не найдено" : "Пока пусто"}
                    </p>
                    <p class="text-sm text-[var(--text-secondary)] max-w-md">
                      {#if modSearch}
                        По запросу «{modSearch}» в текущей вкладке ничего нет. Попробуйте изменить запрос или очистить фильтры.
                      {:else}
                        В этой сборке ещё нет {contentTabDefs.find((t) => t.id === contentTab)?.label?.toLowerCase() || "контента"}.
                        Загрузите файл или найдите нужное в каталоге.
                      {/if}
                    </p>
                  </div>
                  {#if !modSearch}
                    <button
                      type="button"
                      on:click={() => {
                        discoverFromContentRow = false;
                        showModBrowser = true;
                      }}
                      class="ui-btn ui-btn-primary px-5 py-2.5 shadow-[0_6px_18px_var(--accent-soft)]"
                    >
                      <Plus size={16} />
                      Открыть каталог
                    </button>
                    <p class="text-[11px] text-[var(--text-secondary)] opacity-80">
                      или перетащите .jar / .zip сюда
                    </p>
                  {/if}
                </div>
              {:else if contentViewMode === "grid"}
                <!-- Плитка: 2-4 колонки, большие карточки с иконкой. -->
                <div class="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-3">
                  {#each displayedMods as m (m.filename)}
                    {@const source = detectModSource(m.project_id)}
                    {@const hasUpdate = !!updates[m.hash]}
                    <div
                      class="relative group rounded-[var(--radius-lg)] border p-4 flex flex-col gap-3 transition-all duration-200 {m.enabled
                        ? 'bg-[var(--surface-1)] border-[var(--border)] hover:border-[var(--accent)] hover:bg-[var(--surface-hover)] hover:shadow-[0_8px_24px_rgba(0,0,0,0.25)]'
                        : 'bg-[var(--surface-1)] border-red-500/25 opacity-75'}"
                    >
                      {#if hasUpdate}
                        <div
                          class="absolute -top-1.5 -right-1.5 px-2 py-0.5 rounded-full text-[10px] font-bold bg-[var(--accent)] text-[var(--accent-contrast,#000)] shadow-md"
                          title="Доступно обновление"
                        >UPDATE</div>
                      {/if}
                      <div class="flex items-start gap-3">
                        <div class="relative shrink-0">
                          {#if m.icon_url}
                            <img
                              src={m.icon_url}
                              class="w-14 h-14 rounded-xl object-cover bg-[var(--surface-2)] ring-1 ring-[var(--border)]"
                              alt=""
                            />
                          {:else}
                            <div
                              class="w-14 h-14 rounded-xl bg-[var(--surface-2)] ring-1 ring-[var(--border)] flex items-center justify-center"
                            >
                              <svelte:component this={contentTabIcon(contentTab)} size={26} class="text-[var(--text-secondary)]" />
                            </div>
                          {/if}
                        </div>
                        <div class="min-w-0 flex-1">
                          <h4 class="font-semibold text-[var(--text)] truncate">
                            {m.title || m.clean_name}
                          </h4>
                          <p class="text-xs text-[var(--text-secondary)] truncate mt-0.5">
                            {m.version_name || m.clean_name}
                          </p>
                          <div class="flex flex-wrap items-center gap-1 mt-1.5">
                            {#if source === "modrinth"}
                              <span class="ui-chip ui-chip-accent text-[10px] py-0 px-1.5">Modrinth</span>
                            {:else if source === "curseforge"}
                              <span
                                class="inline-flex items-center px-1.5 py-0.5 rounded-full text-[10px] font-semibold"
                                style="background: color-mix(in srgb, #f59e0b 18%, transparent); color: #fbbf24"
                              >CurseForge</span>
                            {:else}
                              <span class="ui-chip text-[10px] py-0 px-1.5">Локальный</span>
                            {/if}
                            {#if !m.enabled}
                              <span class="ui-chip ui-chip-danger text-[10px] py-0 px-1.5">Отключён</span>
                            {/if}
                          </div>
                        </div>
                      </div>
                      <div class="flex items-center justify-between gap-2 mt-auto pt-2 border-t border-[var(--border)]">
                        <!-- svelte-ignore a11y_click_events_have_key_events -->
                        <!-- svelte-ignore a11y_no_static_element_interactions -->
                        <div
                          on:click|stopPropagation={() => toggleMod(m.filename, !m.enabled)}
                          class="w-11 h-6 flex items-center rounded-full p-0.5 cursor-pointer transition-colors duration-200 {m.enabled
                            ? 'bg-[var(--accent)]'
                            : 'bg-[var(--surface-hover)] border border-[var(--border)]'}"
                          title={m.enabled ? "Выключить" : "Включить"}
                        >
                          <div
                            class="bg-white w-5 h-5 rounded-full shadow-md transform transition-transform duration-200 {m.enabled
                              ? 'translate-x-5'
                              : ''}"
                          />
                        </div>
                        <div class="flex items-center gap-1.5">
                          {#if hasUpdate}
                            <button
                              type="button"
                              on:click|stopPropagation={() => updateMod(m.filename, m.hash)}
                              class="ui-btn ui-btn-primary ui-btn-sm"
                            >Обновить</button>
                          {/if}
                          {#if String(m.project_id || "").trim()}
                            <button
                              type="button"
                              on:click|stopPropagation={() => {
                                discoverFromContentRow = true;
                                openModProjectId = String(m.project_id);
                                showModBrowser = true;
                              }}
                              class="ui-btn ui-btn-ghost ui-btn-icon"
                              aria-label="Открыть в каталоге"
                              title="Открыть в каталоге"
                            ><Search size={14} /></button>
                          {/if}
                          {#if confirmDeleteMod === m.filename}
                            <button
                              type="button"
                              on:click|stopPropagation={() => deleteMod(m.filename)}
                              class="ui-btn ui-btn-danger ui-btn-sm"
                            >Удалить?</button>
                            <button
                              type="button"
                              on:click|stopPropagation={() => (confirmDeleteMod = null)}
                              class="ui-btn ui-btn-ghost ui-btn-icon"
                              aria-label="Отмена"
                            ><X size={14} /></button>
                          {:else}
                            <button
                              type="button"
                              on:click|stopPropagation={() => (confirmDeleteMod = m.filename)}
                              class="ui-btn ui-btn-ghost ui-btn-icon hover:text-red-400"
                              aria-label="Удалить"
                              title="Удалить"
                            ><Trash2 size={16} /></button>
                          {/if}
                        </div>
                      </div>
                    </div>
                  {/each}
                </div>
              {:else}
                <!-- Список: крупные строки с источником, версией и быстрыми действиями. -->
                <div class="flex flex-col gap-2">
                {#each displayedMods as m (m.filename)}
                  {@const source = detectModSource(m.project_id)}
                  {@const hasUpdate = !!updates[m.hash]}
                  <div
                    class="flex items-center gap-3 p-3 rounded-[var(--radius)] border transition-all duration-150 {m.enabled
                      ? 'bg-[var(--surface-1)] border-[var(--border)] hover:border-[var(--accent)]/60 hover:bg-[var(--surface-hover)]'
                      : 'bg-[var(--surface-1)] border-red-500/25 opacity-75'}"
                  >
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <div
                      on:click|stopPropagation={() => toggleMod(m.filename, !m.enabled)}
                      class="w-11 h-6 flex items-center rounded-full p-0.5 cursor-pointer transition-colors duration-200 shrink-0 {m.enabled
                        ? 'bg-[var(--accent)]'
                        : 'bg-[var(--surface-hover)] border border-[var(--border)]'}"
                      title={m.enabled ? "Выключить" : "Включить"}
                    >
                      <div
                        class="bg-white w-5 h-5 rounded-full shadow-md transform transition-transform duration-200 {m.enabled
                          ? 'translate-x-5'
                          : ''}"
                      />
                    </div>

                    <div class="relative shrink-0">
                      {#if m.icon_url}
                        <img
                          src={m.icon_url}
                          class="w-12 h-12 rounded-lg object-cover bg-[var(--surface-2)] ring-1 ring-[var(--border)]"
                          alt=""
                        />
                      {:else}
                        <div
                          class="w-12 h-12 rounded-lg bg-[var(--surface-2)] ring-1 ring-[var(--border)] flex items-center justify-center"
                        >
                          <svelte:component this={contentTabIcon(contentTab)} size={22} class="text-[var(--text-secondary)]" />
                        </div>
                      {/if}
                      {#if hasUpdate}
                        <div
                          class="absolute -top-1 -right-1 w-3.5 h-3.5 rounded-full bg-[var(--accent)] ring-2 ring-[var(--surface-1)]"
                          title="Доступно обновление"
                        />
                      {/if}
                    </div>

                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <div
                      class="flex-1 min-w-0"
                      on:click={() => {
                        const pid = String(m.project_id || "").trim();
                        if (pid) {
                          discoverFromContentRow = true;
                          openModProjectId = pid;
                          showModBrowser = true;
                        }
                      }}
                    >
                      <div class="flex items-center gap-2 min-w-0">
                        <h4
                          class="font-semibold text-[var(--text)] truncate transition-colors {String(
                            m.project_id || '',
                          ).trim()
                            ? 'cursor-pointer group-hover:text-[var(--accent)] hover:text-[var(--accent)]'
                            : ''}"
                        >
                          {m.title || m.clean_name}
                        </h4>
                        {#if source === "modrinth"}
                          <span class="ui-chip ui-chip-accent text-[10px] py-0 px-1.5 shrink-0">Modrinth</span>
                        {:else if source === "curseforge"}
                          <span
                            class="inline-flex items-center px-1.5 py-0.5 rounded-full text-[10px] font-semibold shrink-0"
                            style="background: color-mix(in srgb, #f59e0b 18%, transparent); color: #fbbf24"
                          >CurseForge</span>
                        {/if}
                        {#if !m.enabled}
                          <span class="ui-chip ui-chip-danger text-[10px] py-0 px-1.5 shrink-0">Выкл.</span>
                        {/if}
                      </div>
                      <p class="text-xs text-[var(--text-secondary)] truncate mt-0.5">
                        {m.version_name || m.clean_name}
                      </p>
                    </div>

                    <div class="flex items-center gap-1.5 shrink-0">
                      {#if hasUpdate}
                        <button
                          type="button"
                          on:click|stopPropagation={() => updateMod(m.filename, m.hash)}
                          class="ui-btn ui-btn-primary ui-btn-sm"
                          >Обновить</button>
                      {/if}
                      {#if confirmDeleteMod === m.filename}
                        <button
                          type="button"
                          on:click|stopPropagation={() => deleteMod(m.filename)}
                          class="ui-btn ui-btn-danger ui-btn-sm"
                          >Удалить?</button>
                        <button
                          type="button"
                          on:click|stopPropagation={() => (confirmDeleteMod = null)}
                          class="ui-btn ui-btn-ghost ui-btn-icon"
                          aria-label="Отмена"
                        ><X size={14} /></button>
                      {:else}
                        <button
                          type="button"
                          on:click|stopPropagation={() => (confirmDeleteMod = m.filename)}
                          class="ui-btn ui-btn-ghost ui-btn-icon hover:text-red-400"
                          aria-label="Удалить"
                          title="Удалить"
                        ><Trash2 size={16} /></button>
                      {/if}
                    </div>
                  </div>
                {/each}
                </div>
              {/if}
            </div>
          </div>
        {:else if instanceTab === "worlds"}
          <div class="flex flex-col h-full gap-6">
            {#if lastWorldGlobal}
              <div>
                <h3 class="text-lg font-bold text-white mb-3 flex items-center gap-2">
                  <Globe size={20} />
                  Последний мир
                </h3>
                <div class="p-3 rounded-xl bg-jm-accent/10 border border-jm-accent/30 text-white">
                  <div class="font-bold truncate">{lastWorldGlobal.world_name}</div>
                  <div class="text-xs text-[var(--text-secondary)] truncate">
                    Сборка: {lastWorldGlobal.instance_name}
                  </div>
                </div>
              </div>
            {/if}
            <div>
              <h3 class="text-lg font-bold text-white mb-3 flex items-center gap-2">
                <Globe size={20} />
                Миры
              </h3>
              <div class="flex flex-col gap-2 max-h-48 overflow-y-auto custom-scrollbar">
                {#if worldsList.length === 0}
                  <p class="text-[var(--text-secondary)] py-4">Нет сохранённых миров</p>
                {:else}
                  {#each worldsList as w (w)}
                    <button
                      type="button"
                      on:click={() => launchInstance(si, undefined, w)}
                      disabled={isLaunching || isRepairing || isBusy}
                      class="flex items-center gap-4 p-3 rounded-xl bg-black/30 border border-white/5 hover:border-jm-accent/50 hover:bg-jm-accent/10 text-left transition-colors disabled:opacity-50"
                    >
                      <Globe size={20} class="text-jm-accent shrink-0" />
                      <span class="font-bold text-white truncate">{w}</span>
                      <Play size={16} class="text-jm-accent ml-auto shrink-0" />
                    </button>
                  {/each}
                {/if}
              </div>
            </div>
            <div>
              <h3 class="text-lg font-bold text-white mb-3 flex items-center gap-2">
                <Server size={20} />
                Серверы
              </h3>
              <div class="flex flex-col gap-2 max-h-48 overflow-y-auto custom-scrollbar">
                {#if recentServers.length === 0}
                  <p class="text-[var(--text-secondary)] py-4">Нет сохранённых серверов</p>
                {:else}
                  {#each recentServers as srv, i (i)}
                    <button
                      type="button"
                      on:click={() => launchInstance(si, srv.ip)}
                      disabled={isLaunching || isRepairing || isBusy}
                      class="flex items-center gap-4 p-3 rounded-xl bg-black/30 border border-white/5 hover:border-jm-accent/50 hover:bg-jm-accent/10 text-left transition-colors disabled:opacity-50"
                    >
                      <Server size={20} class="text-jm-accent shrink-0" />
                      <div class="min-w-0 flex-1">
                        <div class="font-bold text-white truncate">{srv.name || srv.ip}</div>
                        <div class="text-xs text-[var(--text-secondary)] truncate">{srv.ip}</div>
                      </div>
                      <Play size={16} class="text-jm-accent ml-auto shrink-0" />
                    </button>
                  {/each}
                {/if}
              </div>
            </div>
          </div>
        {:else if instanceTab === "logs"}
          <div class="h-full flex flex-col min-h-0">
            <div class="flex flex-wrap items-center justify-between gap-2 mb-4">
              <h3 class="text-xl font-bold text-white shrink-0">Консоль игры</h3>
              <div class="flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  on:click={() => void copySelectedLogs()}
                  class="text-sm bg-jm-accent/20 hover:bg-jm-accent text-jm-accent hover:text-black px-3 py-2 rounded-lg transition-colors font-bold flex items-center gap-1.5"
                  title="Если есть выделение — копирует его, иначе весь лог"
                >
                  <Copy size={16} /> Копировать
                </button>
                <button
                  type="button"
                  on:click={() => void copyAllLogs()}
                  class="text-sm bg-white/10 hover:bg-white/20 px-3 py-2 rounded-lg transition-colors flex items-center gap-1.5"
                  title="Скопировать весь лог в буфер обмена"
                >
                  <Copy size={16} /> Всё
                </button>
                <button
                  type="button"
                  on:click={selectAllLogs}
                  class="text-sm bg-white/10 hover:bg-white/20 px-3 py-2 rounded-lg transition-colors flex items-center gap-1.5"
                  title="Выделить весь текст лога (можно потом Ctrl+C)"
                >
                  <TextSelect size={16} /> Выделить всё
                </button>
                <button
                  type="button"
                  on:click={() => (logs = [])}
                  class="text-sm bg-white/10 hover:bg-white/20 px-3 py-2 rounded-lg transition-colors"
                  >Очистить</button
                >
              </div>
            </div>
            <!-- tabindex: фокус для выделения с клавиатуры; select-text — явное выделение мышью -->
            <div
              bind:this={logsScrollEl}
              tabindex="0"
              role="log"
              aria-label="Текст консоли игры, можно выделять и копировать"
              class="flex-grow min-h-[12rem] bg-[var(--input-bg)] rounded-xl border border-white/10 p-4 font-mono text-xs text-[var(--text-secondary)] overflow-y-auto custom-scrollbar leading-relaxed select-text outline-none focus-visible:ring-2 focus-visible:ring-jm-accent/50"
            >
              {#each logs as l, i (i)}
                <div class={l.includes("[ERROR]") ? "text-red-400" : ""}>{l}</div>
              {/each}
              <div bind:this={logsEndEl} />
            </div>
            <p class="text-[10px] text-[var(--text-secondary)] mt-2">
              Выделяйте мышью или «Выделить всё», затем Ctrl+C или «Копировать».
            </p>
          </div>
        {:else if instanceTab === "options"}
          <div class="flex flex-col h-full">
            <div class="flex gap-4 mb-6 border-b border-white/10 pb-4">
              <button
                type="button"
                on:click={() => (settingsSubTab = "general")}
                class="font-bold transition-colors {settingsSubTab === 'general'
                  ? 'text-jm-accent'
                  : 'text-[var(--text-secondary)] hover:text-white'}">Общие</button
              >
              {#if packSourceInfo}
                <button
                  type="button"
                  on:click={() => (settingsSubTab = "pack")}
                  class="font-bold transition-colors {settingsSubTab === 'pack'
                    ? 'text-jm-accent'
                    : 'text-[var(--text-secondary)] hover:text-white'}">Сборка</button
                >
              {/if}
              <button
                type="button"
                on:click={onCoreSettingsTabClick}
                class="font-bold transition-colors {settingsSubTab === 'core'
                  ? 'text-jm-accent'
                  : 'text-[var(--text-secondary)] hover:text-white'}">Ядро (Core)</button
              >
              <button
                type="button"
                on:click={() => {
                  settingsSubTab = "advanced";
                  javaPickMenuOpen = false;
                }}
                class="font-bold transition-colors {settingsSubTab === 'advanced'
                  ? 'text-jm-accent'
                  : 'text-[var(--text-secondary)] hover:text-white'}">Расширенные</button
              >
            </div>

            {#if settingsSubTab === "general"}
              <div class="space-y-6">
                <div class="p-4 bg-black/30 rounded-2xl border border-white/5">
                  <h4 class="text-sm font-bold text-[var(--text-secondary)] mb-3">Название сборки</h4>
                  <input
                    bind:value={renameValue}
                    class="w-full bg-black/50 border border-white/10 rounded-xl px-4 py-3 text-white outline-none focus:border-jm-accent"
                    placeholder="Как отображается в списке"
                  />
                  <p class="text-[11px] text-[var(--text-secondary)] mt-2">
                    Папка на диске ({selectedInstance?.id}) не переименовывается — меняется только подпись в лаунчере.
                  </p>
                  <button
                    type="button"
                    on:click={applyRename}
                    disabled={isRunning}
                    class="mt-3 bg-jm-accent/20 hover:bg-jm-accent text-jm-accent hover:text-black px-4 py-2 rounded-xl font-bold text-sm disabled:opacity-50"
                    >Сохранить название</button
                  >
                </div>
                <label class="flex items-center gap-3 cursor-pointer p-4 bg-black/30 rounded-xl border border-white/5">
                  <input
                    type="checkbox"
                    checked={instSettings.override_global}
                    on:change={(e) =>
                      saveInstSettings({ ...instSettings, override_global: e.currentTarget.checked })}
                    class="w-5 h-5 accent-jm-accent cursor-pointer"
                  />
                  <span class="text-white font-bold">Использовать персональные настройки (ОЗУ, JVM)</span>
                </label>
                <div
                  class="space-y-6 transition-opacity {instSettings.override_global
                    ? 'opacity-100'
                    : 'opacity-30 pointer-events-none'}"
                >
                  <div>
                    <label class="text-sm text-[var(--text-secondary)] mb-2 block"
                      >Выделение ОЗУ: <strong class="text-jm-accent">{instSettings.ram_mb} MB</strong></label
                    >
                    <input
                      type="range"
                      min="1024"
                      max={maxRam}
                      step="512"
                      value={instSettings.ram_mb}
                      on:input={(e) =>
                        saveInstSettings({ ...instSettings, ram_mb: parseInt(e.currentTarget.value, 10) })}
                      class="w-full accent-jm-accent cursor-pointer"
                    />
                  </div>
                  <div>
                    <label class="text-sm text-[var(--text-secondary)] mb-2 block">Аргументы JVM</label>
                    <input
                      type="text"
                      value={instSettings.jvm_args}
                      on:input={(e) =>
                        saveInstSettings({ ...instSettings, jvm_args: e.currentTarget.value })}
                      class="w-full bg-black/50 border border-white/10 rounded-xl px-4 py-3 text-white outline-none focus:border-jm-accent font-mono text-sm transition-colors"
                    />
                  </div>
                </div>
                <label class="flex items-center gap-3 cursor-pointer p-4 bg-black/30 rounded-xl border border-white/5">
                  <input
                    type="checkbox"
                    checked={instSettings.use_discrete_gpu ?? false}
                    on:change={(e) =>
                      saveInstSettings({ ...instSettings, use_discrete_gpu: e.currentTarget.checked })}
                    class="w-5 h-5 accent-jm-accent cursor-pointer"
                  />
                  <span class="text-white font-bold">Использовать дискретную видеокарту</span>
                </label>
                <p class="text-xs text-[var(--text-secondary)] -mt-2">
                  Linux: при включении — PRIME + NVIDIA GLX; если выключено, лаунчер сбрасывает унаследованные
                  <code class="font-mono text-[10px]">__GLX_VENDOR_LIBRARY_NAME</code>
                  /
                  <code class="font-mono text-[10px]">__NV_PRIME_RENDER_OFFLOAD</code>
                  (иначе на интегрированной графике бывает GLXBadFBConfig у Forge). Windows: GPU в параметрах системы.
                </p>
              </div>
            {:else if settingsSubTab === "pack" && packSourceInfo}
              <div class="space-y-6">
                {#if (packSourceInfo.source === "modrinth" || packSourceInfo.source === "curseforge") && packVersions.length > 0}
                  <div class="p-4 bg-black/30 rounded-2xl border border-white/5">
                    <h4 class="text-sm font-bold text-[var(--text-secondary)] mb-3">Версия сборки</h4>
                    <div class="flex flex-col sm:flex-row gap-3 sm:items-stretch">
                      <div class="flex-1 min-w-0">
                        <PackVersionList
                          versions={packVersions}
                          selectedId={String(selectedPackVersion || packSourceInfo.version_id || "")}
                          onSelect={(id) => (selectedPackVersion = id)}
                          compact={true}
                        />
                      </div>
                      <button
                        type="button"
                        on:click={applyPackVersionUpdate}
                        disabled={isRunning || isUpdatingPack}
                        class="shrink-0 sm:self-start bg-jm-accent/20 hover:bg-jm-accent text-jm-accent hover:text-black px-4 py-3 rounded-xl font-bold text-sm disabled:opacity-50 min-h-[44px]"
                        >{isUpdatingPack ? "..." : "Обновить до версии"}</button
                      >
                    </div>
                  </div>
                {/if}
                <div class="p-4 bg-red-500/10 rounded-2xl border border-red-500/30">
                  <h4 class="text-sm font-bold text-red-400 mb-2">Отвязать от модпака</h4>
                  <p class="text-xs text-[var(--text-secondary)] mb-3">
                    Сборка перестанет показывать обновления. Файлы не удаляются.
                  </p>
                  <button
                    type="button"
                    on:click={unlinkPack}
                    disabled={isRunning}
                    class="bg-red-500/20 hover:bg-red-500 text-red-400 hover:text-white px-4 py-2 rounded-xl font-bold text-sm disabled:opacity-50"
                    >Отвязать</button
                  >
                </div>
              </div>
            {:else if settingsSubTab === "core"}
              <div class="space-y-6">
                <div class="flex items-center gap-5 p-5 bg-black/30 rounded-2xl border border-white/5">
                  <div
                    class="w-16 h-16 rounded-2xl bg-black/50 border border-white/20 flex items-center justify-center shrink-0 text-white/70"
                  >
                    <LoaderIcon loader={si.loader} size={32} />
                  </div>
                  <div>
                    <h4 class="text-lg font-bold text-white capitalize">
                      {si.loader === "vanilla" ? "Vanilla" : si.loader}
                    </h4>
                    <div class="flex gap-3 mt-1">
                      <span class="text-sm text-[var(--text-secondary)]"
                        >Minecraft <strong class="text-white">{si.game_version}</strong></span
                      >
                      {#if si.loader_version}
                        <span class="text-sm text-[var(--text-secondary)]"
                          >Ядро <strong class="text-jm-accent">{si.loader_version}</strong></span
                        >
                      {/if}
                    </div>
                  </div>
                </div>
                <div class="space-y-4 p-4 bg-black/20 rounded-2xl border border-white/5">
                  <h4 class="text-sm font-bold text-[var(--text-secondary)] mb-2">Сменить ядро</h4>
                  <div class="flex flex-wrap gap-4 mb-1">
                    <label class="flex items-center gap-2 cursor-pointer text-xs text-[var(--text-secondary)]">
                      <input
                        type="checkbox"
                        class="w-4 h-4 accent-jm-accent"
                        bind:checked={showMcSnapshotVersions}
                        on:change={persistMcVersionFilters}
                      />
                      Снапшоты / предрелизы
                    </label>
                    <label class="flex items-center gap-2 cursor-pointer text-xs text-[var(--text-secondary)]">
                      <input
                        type="checkbox"
                        class="w-4 h-4 accent-jm-accent"
                        bind:checked={showMcAlphaBetaVersions}
                        on:change={persistMcVersionFilters}
                      />
                      Альфа / бета (legacy)
                    </label>
                  </div>
                  <div class="grid grid-cols-3 gap-3">
                    <LibrarySelect
                      label="Загрузчик"
                      value={coreLoader || si.loader}
                      onChange={setCoreLoaderChoice}
                      options={loaderOptions}
                    />
                    <LibrarySelect
                      label="Версия игры"
                      value={coreGameVer || si.game_version}
                      onChange={setCoreGameVerChoice}
                      options={coreGameOptions}
                    />
                    {#if (coreLoader || si.loader) !== "vanilla"}
                      <LibrarySelect
                        label="Версия ядра"
                        value={coreLoaderVer || si.loader_version || ""}
                        onChange={(v) => (coreLoaderVer = v)}
                        options={coreLoaderVerOptions}
                      />
                    {/if}
                  </div>
                  <button
                    type="button"
                    on:click={applyCoreUpdate}
                    disabled={isRunning}
                    class="w-full bg-jm-accent/20 hover:bg-jm-accent text-jm-accent hover:text-black py-2.5 rounded-xl font-bold transition-colors border border-jm-accent/30 disabled:opacity-50 text-sm"
                    >Применить</button
                  >
                </div>
                {#if (packSourceInfo?.source === "modrinth" || packSourceInfo?.source === "curseforge") && packVersions.length > 0}
                  <div class="space-y-3 p-4 bg-black/20 rounded-2xl border border-white/5">
                    <h4 class="text-sm font-bold text-[var(--text-secondary)]">
                      Версия модпака ({packSourceInfo.source === "curseforge" ? "CurseForge" : "Modrinth"})
                    </h4>
                    <p class="text-xs text-[var(--text-secondary)]">
                      Сейчас:
                      <span class="text-white font-medium"
                        >{packSourceInfo.version_name || packSourceInfo.version_id || "—"}</span
                      >
                    </p>
                    <button
                      type="button"
                      on:click={() => {
                        selectedPackVersion = selectedPackVersion || packSourceInfo.version_id || "";
                        corePackVersionModalOpen = true;
                      }}
                      disabled={isRunning || isUpdatingPack}
                      class="w-full bg-white/10 hover:bg-white/15 text-white py-2.5 rounded-xl font-bold transition-colors border border-white/10 disabled:opacity-50 text-sm"
                      >Выбрать версию сборки…</button
                    >
                  </div>
                {:else if (packSourceInfo?.source === "modrinth" || packSourceInfo?.source === "curseforge") && packVersions.length === 0}
                  <div class="space-y-2 p-4 bg-amber-500/10 rounded-2xl border border-amber-500/25">
                    <h4 class="text-sm font-bold text-amber-200">Версии модпака не загрузились</h4>
                    <p class="text-xs text-[var(--text-secondary)]">
                      Сеть, API или ключ CurseForge. Текущая версия в данных сборки:
                      <span class="text-white font-medium"
                        >{packSourceInfo.version_name || packSourceInfo.version_id || "—"}</span
                      >
                    </p>
                    <button
                      type="button"
                      on:click={() => selectedInstance && loadPackSourceForSelection(selectedInstance.id)}
                      class="w-full bg-white/10 hover:bg-white/15 text-white py-2 rounded-xl font-bold text-sm border border-white/10"
                      >Повторить загрузку списка</button
                    >
                  </div>
                {:else if packSourceInfo?.source === "custom"}
                  <div class="space-y-2 p-4 bg-black/20 rounded-2xl border border-white/10">
                    <h4 class="text-sm font-bold text-[var(--text-secondary)]">Модпак по прямой ссылке</h4>
                    <p class="text-xs text-[var(--text-secondary)] leading-relaxed">
                      Обновления идут по тому же URL. Выбор другой версии из каталога Modrinth/CurseForge — переустановите
                      сборку из браузера лаунчера (тогда появится привязка к проекту).
                    </p>
                  </div>
                {/if}
                {#if packSourceInfo}
                  <div class="p-4 bg-red-500/10 rounded-2xl border border-red-500/25">
                    <h4 class="text-sm font-bold text-red-400 mb-1">Отвязать от модпака</h4>
                    <p class="text-xs text-[var(--text-secondary)] mb-3">
                      Уберётся вкладка «Сборка», обновления и выбор версии модпака здесь и в настройках.
                    </p>
                    <button
                      type="button"
                      on:click={unlinkPack}
                      disabled={isRunning}
                      class="bg-red-500/20 hover:bg-red-500 text-red-400 hover:text-white px-4 py-2 rounded-xl font-bold text-sm disabled:opacity-50"
                      >Отвязать</button
                    >
                  </div>
                {/if}
                <div class="flex gap-4 mt-4">
                  <button
                    type="button"
                    on:click={repairCore}
                    disabled={isRunning}
                    class="bg-red-500/10 hover:bg-red-500 text-red-400 hover:text-white px-6 py-3 rounded-xl font-bold flex items-center gap-2 transition-colors border border-red-500/30 disabled:opacity-50 text-sm"
                  >
                    <RefreshCw size={18} />
                    Починить ядро
                  </button>
                </div>
              </div>
            {:else if settingsSubTab === "advanced"}
              <div class="space-y-6 overflow-y-auto custom-scrollbar pr-1">
                <div class="p-4 bg-black/30 rounded-2xl border border-white/5">
                  <h4 class="text-sm font-bold text-[var(--text-secondary)] mb-2 flex items-center gap-2">
                    <Sliders size={16} class="text-jm-accent" /> Контент на диске
                  </h4>
                  {#if instanceAdvancedCounts}
                    <p class="text-sm text-white">
                      Моды: <strong class="text-jm-accent">{instanceAdvancedCounts.mods}</strong>
                      · Ресурспаки:
                      <strong class="text-jm-accent">{instanceAdvancedCounts.resourcepacks}</strong>
                      · Шейдеры:
                      <strong class="text-jm-accent">{instanceAdvancedCounts.shaderpacks}</strong>
                    </p>
                  {:else}
                    <p class="text-xs text-[var(--text-secondary)]">Не удалось загрузить счётчики.</p>
                  {/if}
                </div>

                <div class="p-4 bg-black/30 rounded-2xl border border-white/5 space-y-3 relative">
                  <h4 class="text-sm font-bold text-[var(--text-secondary)] flex items-center gap-2">
                    <Coffee size={16} class="text-jm-accent" /> Java для этой сборки
                  </h4>
                  <p class="text-xs text-[var(--text-secondary)]">
                    Перекрывает глобальный выбор. Пусто — как в общих настройках лаунчера / авто.
                  </p>
                  <div class="flex flex-wrap gap-2 items-center">
                    <button
                      type="button"
                      on:click={() => (javaPickMenuOpen = !javaPickMenuOpen)}
                      class="flex-1 min-w-[12rem] text-left bg-black/50 border border-white/10 rounded-xl px-4 py-3 text-sm font-bold text-white hover:border-jm-accent/50 transition-colors"
                    >
                      {#if instSettings.custom_java_path}
                        {javaRuntimesForInstance.find((r) => r.path === instSettings.custom_java_path)
                          ?.label || instSettings.custom_java_path}
                      {:else}
                        По умолчанию (лаунчер)
                      {/if}
                    </button>
                    {#if instSettings.custom_java_path}
                      <button
                        type="button"
                        on:click={() =>
                          saveInstSettings({ ...instSettings, custom_java_path: "" }, true)}
                        class="px-4 py-3 rounded-xl text-sm font-bold bg-white/10 hover:bg-white/20 text-[var(--text-secondary)]"
                        >Сбросить</button
                      >
                    {/if}
                  </div>
                  {#if javaPickMenuOpen}
                    <div
                      class="absolute left-4 right-4 top-full mt-1 z-30 max-h-56 overflow-y-auto custom-scrollbar rounded-xl border border-jm-accent/30 bg-[var(--input-bg)] shadow-xl py-1"
                    >
                      <button
                        type="button"
                        class="w-full text-left px-4 py-2.5 text-sm hover:bg-jm-accent/15 text-[var(--text-secondary)]"
                        on:click={() => {
                          saveInstSettings({ ...instSettings, custom_java_path: "" }, true);
                          javaPickMenuOpen = false;
                        }}>По умолчанию</button
                      >
                      {#each javaRuntimesForInstance as jr (jr.path)}
                        <button
                          type="button"
                          class="w-full text-left px-4 py-2.5 text-sm hover:bg-jm-accent/15 text-white font-medium border-t border-white/5"
                          on:click={() => {
                            saveInstSettings({ ...instSettings, custom_java_path: jr.path }, true);
                            javaPickMenuOpen = false;
                          }}>{jr.label}</button
                        >
                      {/each}
                    </div>
                  {/if}
                  <div class="flex flex-wrap gap-2 items-end pt-2 border-t border-white/5">
                    <div class="flex-1 min-w-[6rem]">
                      <label class="text-[10px] text-[var(--text-secondary)] uppercase font-bold block mb-1"
                        >Мажор Java</label
                      >
                      <input
                        type="number"
                        min="8"
                        max="25"
                        bind:value={downloadJavaMajorStr}
                        class="w-full bg-black/50 border border-white/10 rounded-xl px-3 py-2 text-white text-sm outline-none focus:border-jm-accent"
                      />
                    </div>
                    <button
                      type="button"
                      disabled={isDownloadingJavaForInstance}
                      on:click={downloadJavaForInstanceMajor}
                      class="bg-jm-accent/20 hover:bg-jm-accent text-jm-accent hover:text-black px-4 py-2 rounded-xl text-sm font-bold disabled:opacity-50 flex items-center gap-2"
                    >
                      {#if isDownloadingJavaForInstance}
                        <Loader2 size={16} class="animate-spin" />
                      {:else}
                        <Download size={16} />
                      {/if}
                      Скачать
                    </button>
                  </div>
                </div>

                <div class="p-4 bg-black/30 rounded-2xl border border-white/5 space-y-3">
                  <h4 class="text-sm font-bold text-[var(--text-secondary)] flex items-center gap-2">
                    <Archive size={16} class="text-jm-accent" /> Резервная копия
                  </h4>
                  <p class="text-xs text-[var(--text-secondary)]">
                    Архив всей папки сборки. Будет запрос пути для сохранения .zip.
                  </p>
                  <button
                    type="button"
                    disabled={isBackingUpInstance || isRunning}
                    on:click={backupCurrentInstance}
                    class="w-full bg-white/10 hover:bg-jm-accent/25 text-white py-3 rounded-xl font-bold text-sm border border-white/10 disabled:opacity-50 flex items-center justify-center gap-2"
                  >
                    {#if isBackingUpInstance}
                      <Loader2 size={18} class="animate-spin" />
                    {:else}
                      <Archive size={18} />
                    {/if}
                    Создать бэкап…
                  </button>
                </div>
              </div>
            {/if}
          </div>
        {/if}

        {#if corePackVersionModalOpen && packSourceInfo}
          <div
            use:portal
            class="fixed inset-0 z-[10060] flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm"
            transition:fade={{ duration: 200 }}
          >
            <button
              type="button"
              class="absolute inset-0 cursor-default border-0 bg-transparent p-0"
              aria-label="Закрыть"
              on:click={() => (corePackVersionModalOpen = false)}
            />
            <div
              class="relative w-full max-w-lg max-h-[85vh] flex flex-col bg-jm-card border border-white/10 rounded-2xl shadow-xl overflow-hidden"
              transition:scale={{ duration: 220, start: 0.96, easing: quintOut }}
            >
              <div class="flex justify-between items-center p-4 border-b border-white/10 shrink-0">
                <h3 class="font-bold text-white text-lg">
                  Версия сборки ({packSourceInfo?.source === "curseforge" ? "CurseForge" : "Modrinth"})
                </h3>
                <button
                  type="button"
                  on:click={() => (corePackVersionModalOpen = false)}
                  class="text-[var(--text-secondary)] hover:text-white p-2 bg-black/50 rounded-xl transition-colors"
                  ><X size={20} /></button
                >
              </div>
              <div class="p-4 overflow-y-auto custom-scrollbar flex flex-col gap-4">
                <label class="text-xs text-[var(--text-secondary)] font-bold block">Версия .mrpack</label>
                <PackVersionList
                  versions={packVersions}
                  selectedId={String(selectedPackVersion || packSourceInfo.version_id || "")}
                  onSelect={(id) => (selectedPackVersion = id)}
                  collapsible={false}
                />
                <button
                  type="button"
                  on:click={applyPackVersionFromCoreModal}
                  disabled={isRunning || isUpdatingPack}
                  class="w-full bg-jm-accent/20 hover:bg-jm-accent text-jm-accent hover:text-black py-3 rounded-xl font-bold transition-colors border border-jm-accent/30 disabled:opacity-50 text-sm"
                  >{isUpdatingPack ? "Применение…" : "Перейти на эту версию"}</button
                >
              </div>
            </div>
          </div>
        {/if}

        {#if showModBrowser}
          <div
            use:portal
            class="fixed left-0 right-0 bottom-0 z-[10056] flex bg-black/55 backdrop-blur-md"
            style="top: var(--jm-chrome-stack, 5.75rem);"
            transition:fade={{ duration: 240 }}
          >
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="hidden sm:block sm:w-[5%] md:w-[10%] lg:w-[18%] bg-black/50 backdrop-blur-sm shrink-0"
              on:click={closeModBrowser}
            />
            <div
              class="w-full sm:w-[95%] md:w-[90%] lg:w-[82%] h-full min-h-0 bg-jm-bg/98 border-l border-jm-accent/15 shadow-[0_-12px_60px_rgba(0,0,0,0.45)] flex flex-col rounded-tl-2xl overflow-hidden"
              in:fly={{ x: 380, duration: 340, easing: quintOut }}
              out:fly={{ x: 380, duration: 260, easing: quintOut }}
            >
              <div
                class="flex justify-between items-center p-3 bg-jm-card border-b border-white/10 shrink-0"
              >
                <h3 class="font-bold text-white text-sm md:text-base truncate">
                  Добавление в {si.name}
                </h3>
                <button
                  type="button"
                  on:click={closeModBrowser}
                  class="text-[var(--text-secondary)] hover:text-white p-2 bg-black/50 rounded-xl transition-colors"
                  ><X size={20} /></button
                >
              </div>
              <div class="flex-grow overflow-hidden relative">
                <DiscoverTab
                  contextInstance={si}
                  installedMods={discoverInstalledModSlugs}
                  installedModSummaries={installedModSummariesForDiscover}
                  installedContentProjectIds={installedDiscoverContentProjectIds}
                  libraryEmbedMode={true}
                  openDiscoverFromContentShortcut={discoverFromContentRow}
                  onModsChanged={reloadAllInstanceContent}
                  initialProjectId={openModProjectId}
                  onInitialProjectOpened={() => {
                    openModProjectId = undefined;
                  }}
                />
              </div>
            </div>
          </div>
        {/if}

        {#if showExportModal}
          <ExportModal
            instanceId={si.id}
            instanceName={si.name}
            onClose={() => (showExportModal = false)}
            {showToast}
          />
        {/if}
      </div>
    </div>
    </div>
  {/key}
{:else}
  <div class="jm-container flex flex-col h-full min-h-0">
    <div class="flex justify-between items-center mb-4 flex-wrap gap-3 shrink-0">
      <h2 class="text-xl md:text-2xl font-bold text-jm-accent-light">Ваши сборки</h2>
      <div class="flex items-center gap-2 flex-wrap flex-1 justify-end min-w-0">
        <div
          class="flex items-center gap-2 min-w-[140px] max-w-[220px] flex-1 bg-black/25 border border-white/10 rounded-xl px-2 py-1.5"
        >
          <Search size={14} class="text-[var(--text-secondary)] shrink-0" />
          <input
            type="search"
            bind:value={instanceSearchQuery}
            placeholder="Поиск…"
            class="w-full bg-transparent text-xs text-white placeholder:text-[var(--text-secondary)] outline-none"
          />
        </div>
        <div class="w-36">
          <LibrarySelect
            label=""
            value={globalFilter}
            onChange={(v) => (globalFilter = v)}
            options={[
              { value: "all", label: "Все" },
              { value: "vanilla", label: "Vanilla" },
              { value: "fabric", label: "Fabric" },
              { value: "forge", label: "Forge" },
              { value: "neoforge", label: "NeoForge" },
              { value: "quilt", label: "Quilt" },
              ...(enableAlphaLoaders
                ? [
                    { value: "liteloader", label: "LiteLoader (α)" },
                    { value: "modloader", label: "ModLoader (α)" },
                  ]
                : []),
            ]}
          />
        </div>
        <button
          type="button"
          on:click={importInstance}
          class="bg-white/10 hover:bg-white/20 text-white px-3 py-2 rounded-lg font-bold text-xs flex items-center gap-1.5 transition-colors"
        >
          <PackageOpen size={14} />
          Импорт
        </button>
        <button
          type="button"
          on:click={() => {
            isCreating = true;
          }}
          class="bg-jm-accent hover:bg-jm-accent-light text-black px-3 py-2 rounded-lg font-bold text-xs flex items-center gap-1.5 transition-colors"
          ><Plus size={14} />
          Создать</button
        >
      </div>
    </div>

    <div
      class="flex-1 min-h-0 overflow-y-auto custom-scrollbar pb-6 pr-0.5"
    >
      <div
        class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 [grid-auto-rows:1fr]"
      >
      {#each displayedInstances as inst, idx (inst.id)}
        {@const isRunning = runningInstances.includes(inst.id)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          in:fly={{ y: 16, duration: 320, delay: Math.min(idx * 40, 350), easing: quintOut }}
          on:click={() => (selectedInstance = inst)}
          class="bg-jm-card border border-white/10 rounded-xl p-3 flex flex-col min-h-[168px] h-full hover:border-jm-accent/60 transition-all duration-300 cursor-pointer group relative overflow-hidden hover:shadow-[0_14px_40px_rgba(134,168,134,0.12)] hover:-translate-y-1 active:scale-[0.99] jm-tap-scale card-hover-subtle"
        >
          <div class="flex items-center gap-3 mb-3 relative z-10">
            {#if instanceIconSrc(inst.icon)}
              <img
                src={instanceIconSrc(inst.icon) || ""}
                alt=""
                class="w-10 h-10 rounded-lg object-cover shadow-inner border border-white/10 shrink-0"
                on:error={onListCardIconError}
              />
            {/if}
            <div
              class="w-10 h-10 rounded-lg bg-black/50 border border-white/20 flex items-center justify-center text-sm font-medium text-white/70 shrink-0 {instanceIconSrc(
                inst.icon,
              )
                ? 'hidden'
                : ''}"
            >
              {inst.name?.charAt(0)?.toUpperCase() || "?"}
            </div>
            <div class="min-w-0">
              <h3 class="text-sm font-bold text-white truncate">{inst.name}</h3>
              <div class="flex gap-1.5 flex-wrap">
                <span class="text-[10px] bg-white/10 px-1.5 py-0.5 rounded text-[var(--text-secondary)] capitalize"
                  >{inst.loader}</span
                >
                <span class="text-[10px] bg-white/10 px-1.5 py-0.5 rounded text-[var(--text-secondary)]"
                  >{inst.game_version}</span
                >
              </div>
              {#if (inst.playtime ?? 0) > 0}
                <p class="text-[10px] text-[var(--text-secondary)] mt-1.5">
                  Наиграно: {formatPlaytimeSeconds(inst.playtime)}
                </p>
              {/if}
            </div>
          </div>
          <div class="mt-auto flex gap-1.5 relative z-10">
            <button
              type="button"
              on:click|stopPropagation={() => launchInstance(inst)}
              disabled={isLaunching || isRepairing || busyInstanceId === inst.id}
              class="flex-1 font-bold py-2 rounded-lg text-xs flex items-center justify-center gap-1.5 transition-colors border disabled:opacity-50 disabled:cursor-not-allowed {isRunning
                ? 'bg-red-500/20 text-red-500 border-red-500/30 hover:bg-red-500 hover:text-white'
                : 'bg-jm-accent/10 text-jm-accent border-jm-accent/30 hover:bg-jm-accent hover:text-black'}"
            >
              {#if busyInstanceId === inst.id}
                <Loader2 size={14} class="animate-spin" />
                ...
              {:else if isRunning}
                <Square size={14} fill="currentColor" />
                СТОП
              {:else}
                <Play size={14} fill="currentColor" />
                ИГРАТЬ
              {/if}
            </button>
            <button
              type="button"
              class="w-9 bg-white/5 hover:bg-white/10 text-[var(--text-secondary)] hover:text-white rounded-lg flex items-center justify-center transition-colors border border-white/10"
            >
              <Wrench size={14} />
            </button>
          </div>
        </div>
      {/each}
      {#if displayedInstances.length === 0}
        <div
          class="col-span-full flex flex-col items-center justify-center py-16 px-6 text-center rounded-2xl border border-white/10 bg-jm-card/50"
        >
          {#if instances.length === 0}
            <p class="text-[var(--text-secondary)] text-sm mb-2 max-w-md">
              Список сборок пуст или не удалось его загрузить (например, во время установки пака).
            </p>
            <button
              type="button"
              on:click={() => void loadData()}
              class="mt-2 px-4 py-2 rounded-xl bg-jm-accent/20 hover:bg-jm-accent text-jm-accent hover:text-black font-bold text-sm flex items-center gap-2 border border-jm-accent/30 transition-colors"
            >
              <RefreshCw size={16} />
              Обновить список
            </button>
          {:else}
            <p class="text-[var(--text-secondary)] text-sm">
              Нет сборок по текущему фильтру или поиску.
            </p>
          {/if}
        </div>
      {/if}
      </div>
    </div>

    {#if isCreating}
      <div
        class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4"
        transition:fade={{ duration: 200 }}
      >
        <div
          class="bg-jm-card border border-white/10 p-8 rounded-3xl w-full max-w-md shadow-2xl"
          in:scale={{ duration: 280, start: 0.96, easing: quintOut }}
          out:scale={{ duration: 200, start: 0.96, easing: quintOut }}
        >
          <h3 class="text-2xl font-bold text-white mb-6">Новая сборка</h3>
          <div class="space-y-4 mb-8">
            <div class="flex items-center gap-4">
              <button
                type="button"
                on:click={pickNewInstanceIcon}
                class="w-16 h-16 shrink-0 rounded-xl border-2 border-dashed border-white/20 hover:border-jm-accent/50 flex items-center justify-center bg-black/30 overflow-hidden transition-colors"
                title="Выбрать иконку"
              >
                {#if newIcon}
                  <img src={convertFileSrc(newIcon)} class="w-full h-full object-cover" alt="" />
                {:else}
                  <Plus size={20} class="text-[var(--text-secondary)]" />
                {/if}
              </button>
              <div class="flex-1">
                <label class="text-sm text-[var(--text-secondary)] mb-1 block" for="lib-new-name">Название</label>
                <input
                  id="lib-new-name"
                  type="text"
                  placeholder="Например: Выживание с модами"
                  bind:value={newName}
                  class="w-full bg-black/50 border border-white/10 rounded-xl px-4 py-3 text-white outline-none focus:border-jm-accent transition-colors"
                />
              </div>
            </div>
            <LibrarySelect
              label="Ядро (Загрузчик)"
              value={newLoader}
              onChange={(v) => (newLoader = v)}
              options={newLoaderOptions}
            />
            <div class="flex flex-col gap-2 p-3 bg-black/25 rounded-xl border border-white/10">
              <span class="text-xs font-bold text-[var(--text-secondary)] uppercase tracking-wide"
                >Видимость версий Minecraft</span
              >
              <label class="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  bind:checked={showMcSnapshotVersions}
                  on:change={persistMcVersionFilters}
                  class="w-4 h-4 accent-jm-accent cursor-pointer shrink-0"
                />
                <span class="text-sm text-[var(--text-secondary)]"
                  >Снапшоты и предрелизы (Modrinth: snapshot; Mojang: snapshot)</span
                >
              </label>
              <label class="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  bind:checked={showMcAlphaBetaVersions}
                  on:change={persistMcVersionFilters}
                  class="w-4 h-4 accent-jm-accent cursor-pointer shrink-0"
                />
                <span class="text-sm text-[var(--text-secondary)]"
                  >Альфа и бета (Modrinth: alpha/beta; Mojang: old_alpha / old_beta)</span
                >
              </label>
            </div>
            <div class="flex gap-4">
              <div class="flex-1">
                {#if isLoadingVersions}
                  <div class="h-[72px] flex items-end">
                    <div
                      class="w-full bg-black/50 border border-white/10 rounded-xl px-4 py-3 text-[var(--text-secondary)] flex items-center gap-2"
                    >
                      <Loader2 class="animate-spin" size={16} />
                      Загрузка...
                    </div>
                  </div>
                {:else}
                  <LibrarySelect
                    label="Версия игры"
                    value={newVersion}
                    onChange={(v) => (newVersion = v)}
                    options={availableVersions.map((v) => ({ value: v, label: v }))}
                  />
                {/if}
              </div>
              {#if newLoader !== "vanilla"}
                <div class="flex-1">
                  {#if isLoadingLoaderVersions}
                    <div class="h-[72px] flex items-end">
                      <div
                        class="w-full bg-black/50 border border-white/10 rounded-xl px-4 py-3 text-[var(--text-secondary)] flex items-center gap-2"
                      >
                        <Loader2 class="animate-spin" size={16} />
                        Загрузка...
                      </div>
                    </div>
                  {:else}
                    <LibrarySelect
                      label="Версия ядра"
                      value={newLoaderVersion}
                      onChange={(v) => (newLoaderVersion = v)}
                      options={availableLoaderVersions.map((v) => ({ value: v, label: v }))}
                      disabled={availableLoaderVersions.length === 0}
                    />
                  {/if}
                </div>
              {/if}
            </div>
          </div>
          <div class="flex gap-4">
            <button
              type="button"
              on:click={() => (isCreating = false)}
              class="flex-1 py-3 rounded-xl font-bold bg-white/5 hover:bg-white/10 text-white transition-colors"
              >Отмена</button
            >
            <button
              type="button"
              on:click={handleCreate}
              disabled={!newName || isLoadingVersions || isLoadingLoaderVersions}
              class="flex-1 py-3 rounded-xl font-bold bg-jm-accent text-black hover:bg-jm-accent-light transition-colors disabled:opacity-50"
              >Создать</button
            >
          </div>
        </div>
      </div>
    {/if}
  </div>
{/if}

<FileBrowserModal
  open={fileBrowserOpen}
  initialRelPath={fileBrowserInitialPath}
  onClose={() => (fileBrowserOpen = false)}
/>
