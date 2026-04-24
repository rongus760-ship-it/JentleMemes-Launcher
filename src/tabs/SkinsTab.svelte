<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { listen, emit } from "@tauri-apps/api/event";
  import { fly, scale, fade } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import { SkinViewer, WalkingAnimation } from "skinview3d";
  import SkinBody2D from "../components/SkinBody2D.svelte";
  import {
    ChevronLeft,
    ChevronRight,
    Plus,
    Check,
    Trash2,
    Save,
    X,
    Upload,
    Loader2,
    Box,
    Image as ImageIcon,
    Shirt,
  } from "lucide-svelte";

  type Profiles = {
    accounts: any[];
    active_account_id: string;
    skin_presets: any[];
  };

  let profiles: Profiles = { accounts: [], active_account_id: "", skin_presets: [] };
  let activeAccount: any = null;
  let previewSkin: any = null;
  let use3D = false;
  let canvasEl: HTMLCanvasElement;
  let viewer: SkinViewer | null = null;
  let scrollEl: HTMLDivElement;
  let canScrollLeft = false;
  let canScrollRight = false;
  let editingPreset: any = null;
  let editName = "";
  let editUsername = "";
  let pngImportReading = false;
  let pngImportSaving = false;
  let pendingPngImport: { dataUrl: string; suggestedName: string } | null = null;
  let pngImportName = "";
  let editModel: "default" | "slim" = "default";
  let editCapeUrl = "";
  let editCapeType: "none" | "url" | "local" = "none";
  /** Текстура с session-сервера для текущего аккаунта (Mojang / Ely), если ник в пресете совпадает */
  let resolvedSession: { url: string; slim: boolean } | null = null;
  /** Не сбрасываем при кратком обнулении resolvedSession (наведение на чужой пресет → свой). */
  let lastResolvedSession: { url: string; slim: boolean } | null = null;
  let prevActiveAccountId = "";
  let resolveTimer: ReturnType<typeof setTimeout> | undefined;
  let resolveGen = 0;
  let applyingPreset = false;
  /** Миниатюры пресетов «по нику» — URL полной текстуры (Mojang API), не minotar 64×32 */
  let presetRemoteTextureUrls: Record<string, string> = {};
  let mojangResolveTimer: ReturnType<typeof setTimeout> | undefined;
  let mojangResolveGen = 0;
  let resolvedMojangNick: { url: string; slim: boolean } | null = null;
  let mojangNickQuery = "";
  /** Пока новый запрос Mojang в полёте — не откатываемся на minotar (убирает мигание). */
  let stickyMojangUrl = "";
  let stickyMojangForNick = "";
  let skinDropHover = false;

  let unlistenProfiles: (() => void) | undefined;

  function inferAccType(acc: any): string {
    const t = String(acc?.acc_type || "").trim();
    if (t) return t;
    if (acc.id?.startsWith("ms-")) return "microsoft";
    if (acc.id?.startsWith("elyby-")) return "elyby";
    if (acc.id?.startsWith("offline-")) return "offline";
    return "offline";
  }

  /** Офлайн: скин в игре не меняется через сервисы — не показываем редактор. */
  $: skinsDisabledOffline =
    !!activeAccount && inferAccType(activeAccount) === "offline";

  /** Запасной URL, если Mojang API недоступен (часто 64×32 без слоя). */
  function remoteSkinPngUrl(nick: string, _acc: any): string {
    const n = encodeURIComponent(String(nick || "Steve").trim() || "Steve");
    return `https://minotar.net/skin/${n}.png`;
  }

  function patchSessionThumbnails() {
    const sessionU = lastResolvedSession?.url;
    if (!sessionU || !activeAccount) return;
    const acc = (activeAccount.username || "").trim().toLowerCase();
    const next = { ...presetRemoteTextureUrls };
    let changed = false;
    for (const p of profiles.skin_presets) {
      if (p.skin_type === "local") continue;
      const n = String(p.skin_data || p.username || "").trim().toLowerCase();
      if (n === acc && next[p.id] !== sessionU) {
        next[p.id] = sessionU;
        changed = true;
      }
    }
    if (changed) presetRemoteTextureUrls = next;
  }

  async function refreshPresetRemoteThumbnails() {
    const m: Record<string, string> = { ...presetRemoteTextureUrls };
    const accName = (activeAccount?.username || "").trim().toLowerCase();
    const sessionU = lastResolvedSession?.url;
    for (const p of profiles.skin_presets) {
      if (p.skin_type === "local") continue;
      const n = String(p.skin_data || p.username || "").trim();
      if (!n) continue;
      if (sessionU && accName && n.toLowerCase() === accName) {
        m[p.id] = sessionU;
        continue;
      }
      try {
        const r: any = await invoke("resolve_skin_texture_by_username", { username: n });
        if (r?.url) m[p.id] = r.url;
      } catch {
        /* оставить старый / minotar в разметке */
      }
    }
    presetRemoteTextureUrls = m;
  }

  function scheduleResolveMojangNickFor(nick: string) {
    mojangResolveGen++;
    const g = mojangResolveGen;
    const n = nick.trim();
    if (!n) {
      resolvedMojangNick = null;
      mojangNickQuery = "";
      return;
    }
    if (mojangResolveTimer) clearTimeout(mojangResolveTimer);
    mojangResolveTimer = setTimeout(async () => {
      if (g !== mojangResolveGen) return;
      try {
        const r: any = await invoke("resolve_skin_texture_by_username", { username: n });
        if (g !== mojangResolveGen) return;
        if (r?.url) {
          resolvedMojangNick = { url: r.url, slim: !!r.slim };
          mojangNickQuery = n;
          stickyMojangUrl = r.url;
          stickyMojangForNick = n;
        } else {
          resolvedMojangNick = null;
          mojangNickQuery = n;
        }
      } catch {
        if (g === mojangResolveGen) {
          resolvedMojangNick = null;
          mojangNickQuery = n;
        }
      }
    }, 280);
  }

  /** Mojang/Minotar отдают CORS для WebGL; Ely и часть зеркал — нет, там null. */
  function crossOriginForRemoteSkinUrl(url: string): string | null {
    const u = url.toLowerCase();
    if (
      u.includes("textures.minecraft.net") ||
      u.includes("mojang.com") ||
      u.includes("minecraft.net") ||
      u.includes("crafthead.net") ||
      u.includes("minotar.net")
    ) {
      return "anonymous";
    }
    return null;
  }

  function skinTexSourceForView3d(url: string): string | { src: string; crossOrigin: string | null } {
    if (url && /^https?:\/\//i.test(url)) {
      const cm = crossOriginForRemoteSkinUrl(url);
      return { src: url, crossOrigin: cm };
    }
    return url;
  }

  let viewerGen = 0;

  function scheduleResolveSession() {
    resolveGen++;
    const g = resolveGen;
    if (resolveTimer) clearTimeout(resolveTimer);
    resolveTimer = setTimeout(async () => {
      if (g !== resolveGen) return;
      try {
        const r: any = await invoke("resolve_session_skin", {
          uuid: activeAccount.uuid,
          accountType: inferAccType(activeAccount),
          username: String(activeAccount.username || "").trim(),
        });
        if (g !== resolveGen) return;
        if (r?.url) {
          resolvedSession = { url: r.url, slim: !!r.slim };
          lastResolvedSession = resolvedSession;
          patchSessionThumbnails();
        } else resolvedSession = null;
      } catch {
        if (g === resolveGen) resolvedSession = null;
      }
    }, 220);
  }

  /** После наведения на пресет — вернуть превью к надетому скину (без полного reload). */
  function restorePreviewFromApplied() {
    if (editingPreset) return;
    const active = profiles.accounts.find((a: any) => a.id === profiles.active_account_id);
    if (!active?.active_skin_id) {
      previewSkin = null;
      return;
    }
    const applied = profiles.skin_presets.find((p: any) => p.id === active.active_skin_id);
    previewSkin = applied || null;
  }

  async function loadProfiles() {
    const data: any = await invoke("load_profiles");
    if (!data.skin_presets) data.skin_presets = [];
    profiles = data;
    const active = data.accounts.find((a: any) => a.id === data.active_account_id);
    const newAid = active?.id || "";
    if (newAid !== prevActiveAccountId) {
      prevActiveAccountId = newAid;
      lastResolvedSession = null;
      resolvedSession = null;
    }
    activeAccount = active || null;
    if (!active && !editingPreset) {
      previewSkin = null;
    } else if (active && !editingPreset) {
      if (active.active_skin_id) {
        const appliedSkin = data.skin_presets.find((p: any) => p.id === active.active_skin_id);
        previewSkin = appliedSkin || null;
      } else {
        previewSkin = null;
      }
    }
    await tick();
    checkScroll();
    void refreshPresetRemoteThumbnails();
  }

  onMount(() => {
    void loadProfiles();
    listen("profiles_updated", loadProfiles).then((f) => (unlistenProfiles = f));
  });

  onDestroy(() => {
    unlistenProfiles?.();
    if (resolveTimer) clearTimeout(resolveTimer);
    if (mojangResolveTimer) clearTimeout(mojangResolveTimer);
    if (viewer) {
      viewer.dispose();
      viewer = null;
    }
  });

  function disposeViewer() {
    if (viewer) {
      viewer.dispose();
      viewer = null;
    }
  }

  $: {
    if (
      !previewSkin ||
      previewSkin.skin_type === "local" ||
      !activeAccount?.uuid
    ) {
      resolvedSession = null;
      if (resolveTimer) clearTimeout(resolveTimer);
    } else {
      const at = inferAccType(activeAccount);
      if (at !== "microsoft" && at !== "elyby") {
        resolvedSession = null;
        if (resolveTimer) clearTimeout(resolveTimer);
      } else {
        const nick = (previewSkin.skin_data || previewSkin.username || "").trim();
        const same =
          nick.toLowerCase() === (activeAccount.username || "").trim().toLowerCase();
        if (!same) {
          resolvedSession = null;
          if (resolveTimer) clearTimeout(resolveTimer);
        } else {
          scheduleResolveSession();
        }
      }
    }
  }

  $: nickForPreview = previewSkin
    ? String(previewSkin.skin_data || previewSkin.username || "Steve").trim() || "Steve"
    : "";

  $: {
    const n = nickForPreview || "Steve";
    if (n !== stickyMojangForNick) {
      stickyMojangForNick = n;
      stickyMojangUrl = "";
    }
  }

  $: sameNickAsAccount =
    !!previewSkin &&
    previewSkin.skin_type !== "local" &&
    !!activeAccount &&
    nickForPreview.toLowerCase() === (activeAccount.username || "").trim().toLowerCase();

  $: useSessionTexture =
    sameNickAsAccount &&
    (inferAccType(activeAccount) === "microsoft" || inferAccType(activeAccount) === "elyby");

  $: sessionTextureUrl =
    useSessionTexture && resolvedSession
      ? resolvedSession.url
      : useSessionTexture && lastResolvedSession
        ? lastResolvedSession.url
        : null;

  $: {
    if (!previewSkin || previewSkin.skin_type === "local") {
      resolvedMojangNick = null;
      mojangNickQuery = "";
      if (mojangResolveTimer) clearTimeout(mojangResolveTimer);
    } else if (sameNickAsAccount && useSessionTexture) {
      resolvedMojangNick = null;
      mojangNickQuery = "";
      if (mojangResolveTimer) clearTimeout(mojangResolveTimer);
    } else {
      scheduleResolveMojangNickFor(nickForPreview || "Steve");
    }
  }

  /** Полная PNG: сессия → Mojang API по нику → закреплённый URL → minotar. */
  $: skinTextureUrl = previewSkin
    ? previewSkin.skin_type === "local"
      ? previewSkin.skin_data
      : sessionTextureUrl ||
          (mojangNickQuery === nickForPreview && resolvedMojangNick?.url
            ? resolvedMojangNick.url
            : "") ||
          (stickyMojangForNick === nickForPreview && stickyMojangUrl ? stickyMojangUrl : "") ||
          remoteSkinPngUrl(nickForPreview, activeAccount)
    : "";

  $: skinUrl3d = skinTextureUrl;

  $: sessionForModel = resolvedSession || lastResolvedSession;

  $: mojangModelFromApi =
    !(sameNickAsAccount && useSessionTexture) &&
    mojangNickQuery === nickForPreview &&
    resolvedMojangNick
      ? resolvedMojangNick.slim
        ? "slim"
        : "default"
      : null;

  $: model3d =
    previewSkin?.skin_type === "local"
      ? previewSkin?.model === "slim"
        ? "slim"
        : "default"
      : useSessionTexture && sessionForModel
        ? sessionForModel.slim
          ? "slim"
          : "default"
        : mojangModelFromApi !== null
          ? mojangModelFromApi
          : previewSkin?.model === "slim"
            ? "slim"
            : "default";

  $: {
    const ready = !!(canvasEl && use3D && previewSkin && skinUrl3d);
    if (!use3D || !previewSkin || !skinUrl3d) {
      queueMicrotask(() => disposeViewer());
    }
    if (ready) {
      const gen = ++viewerGen;
      const url = skinUrl3d;
      const m = model3d;
      const cape = previewSkin.cape_url ? String(previewSkin.cape_url) : "";
      void tick().then(() => {
        if (gen !== viewerGen) return;
        if (!canvasEl || !use3D || !previewSkin || skinUrl3d !== url) return;
        disposeViewer();
        try {
          const v = new SkinViewer({
            canvas: canvasEl,
            width: 260,
            height: 360,
            skin: skinTexSourceForView3d(url),
            model: m === "slim" ? "slim" : "default",
          });
          v.animation = new WalkingAnimation();
          v.autoRotate = true;
          v.autoRotateSpeed = 0.5;
          if (cape) v.loadCape(skinTexSourceForView3d(cape));
          if (gen !== viewerGen) {
            v.dispose();
            return;
          }
          viewer = v;
        } catch (e) {
          console.error("Ошибка WebGL:", e);
          use3D = false;
        }
      });
    }
  }

  function checkScroll() {
    if (!scrollEl) return;
    const { scrollLeft, scrollWidth, clientWidth } = scrollEl;
    canScrollLeft = scrollLeft > 0;
    canScrollRight = Math.ceil(scrollLeft + clientWidth) < scrollWidth - 2;
  }

  function scrollDir(direction: "left" | "right") {
    if (!scrollEl) return;
    scrollEl.scrollBy({ left: direction === "left" ? -200 : 200, behavior: "smooth" });
    setTimeout(checkScroll, 320);
  }

  async function saveProfilesData(newProfiles: Profiles) {
    const fresh: any = await invoke("load_profiles");
    const byId = new Map<string, any>(fresh.accounts.map((a: any) => [a.id, a]));
    const mergedAccounts = newProfiles.accounts.map((a: any) => {
      const prev = byId.get(a.id) || {};
      const merged = { ...prev, ...a };
      if (!merged.acc_type) merged.acc_type = inferAccType(merged);
      return merged;
    });
    const next = { ...newProfiles, accounts: mergedAccounts };
    profiles = next;
    await invoke("save_profiles", { profiles: next });
    await emit("profiles_updated");
  }

  async function handleApplySkin(presetId: string) {
    if (!activeAccount || applyingPreset) return;
    if (inferAccType(activeAccount) === "offline") {
      window.dispatchEvent(
        new CustomEvent("jm_toast", {
          detail: "Для офлайн-аккаунта скин в Minecraft не меняется через лаунчер.",
        }),
      );
      return;
    }
    const preset = profiles.skin_presets.find((p: any) => p.id === presetId);
    const accType = inferAccType(activeAccount);
    applyingPreset = true;
    try {
      const nick = String(preset?.skin_data || preset?.username || "").trim();
      const isRemoteNick =
        preset &&
        preset.skin_type !== "local" &&
        !!nick &&
        (accType === "microsoft" || accType === "elyby");
      /** Ely.by не отдаёт публичный API смены скина по accessToken игры (Chrly /auth — другой JWT). */
      const elySkinCabinet = "https://account.ely.by/profile";
      if (accType === "elyby" && (preset?.skin_type === "local" || isRemoteNick)) {
        await openUrl(elySkinCabinet);
        window.dispatchEvent(
          new CustomEvent("jm_toast", {
            detail:
              "Ely.by: смена скина только на сайте (открыта страница аккаунта). В лаунчере сохранён выбранный пресет для превью.",
          }),
        );
      } else if (preset?.skin_type === "local" && accType === "microsoft") {
        await invoke("upload_skin_mojang_for_account", {
          accountId: activeAccount.id,
          pngBase64: preset.skin_data,
          slim: preset.model === "slim",
        });
        window.dispatchEvent(
          new CustomEvent("jm_toast", {
            detail: "Скин отправлен в профиль Minecraft",
          }),
        );
      } else if (isRemoteNick && accType === "microsoft") {
        await invoke("upload_skin_from_remote_username_for_account", {
          accountId: activeAccount.id,
          username: nick,
          slim: preset.model === "slim",
        });
        window.dispatchEvent(
          new CustomEvent("jm_toast", {
            detail: "Скин с ника загружен в профиль Minecraft",
          }),
        );
      }
      const updatedAccounts = profiles.accounts.map((acc: any) =>
        acc.id === activeAccount.id ? { ...acc, active_skin_id: presetId } : acc
      );
      await saveProfilesData({ ...profiles, accounts: updatedAccounts });
      closeEditor();
    } catch (e) {
      window.dispatchEvent(new CustomEvent("jm_toast", { detail: `${e}` }));
    } finally {
      applyingPreset = false;
    }
  }

  function processSkinPngFile(file: File) {
    if (skinsDisabledOffline) {
      window.dispatchEvent(
        new CustomEvent("jm_toast", {
          detail: "Для офлайн-аккаунта загрузка скина недоступна.",
        }),
      );
      return;
    }
    if (!file.name.toLowerCase().endsWith(".png") && !file.type.includes("png")) {
      window.dispatchEvent(new CustomEvent("jm_toast", { detail: "Нужен файл PNG скина" }));
      return;
    }
    pngImportReading = true;
    const reader = new FileReader();
    reader.onload = (ev) => {
      pngImportReading = false;
      const dataUrl = String(ev.target?.result ?? "");
      const suggested = file.name.replace(/\.png$/i, "") || "Скин";
      pendingPngImport = { dataUrl, suggestedName: suggested };
      pngImportName = suggested;
    };
    reader.onerror = () => {
      pngImportReading = false;
      window.dispatchEvent(new CustomEvent("jm_toast", { detail: "Не удалось прочитать файл" }));
    };
    reader.readAsDataURL(file);
  }

  function cancelPngImport() {
    pendingPngImport = null;
    pngImportName = "";
  }

  async function confirmPngImport() {
    if (!pendingPngImport || pngImportSaving) return;
    const name = pngImportName.trim() || pendingPngImport.suggestedName;
    pngImportSaving = true;
    try {
      const newPreset = {
        id: "preset_" + Date.now(),
        name,
        skin_type: "local",
        skin_data: pendingPngImport.dataUrl,
        model: "default" as const,
      };
      await saveProfilesData({ ...profiles, skin_presets: [...profiles.skin_presets, newPreset] });
      cancelPngImport();
      closeEditor();
    } catch (e) {
      window.dispatchEvent(new CustomEvent("jm_toast", { detail: `${e}` }));
    } finally {
      pngImportSaving = false;
    }
  }

  function handleFileUpload(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    processSkinPngFile(file);
    input.value = "";
  }

  function onSkinFileDrop(e: DragEvent) {
    e.preventDefault();
    skinDropHover = false;
    const file = e.dataTransfer?.files?.[0];
    if (file) processSkinPngFile(file);
  }

  function handleCapeFileUpload(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (ev) => {
      const url = String(ev.target?.result ?? "");
      editCapeUrl = url;
      editCapeType = "local";
      if (previewSkin) previewSkin = { ...previewSkin, cape_url: url, cape_type: "local" };
    };
    reader.readAsDataURL(file);
    input.value = "";
  }

  function openEditor(preset: any = null) {
    if (skinsDisabledOffline) {
      window.dispatchEvent(
        new CustomEvent("jm_toast", {
          detail: "Редактор скинов для офлайн-аккаунта отключён.",
        }),
      );
      return;
    }
    if (preset) {
      editingPreset = preset;
      previewSkin = preset;
      editName = preset.name;
      editUsername = preset.skin_data || preset.username || "";
      editModel = preset.model || "default";
      editCapeUrl = preset.cape_url || "";
      editCapeType = preset.cape_type || "none";
    } else {
      editingPreset = { id: "new" };
      editName = "Новый скин";
      editUsername = "Notch";
      editModel = "default";
      editCapeUrl = "";
      editCapeType = "none";
      previewSkin = { skin_type: "nickname", skin_data: "Notch" };
    }
  }

  function closeEditor() {
    editingPreset = null;
    void loadProfiles();
  }

  async function handleSaveNickname() {
    if (!editingPreset) return;
    if (!editName.trim() || (editingPreset.skin_type !== "local" && !editUsername.trim())) return;
    const capeUrl = editCapeType === "none" ? "" : editCapeUrl;
    let newPresets = [...profiles.skin_presets];
    if (editingPreset.id === "new") {
      newPresets.push({
        id: "preset_" + Date.now(),
        name: editName,
        skin_type: "nickname",
        skin_data: editUsername,
        model: editModel,
        cape_url: capeUrl,
        cape_type: editCapeType,
      });
    } else {
      newPresets = newPresets.map((p) =>
        p.id === editingPreset.id
          ? {
              ...p,
              name: editName,
              skin_data: p.skin_type === "local" ? p.skin_data : editUsername,
              model: editModel,
              cape_url: capeUrl,
              cape_type: editCapeType,
            }
          : p
      );
    }
    await saveProfilesData({ ...profiles, skin_presets: newPresets });
    closeEditor();
  }

  async function handleDelete(id: string) {
    const newPresets = profiles.skin_presets.filter((p: any) => p.id !== id);
    let newAccounts = [...profiles.accounts];
    if (activeAccount && activeAccount.active_skin_id === id)
      newAccounts = newAccounts.map((a) =>
        a.id === activeAccount.id ? { ...a, active_skin_id: "" } : a
      );
    await saveProfilesData({ ...profiles, skin_presets: newPresets, accounts: newAccounts });
    closeEditor();
  }

  function updatePreviewNickname(v: string) {
    editUsername = v;
    previewSkin = {
      skin_type: "nickname",
      skin_data: v || "Steve",
      model: editModel,
      cape_url: editCapeType !== "none" ? editCapeUrl : "",
    };
  }

  function setModel(m: "default" | "slim") {
    editModel = m;
    if (previewSkin) previewSkin = { ...previewSkin, model: m };
  }

  function setCapeNone() {
    editCapeType = "none";
    editCapeUrl = "";
    if (previewSkin) previewSkin = { ...previewSkin, cape_url: "", cape_type: "none" };
  }

  function setCapeUrlType() {
    editCapeType = "url";
  }

  function setCapeLocalType() {
    editCapeType = "local";
  }

  function updateCapeUrlInput(v: string) {
    editCapeUrl = v;
    if (previewSkin) previewSkin = { ...previewSkin, cape_url: v, cape_type: "url" };
  }

  function onWindowKeydownPngImport(e: KeyboardEvent) {
    if (e.key === "Escape" && pendingPngImport) cancelPngImport();
  }
</script>

<svelte:window on:keydown={onWindowKeydownPngImport} />

<div class="ui-page jm-container">
  <div class="flex items-center justify-between gap-3 shrink-0 jm-reveal">
    <h2 class="ui-heading text-lg">Скины</h2>
    <div class="ui-seg">
      <button
        type="button"
        class="ui-seg-item"
        class:is-active={!use3D}
        on:click={() => (use3D = false)}
      >
        <ImageIcon size={13} /> 2D
      </button>
      <button
        type="button"
        class="ui-seg-item"
        class:is-active={use3D}
        on:click={() => (use3D = true)}
      >
        <Box size={13} /> 3D
      </button>
    </div>
  </div>

  {#if skinsDisabledOffline}
    <div class="ui-pane ui-pane-soft p-5 jm-reveal">
      <p class="ui-section-title mb-2">Офлайн-аккаунт</p>
      <p class="text-sm text-[var(--text-secondary)] leading-relaxed">
        Скин в игре для пиратского режима не синхронизируется с Mojang или Ely.by: клиент использует стандартную
        модель. Редактор и пресеты скинов здесь отключены.
      </p>
    </div>
  {:else}
  <div class="flex flex-col lg:flex-row lg:items-start gap-5 lg:gap-6 w-full flex-1 min-h-0 overflow-y-auto custom-scrollbar pr-1">
    <!-- Preview -->
    <div
      class="ui-pane ui-pane-soft w-full max-w-[460px] mx-auto lg:mx-0 lg:max-w-none lg:w-[420px] xl:w-[460px] shrink-0 flex flex-col relative overflow-hidden self-start jm-reveal"
      style="animation-delay: 0.05s"
    >
      <div
        class="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(134,168,134,0.18)_0%,transparent_65%)] animate-pulse-slow"
      ></div>
      <div class="flex shrink-0 items-center justify-center px-4 py-6 min-h-[260px]">
        {#if previewSkin}
          {#key (previewSkin && previewSkin.id ? previewSkin.id : "") +
            use3D +
            (previewSkin && previewSkin.skin_data ? previewSkin.skin_data : "") +
            editModel +
            skinTextureUrl +
            model3d}
            <div
              in:scale={{ duration: 280, start: 0.92, easing: quintOut }}
              class="flex flex-col items-center relative z-10 w-full"
            >
              {#if use3D}
                <canvas
                  bind:this={canvasEl}
                  class="relative z-10 mx-auto block max-w-full shrink-0 drop-shadow-2xl"
                  style="max-width:min(260px,100%);image-rendering:pixelated;"
                />
              {:else}
                <div class="flex flex-col items-center relative z-10">
                  {#if previewSkin.skin_type === "local"}
                    <SkinBody2D
                      src={previewSkin.skin_data}
                      size={200}
                      model={previewSkin.model === "slim" ? "slim" : "default"}
                    />
                  {:else}
                    <SkinBody2D
                      src={skinTextureUrl || remoteSkinPngUrl(nickForPreview, activeAccount)}
                      size={200}
                      model={model3d === "slim" ? "slim" : "default"}
                    />
                  {/if}
                  {#if previewSkin.cape_url}
                    <div class="flex flex-col items-center mt-2">
                      <span class="text-[10px] text-[var(--text-secondary)] mb-0.5">Плащ</span>
                      <img
                        src={previewSkin.cape_url}
                        class="w-[40px] h-auto"
                        style:image-rendering="pixelated"
                        alt="Cape"
                      />
                    </div>
                  {/if}
                </div>
              {/if}
              <div
                class="mt-3 text-center relative z-10 bg-black/40 px-4 py-1.5 rounded-xl border border-white/5 backdrop-blur-sm"
              >
                <div class="text-base font-bold text-white tracking-wide">
                  {previewSkin.name || previewSkin.skin_data || previewSkin.username}
                </div>
              </div>
            </div>
          {/key}
        {:else}
          <div class="text-[var(--text-secondary)] z-10">Скин не выбран</div>
        {/if}
      </div>
    </div>

    <!-- Right -->
    <div
      class="ui-pane flex-grow flex flex-col min-h-[360px] min-w-0 overflow-hidden jm-reveal p-4 md:p-5"
      style="animation-delay: 0.1s"
    >
      {#if !editingPreset}
        <div class="flex justify-between items-center mb-3 shrink-0">
          <h3 class="ui-section-title">Ваши пресеты</h3>
          <span class="ui-hint">{profiles.skin_presets.length} сохранено</span>
        </div>
        <div class="relative flex items-center min-h-0 max-h-[240px]">
          <button
            type="button"
            on:click={() => scrollDir("left")}
            class="absolute -left-3 z-20 w-8 h-8 rounded-full bg-[var(--accent)] text-[var(--accent-contrast,#000)] shadow-lg transition-all duration-200 hover:scale-110 flex items-center justify-center {canScrollLeft
              ? 'opacity-100'
              : 'opacity-0 scale-50 pointer-events-none'}"
            aria-label="Прокрутить влево"
          >
            <ChevronLeft size={18} />
          </button>
          <div
            bind:this={scrollEl}
            on:scroll={checkScroll}
            class="flex gap-3 overflow-x-auto snap-x snap-mandatory py-2 px-1 w-full items-center [&::-webkit-scrollbar]:hidden"
          >
            <button
              type="button"
              on:click={() => openEditor()}
              class="snap-center shrink-0 w-[140px] min-h-[168px] rounded-[var(--radius)] border-2 border-dashed border-[var(--accent)]/45 bg-[var(--accent-softer)] flex flex-col items-center justify-center py-3 text-[var(--accent)] cursor-pointer hover:bg-[var(--accent-soft)] hover:border-[var(--accent)] transition-all duration-200 group"
            >
              <Plus size={28} class="mb-1 group-hover:scale-110 transition-transform duration-200" />
              <span class="font-semibold text-xs text-center px-1">Создать</span>
            </button>
            {#each profiles.skin_presets as preset (preset.id)}
              {@const applied = activeAccount && activeAccount.active_skin_id === preset.id}
              <button
                type="button"
                on:click={() => openEditor(preset)}
                on:mouseenter={() => (previewSkin = preset)}
                on:mouseleave={restorePreviewFromApplied}
                class="snap-center shrink-0 w-[140px] min-h-[168px] rounded-[var(--radius)] p-2.5 flex flex-col items-center justify-between cursor-pointer transition-all duration-200 relative hover:-translate-y-0.5 {applied
                  ? 'border-2 border-[var(--accent)] bg-[var(--accent-softer)] shadow-[0_4px_16px_var(--accent-soft)]'
                  : 'border border-[var(--border)] bg-[var(--surface-1)] hover:border-[var(--accent)]/40 hover:bg-[var(--surface-hover)]'}"
              >
                {#if preset.skin_type === "local"}
                  <SkinBody2D
                    src={preset.skin_data}
                    size={80}
                    model={preset.model === "slim" ? "slim" : "default"}
                  />
                {:else}
                  {@const presetNick =
                    String(preset.skin_data || preset.username || "Steve").trim() || "Steve"}
                  <SkinBody2D
                    src={presetRemoteTextureUrls[preset.id] || remoteSkinPngUrl(presetNick, activeAccount)}
                    size={80}
                    model={preset.model === "slim" ? "slim" : "default"}
                  />
                {/if}
                <span class="font-semibold text-xs text-[var(--text)] truncate w-full text-center mt-auto">{preset.name}</span>
                {#if applied}
                  <div
                    class="absolute -top-1.5 -right-1.5 bg-[var(--accent)] text-[var(--accent-contrast,#000)] p-0.5 rounded-full shadow-md animate-pop"
                    title="Надето"
                  >
                    <Check size={12} />
                  </div>
                {/if}
              </button>
            {/each}
          </div>
          <button
            type="button"
            on:click={() => scrollDir("right")}
            class="absolute -right-3 z-20 w-8 h-8 rounded-full bg-[var(--accent)] text-[var(--accent-contrast,#000)] shadow-lg transition-all duration-200 hover:scale-110 flex items-center justify-center {canScrollRight
              ? 'opacity-100'
              : 'opacity-0 scale-50 pointer-events-none'}"
            aria-label="Прокрутить вправо"
          >
            <ChevronRight size={18} />
          </button>
        </div>
      {:else}
        <div
          in:fly={{ x: 28, duration: 320, easing: quintOut }}
          class="flex flex-col h-full overflow-y-auto custom-scrollbar"
        >
          <div class="flex justify-between items-center mb-4 shrink-0">
            <h3 class="ui-section-title">
              {editingPreset.id === "new" ? "Добавить скин" : "Редактирование скина"}
            </h3>
            <button
              type="button"
              on:click={closeEditor}
              class="ui-btn ui-btn-ghost ui-btn-icon"
              aria-label="Закрыть"
            >
              <X size={18} />
            </button>
          </div>

          <div class="space-y-3 flex-grow min-h-0">
            <div>
              <label class="text-xs text-[var(--text-secondary)] mb-1 block font-medium" for="jm-skin-name"
                >Название пресета</label
              >
              <input
                id="jm-skin-name"
                type="text"
                bind:value={editName}
                class="ui-input"
              />
            </div>

            {#if editingPreset.id === "new"}
              <div class="rounded-[var(--radius)] bg-[var(--surface-1)] border border-[var(--border)] p-3">
                <h4 class="text-[var(--accent)] font-semibold text-sm mb-2 flex items-center gap-2">
                  <Upload size={14} /> Загрузить из файла
                </h4>
                <div
                  class="w-full rounded-[var(--radius)] border-2 border-dashed transition-all duration-200 ease-out {skinDropHover
                    ? 'border-[var(--accent)] bg-[var(--accent-soft)] scale-[1.01]'
                    : 'border-[var(--accent)]/45 bg-[var(--surface-1)]'}"
                  role="presentation"
                  on:dragover|preventDefault={(e) => {
                    const dt = e.dataTransfer;
                    if (dt) dt.dropEffect = "copy";
                    skinDropHover = true;
                  }}
                  on:dragleave={() => (skinDropHover = false)}
                  on:drop={onSkinFileDrop}
                >
                  <label
                    class="w-full hover:bg-[var(--accent-soft)] rounded-[var(--radius)] px-3 py-4 text-[var(--accent)] flex flex-col items-center justify-center cursor-pointer transition-colors relative text-sm"
                  >
                    {#if pngImportReading}
                      <Loader2 size={20} class="animate-spin mb-1" />
                      <span>Чтение файла...</span>
                    {:else}
                      <Upload size={20} class="mb-1" />
                      <span>Нажмите или перетащите .png</span>
                      <input
                        type="file"
                        accept="image/png"
                        class="hidden"
                        on:change={handleFileUpload}
                        disabled={pngImportReading || !!pendingPngImport}
                      />
                    {/if}
                  </label>
                </div>
              </div>
            {/if}

            {#if editingPreset.id === "new" || editingPreset.skin_type === "nickname" || !editingPreset.skin_type}
              <div class="rounded-[var(--radius)] bg-[var(--surface-1)] border border-[var(--border)] p-3">
                <h4 class="text-[var(--accent)] font-semibold text-sm mb-2">Скин по никнейму</h4>
                <input
                  type="text"
                  value={editUsername}
                  on:input={(e) => updatePreviewNickname(e.currentTarget.value)}
                  placeholder="Никнейм (например: Notch)"
                  class="ui-input"
                />
              </div>
            {/if}

            <div class="rounded-[var(--radius)] bg-[var(--surface-1)] border border-[var(--border)] p-3">
              <h4 class="text-[var(--accent)] font-semibold text-sm mb-2 flex items-center gap-2">
                <Shirt size={14} /> Модель рук
              </h4>
              <div class="ui-seg w-full">
                <button
                  type="button"
                  class="ui-seg-item flex-1"
                  class:is-active={editModel === "default"}
                  on:click={() => setModel("default")}
                >
                  Стив (широкие)
                </button>
                <button
                  type="button"
                  class="ui-seg-item flex-1"
                  class:is-active={editModel === "slim"}
                  on:click={() => setModel("slim")}
                >
                  Алекс (тонкие)
                </button>
              </div>
            </div>

            <div class="rounded-[var(--radius)] bg-[var(--surface-1)] border border-[var(--border)] p-3">
              <h4 class="text-[var(--accent)] font-semibold text-sm mb-2">Плащ</h4>
              <div class="ui-seg mb-2 w-full">
                <button
                  type="button"
                  class="ui-seg-item flex-1"
                  class:is-active={editCapeType === "none"}
                  on:click={setCapeNone}
                >
                  Без плаща
                </button>
                <button
                  type="button"
                  class="ui-seg-item flex-1"
                  class:is-active={editCapeType === "url"}
                  on:click={setCapeUrlType}
                >
                  URL
                </button>
                <button
                  type="button"
                  class="ui-seg-item flex-1"
                  class:is-active={editCapeType === "local"}
                  on:click={setCapeLocalType}
                >
                  Файл
                </button>
              </div>
              {#if editCapeType === "url"}
                <input
                  type="text"
                  value={editCapeUrl}
                  on:input={(e) => updateCapeUrlInput(e.currentTarget.value)}
                  placeholder="URL плаща"
                  class="ui-input"
                />
              {/if}
              {#if editCapeType === "local"}
                <label
                  class="w-full bg-[var(--surface-1)] border border-dashed border-[var(--accent)]/45 hover:bg-[var(--accent-soft)] rounded-[var(--radius)] px-3 py-3 text-[var(--accent)] flex flex-col items-center justify-center cursor-pointer transition-colors text-sm"
                >
                  {#if editCapeUrl}
                    <div class="flex flex-col items-center">
                      <img
                        src={editCapeUrl}
                        class="w-[40px] h-auto mb-1"
                        style:image-rendering="pixelated"
                        alt="Cape preview"
                      />
                      <span class="text-xs text-[var(--text-secondary)]">Нажмите, чтобы заменить</span>
                    </div>
                  {:else}
                    <Upload size={16} class="mb-1" />
                    <span>Выбрать .png файл плаща</span>
                  {/if}
                  <input type="file" accept="image/png" class="hidden" on:change={handleCapeFileUpload} />
                </label>
              {/if}
            </div>
          </div>

          <div class="flex flex-col gap-2 mt-4 shrink-0">
            <button
              type="button"
              on:click={() => void handleSaveNickname()}
              class="ui-btn ui-btn-primary w-full"
            >
              <Save size={16} /> Сохранить изменения
            </button>
            {#if editingPreset.id !== "new"}
              <div class="flex gap-2">
                <button
                  type="button"
                  disabled={applyingPreset}
                  on:click={() => void handleApplySkin(editingPreset.id)}
                  class="ui-btn ui-btn-subtle flex-1"
                >
                  {#if applyingPreset}
                    <Loader2 size={16} class="animate-spin" /> …
                  {:else}
                    <Check size={16} /> Надеть
                  {/if}
                </button>
                <button
                  type="button"
                  on:click={() => void handleDelete(editingPreset.id)}
                  class="ui-btn ui-btn-danger flex-1"
                >
                  <Trash2 size={16} /> Удалить
                </button>
              </div>
            {/if}
          </div>
        </div>
      {/if}
    </div>
  </div>
  {/if}

  {#if pendingPngImport}
    <div
      class="fixed inset-0 z-[20060] flex items-center justify-center p-4 bg-black/65 backdrop-blur-md"
      transition:fade={{ duration: 200 }}
      role="presentation"
      on:click|self={cancelPngImport}
    >
      <div
        class="w-full max-w-md ui-card p-6 space-y-4"
        in:scale={{ duration: 340, start: 0.92, easing: quintOut }}
        role="dialog"
        aria-modal="true"
        aria-labelledby="jm-png-import-title"
        on:click|stopPropagation
      >
        <h3 id="jm-png-import-title" class="ui-section-title">Импорт скина</h3>
        <p class="ui-hint">Введите название пресета (не обязательно совпадать с именем файла).</p>
        <div>
          <label class="text-xs text-[var(--text-secondary)] mb-1 block font-medium" for="jm-png-import-name"
            >Название</label
          >
          <input
            id="jm-png-import-name"
            type="text"
            bind:value={pngImportName}
            class="ui-input"
            placeholder={pendingPngImport.suggestedName}
          />
        </div>
        <div class="flex gap-2 pt-1">
          <button
            type="button"
            disabled={pngImportSaving}
            on:click={cancelPngImport}
            class="ui-btn ui-btn-subtle flex-1"
          >
            Отмена
          </button>
          <button
            type="button"
            disabled={pngImportSaving}
            on:click={() => void confirmPngImport()}
            class="ui-btn ui-btn-primary flex-1"
          >
            {#if pngImportSaving}
              <Loader2 size={16} class="animate-spin" />
            {/if}
            Добавить
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>
