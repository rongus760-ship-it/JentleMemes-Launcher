<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { fly, fade, scale } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import {
    MessageCircle,
    LogIn,
    User,
    Users,
    Settings2,
    Send,
    Mic,
    Square,
    ImagePlus,
    PlusCircle,
    Check,
    Upload,
    RefreshCw,
    X,
    KeyRound,
    ChevronDown,
    Loader2,
    Server,
    Phone,
    PhoneOff,
    MoreVertical,
    MicOff,
    Volume2,
    Copy,
    Pencil,
    Globe,
    ExternalLink,
    AtSign,
    CornerUpLeft,
    Share2,
  } from "lucide-svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { showToast } from "../lib/jmEvents";
  import {
    ensureNotificationPermission,
    showDesktopNotificationIfBackground,
  } from "../lib/desktopNotification";
  import { runDmVoiceCall } from "../lib/webrtcDmCall";
  import { resolveSiteMediaUrl } from "../lib/resolveSiteMediaUrl";
  import ServerMotdBlock from "../components/ServerMotdBlock.svelte";

  export let apiBaseUrl: string = "https://jentlememes.ru";
  /** Расширенные: показывать Minecraft-сервер в карточке профиля */
  export let chatProfileMcServer = false;
  /** Вкладка «Чат» активна в шапке лаунчера (для фоновых уведомлений) */
  export let chatChromeVisible = true;
  /** После импорта .jentlepack — открыть сборку и запланировать вход на сервер */
  export let onNavigateLibraryWithServer: ((instanceId: string, serverIp: string) => void) | undefined =
    undefined;
  export let onOpenLibrary: (() => void) | undefined = undefined;

  const TOKEN_KEY = "jm_social_access_token";

  type Tab = "chats" | "site" | "friends" | "profile";

  const TAB_BAR: { id: Tab; icon: any; label: string }[] = [
    { id: "chats", icon: MessageCircle, label: "Чаты" },
    { id: "site", icon: Globe, label: "Сайт" },
    { id: "friends", icon: Users, label: "Друзья" },
    { id: "profile", icon: Settings2, label: "Профиль" },
  ];

  let token = "";
  let username = "";
  let password = "";
  let busy = false;
  let me: any = null;
  let err = "";
  let tab: Tab = "chats";

  let editMc = "";
  let editBannerUrl = "";
  let editBio = "";
  let editMcHost = "";
  let saveBusy = false;

  const CHAT_SCROLL_BOTTOM_PX = 80;
  let messagesScrollEl: HTMLDivElement | null = null;

  let friends: any[] = [];
  let pendingFrom: any[] = [];
  let pendingTo: any[] = [];
  let friendHandle = "";
  let friendsBusy = false;

  let conversations: any[] = [];
  let activeCid: string | null = null;
  let messages: any[] = [];
  let msgText = "";
  let msgBusy = false;
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  let newGroupTitle = "";
  let newGroupHandles = "";

  let audioInputs: MediaDeviceInfo[] = [];
  let audioOutputs: MediaDeviceInfo[] = [];
  let selectedMic = "";
  let selectedSpeaker = "";
  let micMenuOpen = false;
  let micMenuEl: HTMLDivElement | null = null;
  let mediaRecorder: MediaRecorder | null = null;
  let recording = false;
  let recordChunks: BlobPart[] = [];

  let patJustCreated = "";
  let inviteHandleInput = "";

  let myInstances: any[] = [];
  let serverInviteOpen = false;
  let invServerIp = "";
  let invTitle = "";
  let invMode: "instance" | "version_only" = "instance";
  let invInstanceId = "";
  let invGameVersion = "";
  let invFolders: Record<string, boolean> = {};
  let invExportBusy = false;
  let invShowAdvanced = false;
  let acceptBusy = false;
  let callBusy = false;

  /** Активный P2P-звонок: снять трубку / завершить */
  let voiceCallHangup: (() => Promise<void>) | null = null;
  let remoteVoiceStream: MediaStream | null = null;
  let voiceRemoteEl: HTMLAudioElement | null = null;
  let incomingCallPrompt: { cid: string; messageId: string; sessionId: string } | null =
    null;
  let dismissedCallMids: string[] = [];
  /** Контекстное меню сообщения (ПКМ или ⋮): координаты в viewport */
  let msgMenu: { messageId: string; x: number; y: number } | null = null;
  let editingMid: string | null = null;
  let editDraft = "";
  let voiceCallSetMicMuted: ((muted: boolean) => void) | null = null;
  let voiceCallSetAudioOut: ((deviceId: string) => Promise<void>) | null = null;
  let voiceMicMuted = false;
  let voiceBarMicMenuOpen = false;
  let voiceBarSpeakerMenuOpen = false;
  let voiceLocalLevel = 0;
  let voiceRemoteLevel = 0;
  let voiceRttMs: number | null = null;
  let voiceConnState = "";
  let voiceIceState = "";
  let replyTo: { id: string; preview: string; label: string } | null = null;
  let forwardFrom: { label: string; text: string; messageId: string } | null = null;
  let profileModal: { userId: string; isSelf: boolean } | null = null;
  let profileModalData: any = null;
  let profileModalBusy = false;
  let mcProbe: { loading: boolean; err?: string; json?: any } = { loading: false };
  let friendSearchQuery = "";

  /** Лайтбокс для фото/видео */
  let mediaLightbox: { kind: "image" | "video"; url: string } | null = null;
  function openMediaLightbox(kind: "image" | "video", url: string) {
    if (!url) return;
    mediaLightbox = { kind, url };
  }
  function closeMediaLightbox() {
    mediaLightbox = null;
  }

  /** Длительность активного звонка в секундах */
  let voiceCallStartedAt: number | null = null;
  let voiceCallElapsed = 0;
  let voiceCallElapsedTimer: ReturnType<typeof setInterval> | null = null;

  function startVoiceCallTicker() {
    if (voiceCallElapsedTimer) clearInterval(voiceCallElapsedTimer);
    voiceCallStartedAt = Date.now();
    voiceCallElapsed = 0;
    voiceCallElapsedTimer = setInterval(() => {
      if (voiceCallStartedAt == null) return;
      voiceCallElapsed = Math.floor((Date.now() - voiceCallStartedAt) / 1000);
    }, 1000);
  }

  function stopVoiceCallTicker() {
    if (voiceCallElapsedTimer) {
      clearInterval(voiceCallElapsedTimer);
      voiceCallElapsedTimer = null;
    }
    voiceCallStartedAt = null;
    voiceCallElapsed = 0;
  }

  function formatCallElapsed(sec: number): string {
    const s = Math.max(0, Math.floor(sec));
    const mm = Math.floor(s / 60);
    const ss = s % 60;
    const h = Math.floor(mm / 60);
    const mmRest = mm % 60;
    const pad = (x: number) => String(x).padStart(2, "0");
    return h > 0 ? `${pad(h)}:${pad(mmRest)}:${pad(ss)}` : `${pad(mm)}:${pad(ss)}`;
  }

  /** Человекочитаемая метка состояния звонка */
  $: callStateLabel = (() => {
    if (!voiceCallHangup) return "";
    const c = voiceConnState || "";
    const ice = voiceIceState || "";
    if (c === "failed" || ice === "failed") return "Ошибка соединения";
    if (ice === "disconnected" && c === "connected") return "Потеря связи…";
    if (c === "connected" || ice === "connected" || ice === "completed") return "В разговоре";
    if (c === "connecting" || ice === "checking" || ice === "new") return "Соединение…";
    return c || "Ожидание…";
  })();

  /** Кешированные длительности аудио-сообщений, ключ = url */
  let audioDurations: Record<string, number> = {};
  function registerAudioDuration(url: string, dur: number) {
    if (!url || !isFinite(dur) || dur <= 0) return;
    if (audioDurations[url]) return;
    audioDurations = { ...audioDurations, [url]: dur };
  }
  function handleAudioMeta(ev: Event, url: string | undefined) {
    const a = ev.currentTarget as HTMLAudioElement | null;
    if (!a) return;
    registerAudioDuration(url ?? "", a.duration);
  }
  function formatAudioDuration(sec: number): string {
    if (!isFinite(sec) || sec < 0) return "—";
    const s = Math.floor(sec);
    const mm = Math.floor(s / 60);
    const ss = s % 60;
    return `${mm}:${String(ss).padStart(2, "0")}`;
  }

  $: friendSearchLower = friendSearchQuery.trim().toLowerCase();
  $: friendsFiltered =
    !friendSearchLower || !friends.length
      ? friends
      : friends.filter((f) => {
          const blob = [
            f.handle,
            f.display_name,
            f.chat_primary_line,
            f.chat_secondary_line,
            f.public_id,
            f.user_id,
          ]
            .map((x) => String(x ?? "").toLowerCase())
            .join(" ");
          return blob.includes(friendSearchLower);
        });

  async function applyVoiceOutputToCall() {
    if (!selectedSpeaker || !voiceCallHangup) return;
    try {
      await voiceCallSetAudioOut?.(selectedSpeaker);
    } catch {
      /* ignore */
    }
    try {
      const el = voiceRemoteEl as HTMLAudioElement & { setSinkId?: (id: string) => Promise<void> };
      if (el && typeof el.setSinkId === "function" && remoteVoiceStream) {
        await el.setSinkId(selectedSpeaker);
      }
    } catch {
      /* ignore */
    }
  }

  $: if (selectedSpeaker && voiceCallHangup) {
    void applyVoiceOutputToCall();
  }

  $: if (voiceRemoteEl && remoteVoiceStream) {
    voiceRemoteEl.srcObject = remoteVoiceStream;
    voiceRemoteEl.volume = 1;
    void voiceRemoteEl.play().catch(() => {});
    const el = voiceRemoteEl as HTMLAudioElement & { setSinkId?: (id: string) => Promise<void> };
    if (selectedSpeaker && typeof el.setSinkId === "function") {
      void el.setSinkId(selectedSpeaker).catch(() => {});
    }
  }
  $: if (voiceRemoteEl && !remoteVoiceStream) voiceRemoteEl.srcObject = null;

  const notifyPrevIds: Record<string, Set<string>> = {};
  const notifySeenOnce: Record<string, boolean> = {};

  const exportFolderLabels: Record<string, string> = {
    mods: "Моды",
    config: "Конфигурация",
    resourcepacks: "Ресурспаки",
    shaderpacks: "Шейдеры",
    saves: "Миры",
    scripts: "Скрипты",
    logs: "Логи",
    crash_reports: "Краш-репорты",
    options: "Настройки игры",
    screenshots: "Скриншоты",
    schematics: "Схематики",
  };
  const exportCommonFolders = ["mods", "config", "resourcepacks", "shaderpacks", "saves"];
  $: invAdvancedFolders = Object.keys(invFolders).filter((k) => !exportCommonFolders.includes(k));
  $: invSelectedFolders = Object.entries(invFolders)
    .filter(([, v]) => v)
    .map(([k]) => k);

  $: {
    const menu = msgMenu;
    if (menu && !messages.some((x) => x.id === menu.messageId)) msgMenu = null;
  }

  $: directConversations = conversations.filter((c) => !Number(c.is_group));
  $: groupConversations = conversations.filter((c) => !!Number(c.is_group));
  /** Сортировка личных чатов по имени собеседника — слева только люди, без секции «ЛС». */
  /** Ник без @: из поля handle или из подписи вида @user */
  function peerNickAfterAt(p: any): string {
    if (!p) return "";
    const fromHandle = String(p.handle ?? "")
      .replace(/^@+/g, "")
      .trim();
    if (fromHandle) return fromHandle;
    const sec = String(p.chat_secondary_line ?? "").trim();
    if (!sec) return "";
    const m = sec.match(/@([^\s@]+)/);
    if (m?.[1]) return m[1].trim();
    return sec.replace(/^@+/g, "").trim();
  }

  $: directSorted = [...directConversations].sort((a, b) => {
    const ka = String(
      a.direct_peer?.chat_primary_line ||
        a.direct_peer?.display_name ||
        a.direct_peer?.handle ||
        peerNickAfterAt(a.direct_peer) ||
        "",
    ).toLowerCase();
    const kb = String(
      b.direct_peer?.chat_primary_line ||
        b.direct_peer?.display_name ||
        b.direct_peer?.handle ||
        peerNickAfterAt(b.direct_peer) ||
        "",
    ).toLowerCase();
    return ka.localeCompare(kb, "ru");
  });

  function directRowTitle(c: any): string {
    const p = c?.direct_peer;
    return String(
      p?.chat_primary_line || p?.display_name || (p?.handle ? `@${String(p.handle).replace(/^@+/, "")}` : "") || "",
    ).trim() || peerNickAfterAt(p);
  }

  function directRowSubtitle(c: any): string {
    const line = c?.direct_peer?.chat_secondary_line;
    return line ? String(line).trim() : "";
  }

  /** Подпись над пузырём в ЛС: сначала @handle, иначе вторичная строка, иначе ник MC */
  function senderDmLabel(s: any): string {
    if (!s) return "—";
    const h = String(s.handle ?? "")
      .replace(/^@+/g, "")
      .trim();
    if (h) return `@${h}`;
    const sec = String(s.chat_secondary_line ?? "").trim();
    if (sec) return sec.startsWith("@") ? sec : `@${sec.replace(/^@+/g, "")}`;
    return String(s.chat_primary_line ?? "").trim() || "—";
  }

  function textSnippet(s: string, max = 140): string {
    const t = String(s || "")
      .replace(/\s+/g, " ")
      .trim();
    if (t.length <= max) return t;
    return t.slice(0, max - 1) + "…";
  }

  /** last_active_at — unix секунды с сервера */
  function formatPresence(lastAt: number | null | undefined): string {
    if (lastAt == null || Number.isNaN(Number(lastAt))) return "нет данных";
    const ago = Date.now() / 1000 - Number(lastAt);
    if (ago < 120) return "в сети";
    if (ago < 3600) return `был(а) ${Math.floor(ago / 60)} мин назад`;
    if (ago < 86400) return `был(а) ${Math.floor(ago / 3600)} ч назад`;
    if (ago < 604800) return `был(а) ${Math.floor(ago / 86400)} дн. назад`;
    return "давно не в сети";
  }

  async function openProfileById(uid: string, isSelf: boolean) {
    profileModal = { userId: uid, isSelf };
    profileModalData = null;
    profileModalBusy = true;
    mcProbe = { loading: false };
    try {
      const r = await api(`/api/v1/users/${encodeURIComponent(uid)}/profile`);
      const j = await r.json().catch(() => ({}));
      if (r.ok) {
        profileModalData = j;
      } else {
        showToast((j as { error?: string }).error || "Профиль недоступен");
        profileModal = null;
      }
    } finally {
      profileModalBusy = false;
    }
    const host = String(profileModalData?.minecraft_server_host || "").trim();
    if (chatProfileMcServer && host) void loadMcProbe(host);
  }

  function closeProfileModal() {
    profileModal = null;
    profileModalData = null;
    mcProbe = { loading: false };
  }

  async function loadMcProbe(host: string) {
    mcProbe = { loading: true, err: undefined, json: undefined };
    try {
      const j = (await invoke("ping_server", { ip: host.trim() })) as Record<string, unknown>;
      mcProbe = { loading: false, json: j };
    } catch {
      mcProbe = { loading: false, err: "Не удалось запросить сервер" };
    }
  }

  function beginReply(m: any) {
    closeMessageMenu();
    forwardFrom = null;
    replyTo = {
      id: m.id,
      preview: textSnippet(String(m.content?.text || ""), 160),
      label: senderDmLabel(m.sender),
    };
  }

  function clearReply() {
    replyTo = null;
  }

  function beginForward(m: any) {
    closeMessageMenu();
    replyTo = null;
    if (m.kind !== "text" || !String(m.content?.text ?? "").trim()) {
      showToast("Пересылать можно только текст");
      return;
    }
    forwardFrom = {
      messageId: m.id,
      label: senderDmLabel(m.sender),
      text: String(m.content?.text ?? ""),
    };
  }

  function clearForward() {
    forwardFrom = null;
  }

  function insertPingFromMessage(m: any) {
    closeMessageMenu();
    const h = String(m.sender?.handle ?? "")
      .replace(/^@+/g, "")
      .trim();
    if (!h) {
      showToast("Нет handle для упоминания");
      return;
    }
    const at = `@${h} `;
    if (!msgText.includes(`@${h}`)) msgText = `${msgText ? `${msgText} ` : ""}${at}`;
  }

  /** Если API не отдал direct_peer, подставляем превью из сообщений собеседника */
  function enrichConversationPeerFromMessages(cid: string, msgs: any[]) {
    const ix = conversations.findIndex((c) => c.id === cid);
    if (ix < 0) return;
    const c = conversations[ix];
    if (Number(c.is_group)) return;
    const myId = me?.id != null ? String(me.id) : "";
    const myLower = myId.toLowerCase();
    const otherMsg = myId
      ? msgs.find(
          (m) => m.sender_id && String(m.sender_id).toLowerCase() !== myLower,
        )
      : msgs.find((m) => m.sender_id);
    const s = otherMsg?.sender;
    if (!s?.user_id) return;
    const p = c.direct_peer || {};
    const merged = {
      user_id: p.user_id || s.user_id,
      handle: p.handle ?? s.handle ?? null,
      display_name: p.display_name ?? null,
      avatar_url: p.avatar_url || s.avatar_url || null,
      chat_primary_line: p.chat_primary_line || s.chat_primary_line || null,
      chat_secondary_line: p.chat_secondary_line || s.chat_secondary_line || null,
      last_active_at: p.last_active_at ?? s.last_active_at ?? null,
      public_id: p.public_id ?? s.public_id ?? null,
    };
    if (JSON.stringify(merged) === JSON.stringify(c.direct_peer)) return;
    const next = [...conversations];
    next[ix] = { ...c, direct_peer: merged };
    conversations = next;
  }

  /** Пустая строка в списке чатов — добираем peer из истории без открытия диалога */
  function dmNeedsPeerTitleHint(c: any): boolean {
    if (Number(c.is_group)) return false;
    return !String(directRowTitle(c) || "").trim();
  }

  async function prefetchDmPeersForList(list: any[]) {
    if (!token || !me?.id) return;
    const targets = list.filter(dmNeedsPeerTitleHint);
    if (!targets.length) return;
    await Promise.all(
      targets.map(async (c) => {
        try {
          const r = await api(
            `/api/v1/conversations/${encodeURIComponent(c.id)}/messages?limit=100`,
          );
          if (!r.ok) return;
          const j = await r.json();
          enrichConversationPeerFromMessages(c.id, j.messages || []);
        } catch {
          /* ignore */
        }
      }),
    );
  }

  $: activeConversation = conversations.find((c) => c.id === activeCid) ?? null;
  $: isDirectDm = activeConversation != null && !Number(activeConversation.is_group);

  $: if (activeCid) {
    try {
      localStorage.setItem("jm_overlay_chat_active_cid", activeCid);
    } catch {
      /* ignore */
    }
  }

  /** Редактор аватарки */
  let avatarEditorOpen = false;
  let avatarDraftUrl: string | null = null;
  let avatarImgEl: HTMLImageElement | null = null;
  let editorCv: HTMLCanvasElement | null = null;
  let previewCv: HTMLCanvasElement | null = null;
  let editorZoom = 1.15;
  let editorPanX = 0;
  let editorPanY = 0;
  let editorDragging = false;
  let dragRef = { x: 0, y: 0, px: 0, py: 0 };
  let avatarBusy = false;

  function base() {
    return String(apiBaseUrl || "https://jentlememes.ru").replace(/\/$/, "");
  }

  function avatarSrc(url: string | null | undefined): string | null {
    return resolveSiteMediaUrl(apiBaseUrl, url);
  }

  /** Корень сайта для вкладки «Сайт» (без /api/…) */
  function siteWebUrl(): string {
    try {
      const u = new URL(base().startsWith("http") ? base() : `https://${base()}`);
      return `${u.origin}/`;
    } catch {
      return "https://jentlememes.ru/";
    }
  }

  /** WebKit часто кидает TypeError: Load failed при недоступном хосте — показываем понятный текст */
  function formatFetchError(e: unknown): string {
    const raw = e instanceof Error ? e.message : typeof e === "string" ? e : String(e);
    const low = raw.toLowerCase();
    if (
      e instanceof TypeError ||
      low.includes("load failed") ||
      low.includes("failed to fetch") ||
      low.includes("networkerror") ||
      low.includes("network request failed") ||
      low.includes("fetch is aborted")
    ) {
      return `Не удалось связаться с сервером (${base()}). Проверьте адрес социального API в настройках и подключение к сети.`;
    }
    return raw || "Неизвестная ошибка";
  }

  async function api(path: string, init: RequestInit = {}) {
    const h = new Headers(init.headers);
    if (token) h.set("Authorization", `Bearer ${token}`);
    return fetch(`${base()}${path}`, { ...init, headers: h });
  }

  function getCropParams():
    | { sx: number; sy: number; side: number; iw: number; ih: number }
    | null {
    if (!avatarImgEl?.naturalWidth) return null;
    const iw = avatarImgEl.naturalWidth;
    const ih = avatarImgEl.naturalHeight;
    const baseSide = Math.min(iw, ih);
    const side = Math.max(24, baseSide / editorZoom);
    let cx = iw / 2 + editorPanX;
    let cy = ih / 2 + editorPanY;
    let sx = cx - side / 2;
    let sy = cy - side / 2;
    sx = Math.max(0, Math.min(iw - side, sx));
    sy = Math.max(0, Math.min(ih - side, sy));
    return { sx, sy, side, iw, ih };
  }

  function paintCanvas(c: HTMLCanvasElement | null, outSize: number, circular: boolean) {
    if (!c || !avatarImgEl) return;
    const p = getCropParams();
    if (!p) return;
    const ctx = c.getContext("2d");
    if (!ctx) return;
    c.width = outSize;
    c.height = outSize;
    ctx.clearRect(0, 0, outSize, outSize);
    if (circular) {
      ctx.save();
      ctx.beginPath();
      ctx.arc(outSize / 2, outSize / 2, outSize / 2, 0, Math.PI * 2);
      ctx.clip();
    }
    ctx.drawImage(avatarImgEl, p.sx, p.sy, p.side, p.side, 0, 0, outSize, outSize);
    if (circular) ctx.restore();
  }

  function syncEditorCanvases() {
    paintCanvas(editorCv, 240, false);
    paintCanvas(previewCv, 112, true);
  }

  function openAvatarEditorFromFile(f: File) {
    if (avatarDraftUrl) URL.revokeObjectURL(avatarDraftUrl);
    avatarDraftUrl = URL.createObjectURL(f);
    editorZoom = 1.15;
    editorPanX = 0;
    editorPanY = 0;
    avatarEditorOpen = true;
    void tick().then(() => {
      if (avatarImgEl?.complete) syncEditorCanvases();
    });
  }

  function onAvatarImgLoad() {
    syncEditorCanvases();
  }

  function closeAvatarEditor() {
    avatarEditorOpen = false;
    if (avatarDraftUrl) URL.revokeObjectURL(avatarDraftUrl);
    avatarDraftUrl = null;
  }

  function onEditorPointerDown(e: PointerEvent) {
    if (!editorCv) return;
    editorDragging = true;
    dragRef = { x: e.clientX, y: e.clientY, px: editorPanX, py: editorPanY };
    editorCv.setPointerCapture(e.pointerId);
  }

  function onEditorPointerMove(e: PointerEvent) {
    if (!editorDragging || !editorCv || !avatarImgEl) return;
    const p = getCropParams();
    if (!p) return;
    const scale = p.side / editorCv.clientWidth;
    const dx = (e.clientX - dragRef.x) * scale;
    const dy = (e.clientY - dragRef.y) * scale;
    editorPanX = dragRef.px - dx;
    editorPanY = dragRef.py - dy;
    syncEditorCanvases();
  }

  function onEditorPointerUp(e: PointerEvent) {
    editorDragging = false;
    try {
      editorCv?.releasePointerCapture(e.pointerId);
    } catch {
      /* ignore */
    }
  }

  async function uploadAvatarBlob(blob: Blob): Promise<string | null> {
    const fd = new FormData();
    fd.append("file", new File([blob], "avatar.png", { type: "image/png" }));
    const r = await fetch(`${base()}/api/v1/media`, {
      method: "POST",
      headers: { Authorization: `Bearer ${token}` },
      body: fd,
    });
    const j = await r.json().catch(() => ({}));
    if (!r.ok) {
      err = j.error || "upload";
      return null;
    }
    return j.url || null;
  }

  async function applyAvatarCrop() {
    if (!avatarImgEl?.naturalWidth) return;
    avatarBusy = true;
    err = "";
    try {
      const c = document.createElement("canvas");
      const p = getCropParams();
      if (!p || !avatarImgEl) return;
      const out = 512;
      c.width = out;
      c.height = out;
      const ctx = c.getContext("2d");
      if (!ctx) return;
      ctx.drawImage(avatarImgEl, p.sx, p.sy, p.side, p.side, 0, 0, out, out);
      const blob: Blob | null = await new Promise((res) => c.toBlob((b) => res(b), "image/png"));
      if (!blob) return;
      const url = await uploadAvatarBlob(blob);
      if (!url) return;
      const r = await api("/api/v1/me", {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ avatar_url: url }),
      });
      const j = await r.json().catch(() => ({}));
      if (!r.ok) throw new Error(j.error || `HTTP ${r.status}`);
      me = j;
      closeAvatarEditor();
    } catch (e) {
      err = formatFetchError(e);
    } finally {
      avatarBusy = false;
    }
  }

  function onAvatarFilePick(e: Event) {
    const t = e.currentTarget as HTMLInputElement;
    const f = t.files?.[0];
    t.value = "";
    if (f && f.type.startsWith("image/")) openAvatarEditorFromFile(f);
  }

  async function loadMe() {
    err = "";
    me = null;
    if (!token) return;
    try {
      const r = await api("/api/v1/me");
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      me = await r.json();
      editMc = me.mc_nick || "";
      editBannerUrl = String(me.profile_banner_url || "");
      editBio = String(me.profile_bio || "");
      editMcHost = String(me.minecraft_server_host || "");
      if (conversations.length) void prefetchDmPeersForList(conversations);
    } catch (e) {
      err = formatFetchError(e);
      localStorage.removeItem(TOKEN_KEY);
      token = "";
    }
  }

  async function saveProfile() {
    if (!token) return;
    saveBusy = true;
    err = "";
    try {
      const body: Record<string, string | null> = {
        mc_nick: editMc.trim() || null,
        profile_bio: editBio.trim() || null,
        minecraft_server_host: editMcHost.trim() || null,
      };
      if (me?.profile_banner_allowed) {
        body.profile_banner_url = editBannerUrl.trim() || null;
      }
      const r = await api("/api/v1/me", {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      const j = await r.json().catch(() => ({}));
      if (!r.ok) throw new Error(j.error || `HTTP ${r.status}`);
      me = j;
    } catch (e) {
      err = formatFetchError(e);
    } finally {
      saveBusy = false;
    }
  }

  async function loadFriends() {
    if (!token) return;
    friendsBusy = true;
    try {
      const r = await api("/api/v1/friends");
      if (!r.ok) return;
      const j = await r.json();
      friends = j.friends || [];
      pendingFrom = j.pending_from || [];
      pendingTo = j.pending_to || [];
    } finally {
      friendsBusy = false;
    }
  }

  async function requestFriend() {
    if (!friendHandle.trim()) return;
    friendsBusy = true;
    err = "";
    try {
      const r = await api("/api/v1/friends/request-by-handle", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ handle: friendHandle.trim() }),
      });
      const j = await r.json().catch(() => ({}));
      if (!r.ok) throw new Error(j.error || `HTTP ${r.status}`);
      friendHandle = "";
      await loadFriends();
    } catch (e) {
      err = formatFetchError(e);
    } finally {
      friendsBusy = false;
    }
  }

  async function acceptFriend(uid: string) {
    friendsBusy = true;
    try {
      await api("/api/v1/friends/accept", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ from_user_id: uid }),
      });
      await loadFriends();
    } finally {
      friendsBusy = false;
    }
  }

  async function loadConversations() {
    if (!token) return;
    try {
      const r = await api("/api/v1/conversations");
      if (!r.ok) return;
      const j = await r.json();
      conversations = j.conversations || [];
      void prefetchDmPeersForList(conversations);
    } catch {
      /* ignore */
    }
  }

  async function openDm(userId: string) {
    const r = await api("/api/v1/conversations/direct", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ user_id: userId }),
    });
    const j = await r.json().catch(() => ({}));
    if (!r.ok) {
      err = j.error || "direct";
      return;
    }
    activeCid = j.id;
    tab = "chats";
    await loadConversations();
    await loadMessages(j.id, true);
    startPoll(j.id);
  }

  async function createGroup() {
    if (!newGroupTitle.trim()) return;
    const handles = newGroupHandles
      .split(/[\s,]+/)
      .map((s) => s.trim().replace(/^@+/, ""))
      .filter(Boolean);
    const member_ids: string[] = [];
    for (const h of handles) {
      const r = await api(`/api/v1/users/by-handle/${encodeURIComponent(h)}`);
      if (!r.ok) {
        err = `Не найден: ${h}`;
        return;
      }
      const u = await r.json();
      member_ids.push(u.user_id);
    }
    const r = await api("/api/v1/conversations", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        is_group: true,
        title: newGroupTitle.trim(),
        member_ids,
      }),
    });
    const j = await r.json().catch(() => ({}));
    if (!r.ok) {
      err = j.error || "group";
      return;
    }
    newGroupTitle = "";
    newGroupHandles = "";
    activeCid = j.id;
    await loadConversations();
    await loadMessages(j.id, true);
    startPoll(j.id);
  }

  function messageNotifyBody(m: any): string {
    if (m.kind === "text") return String(m.content?.text || "").slice(0, 160);
    if (m.kind === "deleted") return "Сообщение удалено";
    if (m.kind === "call_invite") return "Входящий звонок в чате";
    if (m.kind === "server_invite") return String(m.content?.title || m.content?.server_ip || "Сервер").slice(0, 120);
    if (m.kind === "voice") return "Голосовое сообщение";
    if (m.kind === "image" || m.kind === "gif") return "Изображение";
    if (m.kind === "video") return "Видео";
    return "Новое сообщение";
  }

  async function loadMessages(cid: string, jumpToBottom = false) {
    const r = await api(`/api/v1/conversations/${cid}/messages`);
    if (!r.ok) return;
    const j = await r.json();
    const incoming: any[] = j.messages || [];
    const prev = notifyPrevIds[cid] ?? new Set<string>();
    const inited = notifySeenOnce[cid] === true;
    if (inited && me?.id && token) {
      const myLower = String(me.id).toLowerCase();
      for (const m of incoming) {
        if (!m?.id || prev.has(m.id)) continue;
        if (String(m.sender_id ?? "").toLowerCase() === myLower) continue;
        const title =
          m.kind === "call_invite"
            ? "Звонок — JentleMemes"
            : m.kind === "server_invite"
              ? "Приглашение на сервер"
              : "Чат — JentleMemes";
        const who = senderDmLabel(m.sender) || m.sender?.chat_primary_line || "Собеседник";
        const body =
          m.kind === "call_invite" ? `${who} звонит` : `${who}: ${messageNotifyBody(m)}`;
        showDesktopNotificationIfBackground(title, body, {
          tag: `jm-msg-${cid}-${m.id}`,
          chatVisible: chatChromeVisible,
        });
        break;
      }
    }
    notifySeenOnce[cid] = true;
    notifyPrevIds[cid] = new Set(incoming.map((x) => x.id).filter(Boolean));
    const el = messagesScrollEl;
    const nearBottom =
      !jumpToBottom &&
      cid === activeCid &&
      el != null &&
      el.scrollHeight - el.scrollTop - el.clientHeight <= CHAT_SCROLL_BOTTOM_PX;
    messages = incoming;
    enrichConversationPeerFromMessages(cid, messages);
    scanIncomingCall(cid, incoming);
    await tick();
    if (!messagesScrollEl || activeCid !== cid) return;
    if (jumpToBottom) {
      messagesScrollEl.scrollTop = messagesScrollEl.scrollHeight;
    } else if (nearBottom) {
      messagesScrollEl.scrollTo({ top: messagesScrollEl.scrollHeight, behavior: "smooth" });
    }
  }

  function scanIncomingCall(convCid: string, list: any[]) {
    if (!me?.id || voiceCallHangup) return;
    const myLower = String(me.id).toLowerCase();
    const now = Date.now() / 1000;
    for (let i = list.length - 1; i >= 0; i--) {
      const m = list[i];
      if (!m?.id || m.kind !== "call_invite") continue;
      if (String(m.sender_id ?? "").toLowerCase() === myLower) continue;
      const sid = m.content?.session_id;
      if (typeof sid !== "string" || !sid) continue;
      if (dismissedCallMids.includes(m.id)) continue;
      const age = now - (Number(m.created_at) || 0);
      if (age > 180 || age < -30) continue;
      incomingCallPrompt = { cid: convCid, messageId: m.id, sessionId: sid };
      return;
    }
  }

  async function endVoiceCall() {
    remoteVoiceStream = null;
    voiceCallSetMicMuted = null;
    voiceCallSetAudioOut = null;
    voiceMicMuted = false;
    voiceBarMicMenuOpen = false;
    voiceBarSpeakerMenuOpen = false;
    voiceLocalLevel = 0;
    voiceRemoteLevel = 0;
    voiceRttMs = null;
    voiceConnState = "";
    voiceIceState = "";
    stopVoiceCallTicker();
    const h = voiceCallHangup;
    voiceCallHangup = null;
    if (h) await h();
  }

  function startOutboundVoice(cid: string, sessionId: string) {
    if (!token) return;
    void endVoiceCall().then(() => {
      if (!token) return;
      const { hangup, setMicMuted, setAudioOutputDeviceId } = runDmVoiceCall({
        baseUrl: base(),
        token,
        conversationId: cid,
        sessionId,
        role: "caller",
        micDeviceId: selectedMic || undefined,
        onPlaybackMode: () => {},
        onRemoteStream: (s) => {
          remoteVoiceStream = new MediaStream(s.getAudioTracks());
        },
        onPeerHangup: () => {
          showToast("Собеседник завершил звонок");
          void endVoiceCall();
        },
        onLevels: (lv) => {
          voiceLocalLevel = lv.local;
          voiceRemoteLevel = lv.remote;
        },
        onTelemetry: (t) => {
          voiceRttMs = t.rttMs;
          voiceConnState = t.connectionState;
          voiceIceState = t.iceConnectionState;
        },
        onSetupError: (msg) => {
          showToast(`Микрофон: ${msg}`);
          void endVoiceCall();
        },
        onConnectionState: (st) => {
          if (st === "failed") void endVoiceCall();
        },
      });
      voiceCallHangup = hangup;
      voiceCallSetMicMuted = setMicMuted;
      voiceCallSetAudioOut = setAudioOutputDeviceId;
      voiceMicMuted = false;
      setMicMuted(false);
      startVoiceCallTicker();
    });
  }

  async function acceptIncomingCall() {
    if (!incomingCallPrompt || !token) return;
    const { cid, sessionId, messageId } = incomingCallPrompt;
    dismissedCallMids = [...dismissedCallMids, messageId];
    incomingCallPrompt = null;
    await endVoiceCall();
    activeCid = cid;
    await loadMessages(cid, true);
    startPoll(cid);
    const { hangup, setMicMuted, setAudioOutputDeviceId } = runDmVoiceCall({
      baseUrl: base(),
      token,
      conversationId: cid,
      sessionId,
      role: "callee",
      micDeviceId: selectedMic || undefined,
      onPlaybackMode: () => {},
      onRemoteStream: (s) => {
        remoteVoiceStream = new MediaStream(s.getAudioTracks());
      },
      onPeerHangup: () => {
        showToast("Собеседник завершил звонок");
        void endVoiceCall();
      },
      onLevels: (lv) => {
        voiceLocalLevel = lv.local;
        voiceRemoteLevel = lv.remote;
      },
      onTelemetry: (t) => {
        voiceRttMs = t.rttMs;
        voiceConnState = t.connectionState;
        voiceIceState = t.iceConnectionState;
      },
      onSetupError: (msg) => {
        showToast(`Микрофон: ${msg}`);
        void endVoiceCall();
      },
      onConnectionState: (st) => {
        if (st === "failed") void endVoiceCall();
      },
    });
    voiceCallHangup = hangup;
    voiceCallSetMicMuted = setMicMuted;
    voiceCallSetAudioOut = setAudioOutputDeviceId;
    voiceMicMuted = false;
    setMicMuted(false);
    startVoiceCallTicker();
  }

  function declineIncomingCall() {
    if (!incomingCallPrompt) return;
    dismissedCallMids = [...dismissedCallMids, incomingCallPrompt.messageId];
    incomingCallPrompt = null;
  }

  function closeMessageMenu() {
    msgMenu = null;
  }

  function openMessageMenuAtCursor(e: MouseEvent, messageId: string) {
    e.preventDefault();
    e.stopPropagation();
    const mw = 224;
    const mh = 360;
    const x = Math.max(8, Math.min(e.clientX, window.innerWidth - mw - 8));
    const y = Math.max(8, Math.min(e.clientY, window.innerHeight - mh - 8));
    msgMenu = { messageId, x, y };
  }

  function openMessageMenuFromKebab(e: MouseEvent, messageId: string, mine: boolean) {
    e.stopPropagation();
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    const rect = el.getBoundingClientRect();
    const mw = 224;
    const mh = 360;
    let x = mine ? rect.right - mw : rect.left;
    let y = rect.bottom + 6;
    x = Math.max(8, Math.min(x, window.innerWidth - mw - 8));
    y = Math.max(8, Math.min(y, window.innerHeight - mh - 8));
    msgMenu = { messageId, x, y };
  }

  async function copyMessageText(text: string) {
    closeMessageMenu();
    try {
      await navigator.clipboard.writeText(text);
      showToast("Текст скопирован");
    } catch {
      showToast("Не удалось скопировать");
    }
  }

  async function copyPlain(text: string, ok = "Скопировано") {
    try {
      await navigator.clipboard.writeText(text);
      showToast(ok);
    } catch {
      showToast("Не удалось скопировать");
    }
  }

  function beginEditMessage(m: any) {
    closeMessageMenu();
    if (m.kind !== "text") return;
    editingMid = m.id;
    editDraft = String(m.content?.text ?? "");
  }

  function cancelEditMessage() {
    editingMid = null;
    editDraft = "";
  }

  async function saveEditMessage() {
    if (!activeCid || !editingMid) return;
    const t = editDraft.trim();
    if (!t) return;
    const r = await api(
      `/api/v1/conversations/${encodeURIComponent(activeCid)}/messages/${encodeURIComponent(editingMid)}`,
      {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ text: t }),
      },
    );
    const j = await r.json().catch(() => ({}));
    if (r.ok) {
      cancelEditMessage();
      void loadMessages(activeCid);
    } else showToast((j as { error?: string }).error || "Не удалось сохранить");
  }

  async function deleteChatMessage(mid: string, scope: "me" | "all") {
    if (!activeCid) return;
    closeMessageMenu();
    const r = await api(
      `/api/v1/conversations/${encodeURIComponent(activeCid)}/messages/${encodeURIComponent(mid)}?scope=${scope}`,
      { method: "DELETE" },
    );
    if (r.ok) void loadMessages(activeCid);
    else showToast("Не удалось удалить сообщение");
  }

  function pushLocalMessage(mid: string, kind: string, content: Record<string, unknown>) {
    if (!me?.id) return;
    messages = [
      ...messages,
      {
        id: mid,
        sender_id: me.id,
        kind,
        content,
        created_at: Date.now() / 1000,
        sender: {
          avatar_url: me.avatar_url,
          chat_primary_line: me.chat_primary_line,
          chat_secondary_line: me.chat_secondary_line,
        },
      },
    ];
  }

  function startPoll(cid: string) {
    if (pollTimer) clearInterval(pollTimer);
    pollTimer = setInterval(() => {
      void loadMessages(cid);
    }, 2000);
  }

  async function selectConv(cid: string) {
    closeMessageMenu();
    cancelEditMessage();
    replyTo = null;
    forwardFrom = null;
    await endVoiceCall();
    activeCid = cid;
    await loadMessages(cid, true);
    startPoll(cid);
  }

  async function sendText() {
    if (!activeCid || msgBusy) return;
    const trimmed = msgText.trim();
    if (!trimmed && !forwardFrom) return;
    const bodyText = trimmed || (forwardFrom ? "↩ Переслано" : "");
    const content: Record<string, unknown> = { text: bodyText };
    if (replyTo) {
      content.reply_to = {
        message_id: replyTo.id,
        preview: replyTo.preview,
        label: replyTo.label,
      };
    }
    if (forwardFrom) {
      content.forwarded_from = {
        message_id: forwardFrom.messageId,
        author_label: forwardFrom.label,
        text: forwardFrom.text,
      };
    }
    msgBusy = true;
    try {
      const r = await api(`/api/v1/conversations/${activeCid}/messages`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          kind: "text",
          content,
        }),
      });
      const j = await r.json().catch(() => ({}));
      if (r.ok && j.id) {
        msgText = "";
        replyTo = null;
        forwardFrom = null;
        pushLocalMessage(j.id, "text", content);
        void loadMessages(activeCid);
      }
    } finally {
      msgBusy = false;
    }
  }

  function uploadFilenameForMedia(file: File, kind: "image" | "gif" | "video" | "voice"): string {
    if (file.name && file.name !== "blob") return file.name;
    if (kind === "voice") return "voice.webm";
    if (kind === "video") return "clip.mp4";
    if (kind === "gif") return "image.gif";
    return "image.png";
  }

  async function uploadMedia(file: File, kind: "image" | "gif" | "video" | "voice") {
    if (!token || !activeCid) return;
    const fd = new FormData();
    fd.append("file", file, uploadFilenameForMedia(file, kind));
    const r = await fetch(`${base()}/api/v1/media`, {
      method: "POST",
      headers: { Authorization: `Bearer ${token}` },
      body: fd,
    });
    const j = await r.json().catch(() => ({}));
    if (!r.ok) {
      err = j.error || "upload";
      return;
    }
    const url = j.url;
    const r2 = await api(`/api/v1/conversations/${activeCid}/messages`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ kind, content: { url } }),
    });
    const mj = await r2.json().catch(() => ({}));
    if (r2.ok && mj.id) {
      pushLocalMessage(mj.id, kind, { url });
      void loadMessages(activeCid!);
    }
  }

  function onPickMedia(e: Event) {
    const t = e.currentTarget as HTMLInputElement;
    const f = t.files?.[0];
    t.value = "";
    if (!f) return;
    if (f.type.startsWith("video/")) void uploadMedia(f, "video");
    else if (f.type === "image/gif" || f.name.toLowerCase().endsWith(".gif")) void uploadMedia(f, "gif");
    else void uploadMedia(f, "image");
  }

  async function refreshAudioDevices() {
    try {
      const devs = await navigator.mediaDevices.enumerateDevices();
      audioInputs = devs.filter((d) => d.kind === "audioinput");
      audioOutputs = devs.filter((d) => d.kind === "audiooutput");
    } catch {
      audioInputs = [];
      audioOutputs = [];
    }
  }

  async function refreshMics() {
    await refreshAudioDevices();
  }

  async function toggleRecord() {
    if (recording && mediaRecorder && mediaRecorder.state !== "inactive") {
      mediaRecorder.stop();
      return;
    }
    try {
      const constraints: MediaStreamConstraints = {
        audio: selectedMic ? { deviceId: { exact: selectedMic } } : true,
      };
      const stream = await navigator.mediaDevices.getUserMedia(constraints);
      await refreshMics();
      recordChunks = [];
      mediaRecorder = new MediaRecorder(stream);
      mediaRecorder.ondataavailable = (ev) => {
        if (ev.data.size) recordChunks.push(ev.data);
      };
      mediaRecorder.onstop = async () => {
        stream.getTracks().forEach((tr) => tr.stop());
        recording = false;
        const blob = new Blob(recordChunks, { type: "audio/webm" });
        const file = new File([blob], `voice-${Date.now()}.webm`, { type: "audio/webm" });
        await uploadMedia(file, "voice");
      };
      mediaRecorder.start();
      recording = true;
    } catch (e) {
      err = formatFetchError(e);
    }
  }

  async function sendCallInvite() {
    if (!activeCid || callBusy) return;
    if (!isDirectDm) {
      showToast("Аудиозвонок доступен только в личных чатах");
      return;
    }
    const sessionId = crypto.randomUUID();
    callBusy = true;
    try {
      const r = await api(`/api/v1/conversations/${activeCid}/messages`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          kind: "call_invite",
          content: { v: 2, session_id: sessionId },
        }),
      });
      const j = await r.json().catch(() => ({}));
      if (!r.ok || !j.id) {
        err = (j as { error?: string }).error || "Не удалось отправить звонок";
        return;
      }
      pushLocalMessage(j.id, "call_invite", { v: 2, session_id: sessionId });
      void loadMessages(activeCid);
      showToast("Звонок… подключение");
      startOutboundVoice(activeCid, sessionId);
    } finally {
      callBusy = false;
    }
  }

  async function sendFriendInviteInChat(toHandle: string) {
    const h = toHandle.trim().replace(/^@+/, "");
    if (!activeCid || !h) return;
    const r = await api(`/api/v1/conversations/${activeCid}/messages`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        kind: "friend_invite",
        content: { to_handle: h },
      }),
    });
    const j = await r.json().catch(() => ({}));
    if (!r.ok || !j.id) return;
    pushLocalMessage(j.id, "friend_invite", { to_handle: h });
    void loadMessages(activeCid);
  }

  async function loadMyInstances() {
    try {
      const list = (await invoke("get_instances")) as any[];
      myInstances = Array.isArray(list) ? list : [];
    } catch {
      myInstances = [];
    }
  }

  function openServerInviteModal() {
    if (!activeCid) return;
    err = "";
    invServerIp = "";
    invTitle = "";
    invMode = "instance";
    invInstanceId = myInstances[0]?.id || "";
    invGameVersion = "";
    invFolders = {};
    invShowAdvanced = false;
    serverInviteOpen = true;
    void loadMyInstances().then(() => {
      if (!invInstanceId && myInstances[0]?.id) invInstanceId = myInstances[0].id;
      if (invInstanceId) void loadInvFoldersFor(invInstanceId);
    });
  }

  async function loadInvFoldersFor(id: string) {
    if (!id) return;
    try {
      const dirs = (await invoke("list_instance_folders", { id })) as string[];
      const state: Record<string, boolean> = {};
      for (const d of dirs || []) {
        state[d] = ["mods", "config", "resourcepacks", "shaderpacks"].includes(d);
      }
      invFolders = state;
    } catch {
      invFolders = { mods: true, config: true, resourcepacks: true, shaderpacks: true };
    }
  }

  function toggleInvFolder(key: string) {
    invFolders = { ...invFolders, [key]: !invFolders[key] };
  }

  function toggleInvAll(on: boolean) {
    invFolders = Object.fromEntries(Object.keys(invFolders).map((k) => [k, on]));
  }

  async function submitServerInviteModal() {
    if (!activeCid || !invServerIp.trim() || !invTitle.trim()) {
      err = "Укажите адрес сервера и название приглашения";
      return;
    }
    err = "";
    let jentlepack_url: string | undefined;
    let game_version: string | undefined;
    let instance_label: string | undefined;

    if (invMode === "instance") {
      if (!invInstanceId) {
        err = "Выберите сборку";
        return;
      }
      if (invSelectedFolders.length === 0) {
        err = "Отметьте хотя бы одну папку для экспорта";
        return;
      }
      invExportBusy = true;
      try {
        const tempPath = (await invoke("export_jentlepack_to_temp", {
          id: invInstanceId,
          selectedFolders: invSelectedFolders,
        })) as string;
        const b64 = (await invoke("read_data_tmp_file_base64", { path: tempPath })) as string;
        const bin = atob(b64);
        const u8 = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) u8[i] = bin.charCodeAt(i);
        const blob = new Blob([u8], { type: "application/octet-stream" });
        const file = new File([blob], "invite.jentlepack", { type: "application/octet-stream" });
        const fd = new FormData();
        fd.append("file", file, "invite.jentlepack");
        const up = await fetch(`${base()}/api/v1/media`, {
          method: "POST",
          headers: { Authorization: `Bearer ${token}` },
          body: fd,
        });
        const uj = await up.json().catch(() => ({}));
        if (!up.ok) throw new Error(uj.error || `upload ${up.status}`);
        jentlepack_url = uj.url;
        const inst = myInstances.find((i) => i.id === invInstanceId);
        instance_label = inst?.name || invInstanceId;
        if (inst?.game_version) game_version = String(inst.game_version);
      } catch (e) {
        err = formatFetchError(e);
        return;
      } finally {
        invExportBusy = false;
      }
    } else {
      game_version = invGameVersion.trim() || undefined;
    }

    const content: Record<string, unknown> = {
      title: invTitle.trim(),
      server_ip: invServerIp.trim(),
      jentlepack_url,
      game_version,
      instance_label,
    };
    const r = await api(`/api/v1/conversations/${activeCid}/messages`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ kind: "server_invite", content }),
    });
    const j = await r.json().catch(() => ({}));
    if (!r.ok || !j.id) {
      err = (j as { error?: string }).error || "Не отправилось";
      return;
    }
    pushLocalMessage(j.id, "server_invite", { ...content });
    void loadMessages(activeCid);
    serverInviteOpen = false;
    showToast("Приглашение отправлено");
  }

  async function acceptServerInviteFromMessage(content: Record<string, unknown> | undefined) {
    if (!content || acceptBusy) return;
    const ip = String(content.server_ip || "").trim();
    if (!ip) {
      showToast("В приглашении нет адреса сервера");
      return;
    }
    const url = String(content.jentlepack_url || "").trim();
    if (url) {
      if (!confirm("Установить сборку из приглашения и открыть вход на сервер?")) return;
      acceptBusy = true;
      try {
        const res = (await invoke("import_jentlepack_from_url", { url })) as {
          instance_id: string;
          message?: string;
        };
        showToast(res.message || "Сборка установлена");
        onNavigateLibraryWithServer?.(res.instance_id, ip);
      } catch (e) {
        showToast(`Ошибка установки: ${e}`);
      } finally {
        acceptBusy = false;
      }
    } else {
      showToast(
        `В приглашении нет файла .jentlepack. Версия для ручной сборки: ${content.game_version || "не указана"}.`,
      );
      onOpenLibrary?.();
    }
  }

  function onMsgPaste(e: ClipboardEvent) {
    if (!activeCid || !token) return;
    const items = e.clipboardData?.items;
    if (!items?.length) return;
    for (const it of items) {
      if (it.kind !== "file") continue;
      const f = it.getAsFile();
      if (!f) continue;
      if (f.type.startsWith("image/")) {
        e.preventDefault();
        const k = f.type === "image/gif" || f.name.toLowerCase().endsWith(".gif") ? "gif" : "image";
        void uploadMedia(f, k);
        return;
      }
      if (f.type.startsWith("video/")) {
        e.preventDefault();
        void uploadMedia(f, "video");
        return;
      }
    }
  }

  async function createPat() {
    if (!token) return;
    const r = await api("/api/v1/auth/access-tokens", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "launcher" }),
    });
    const j = await r.json().catch(() => ({}));
    if (r.ok) patJustCreated = j.access_token || "";
  }

  async function login() {
    busy = true;
    err = "";
    try {
      const r = await fetch(`${base()}/api/v1/auth/login`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ username: username.trim(), password }),
      });
      const j = await r.json().catch(() => ({}));
      if (!r.ok) throw new Error(j.detail || j.error || `HTTP ${r.status}`);
      token = j.access_token || "";
      if (token) {
        localStorage.setItem(TOKEN_KEY, token);
        await loadMe();
        void ensureNotificationPermission();
        await loadFriends();
        await loadConversations();
      }
    } catch (e) {
      err = formatFetchError(e);
    } finally {
      busy = false;
    }
  }

  function logout() {
    token = "";
    me = null;
    activeCid = null;
    messages = [];
    closeAvatarEditor();
    if (pollTimer) clearInterval(pollTimer);
    pollTimer = null;
    localStorage.removeItem(TOKEN_KEY);
    for (const k of Object.keys(notifyPrevIds)) delete notifyPrevIds[k];
    for (const k of Object.keys(notifySeenOnce)) delete notifySeenOnce[k];
  }

  $: if (me && tab === "friends") void loadFriends();
  $: if (me && tab === "chats") void loadConversations();

  $: micLabel = !selectedMic
    ? "По умолчанию"
    : audioInputs.find((d) => d.deviceId === selectedMic)?.label?.trim() || "Микрофон";

  function toggleVoiceMic() {
    voiceMicMuted = !voiceMicMuted;
    voiceCallSetMicMuted?.(voiceMicMuted);
  }

  function onGlobalPointerDown(e: PointerEvent) {
    const t = e.target;
    if (t instanceof Element) {
      if (
        t.closest?.("[data-msg-rctx]") ||
        t.closest?.("[data-msg-ctx-trigger]") ||
        t.closest?.("[data-voice-bar-pop]")
      )
        return;
    }
    closeMessageMenu();
    voiceBarMicMenuOpen = false;
    if (!micMenuOpen || !micMenuEl) return;
    if (t instanceof Node && micMenuEl.contains(t)) return;
    micMenuOpen = false;
  }

  onMount(() => {
    token = localStorage.getItem(TOKEN_KEY) || "";
    void loadMe().then(() => {
      if (token) {
        void ensureNotificationPermission();
        void loadFriends();
        void loadConversations();
        void loadMyInstances();
      }
    });
    void refreshMics();
    document.addEventListener("pointerdown", onGlobalPointerDown, true);
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      closeMessageMenu();
      voiceBarMicMenuOpen = false;
      cancelEditMessage();
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onGlobalPointerDown, true);
      document.removeEventListener("keydown", onKey);
    };
  });

  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer);
    if (avatarDraftUrl) URL.revokeObjectURL(avatarDraftUrl);
    void endVoiceCall();
  });
</script>

<div
  class="ui-page"
  in:fade={{ duration: 200 }}
>
  {#if me}
    <div class="flex items-center gap-2 shrink-0 jm-reveal">
      <div class="ui-seg">
        {#each TAB_BAR as item (item.id)}
          <button
            type="button"
            class="ui-seg-item"
            class:is-active={tab === item.id}
            on:click={() => (tab = item.id)}
            title={item.label}
          >
            <svelte:component this={item.icon} size={14} strokeWidth={2.2} />
            <span class="hidden sm:inline">{item.label}</span>
          </button>
        {/each}
      </div>
      <div class="ml-auto flex items-center gap-2">
        <button
          type="button"
          class="ui-btn ui-btn-subtle ui-btn-sm"
          title={`@${String(me.handle || me.username || "").replace(/^@+/, "")}`}
          on:click={() => (tab = "profile")}
        >
          {#if avatarSrc(me.avatar_url)}
            <img src={avatarSrc(me.avatar_url)} alt="" class="w-5 h-5 rounded-full object-cover" />
          {:else}
            <User size={14} />
          {/if}
          <span class="hidden sm:inline max-w-[10rem] truncate">
            {String(me.chat_primary_line || me.display_name || me.handle || "Я").trim()}
          </span>
        </button>
        <button
          type="button"
          class="ui-btn ui-btn-ghost ui-btn-icon hover:text-red-400"
          on:click={logout}
          title="Выйти"
          aria-label="Выйти"
        >
          <LogIn size={14} style="transform: rotate(180deg)" />
        </button>
      </div>
    </div>
  {/if}

  {#if err}
    <p class="text-xs text-red-400 shrink-0" transition:fade={{ duration: 150 }}>{err}</p>
  {/if}

  {#if !me}
    <div class="flex flex-1 min-h-0 items-center justify-center">
      <div
        class="ui-pane p-6 space-y-3 w-full max-w-sm jm-reveal"
        in:scale={{ duration: 320, easing: quintOut, start: 0.96 }}
      >
        <div class="flex items-center gap-2 text-jm-accent">
          <LogIn size={18} />
          <span class="font-bold text-sm">Вход в Jentle Social</span>
        </div>
        <input
          type="text"
          bind:value={username}
          placeholder="Логин"
          class="ui-input"
          autocomplete="username"
        />
        <input
          type="password"
          bind:value={password}
          placeholder="Пароль"
          class="ui-input"
          autocomplete="current-password"
          on:keydown={(e) => e.key === "Enter" && login()}
        />
        <button
          type="button"
          disabled={busy}
          on:click={login}
          class="ui-btn ui-btn-primary w-full py-3"
        >
          {busy ? "…" : "Войти"}
        </button>
      </div>
    </div>
  {:else}
    {#key tab}
      <div
        class="flex flex-col flex-1 min-h-0 overflow-hidden jm-reveal"
        style="animation-delay: 0.02s"
        in:fly={{ y: 10, duration: 260, easing: quintOut }}
      >
        {#if tab === "profile"}
          <div
            class="ui-pane ui-pane-soft flex flex-col flex-1 min-h-0 w-full max-w-2xl mx-auto overflow-hidden"
          >
            <div class="relative shrink-0 h-28 w-full overflow-hidden" in:fade={{ duration: 200 }}>
              {#if resolveSiteMediaUrl(apiBaseUrl, me.profile_banner_url)}
                <div
                  class="absolute inset-0 bg-cover bg-center"
                  style:background-image={`url('${String(resolveSiteMediaUrl(apiBaseUrl, me.profile_banner_url)).replace(/'/g, "%27")}')`}
                />
                <div class="absolute inset-0 bg-gradient-to-t from-jm-card via-jm-card/40 to-transparent" />
              {:else}
                <div
                  class="absolute inset-0 bg-gradient-to-br from-indigo-950/90 via-jm-card to-purple-950/70"
                />
              {/if}
            </div>
            <div
              class="flex-1 min-h-0 overflow-y-auto custom-scrollbar px-4 pb-4 pt-0 -mt-10 relative flex flex-col gap-3"
            >
              <div class="flex gap-3 items-end" in:fade={{ duration: 200 }}>
                {#if avatarSrc(me.avatar_url)}
                  <img
                    src={avatarSrc(me.avatar_url)}
                    alt=""
                    class="w-[4.5rem] h-[4.5rem] rounded-full object-cover ring-4 ring-jm-card shrink-0 transition-transform hover:scale-[1.02]"
                  />
                {:else}
                  <div
                    class="w-[4.5rem] h-[4.5rem] rounded-full bg-black/40 flex items-center justify-center ring-4 ring-jm-card shrink-0"
                  >
                    <User size={28} class="text-[var(--text-secondary)]" />
                  </div>
                {/if}
                <div class="min-w-0 flex-1 pb-0.5">
                  <p class="font-bold text-white text-lg truncate drop-shadow-sm">
                    {me.chat_primary_line || me.display_name}
                  </p>
                  {#if me.chat_secondary_line}
                    <p class="text-xs text-jm-accent/90 truncate">{me.chat_secondary_line}</p>
                  {/if}
                  {#if me.handle}
                    <p class="text-[10px] text-[var(--text-secondary)] mt-0.5 opacity-90">
                      @{me.handle} · ник нельзя сменить
                    </p>
                  {/if}
                </div>
              </div>

              {#if me.profile_banner_allowed}
                <div class="space-y-1" in:fly={{ y: 6, duration: 220, delay: 40, easing: quintOut }}>
                  <label class="text-[11px] text-[var(--text-secondary)]" for="jm-tab-banner">Баннер (URL)</label>
                  <input
                    id="jm-tab-banner"
                    type="url"
                    bind:value={editBannerUrl}
                    placeholder="https://…"
                    class="w-full rounded-xl px-3 py-2 text-sm border border-[var(--border)] bg-[var(--input-bg)] transition-colors focus:border-jm-accent/40 font-mono text-xs"
                  />
                </div>
              {:else}
                <p class="text-[10px] text-amber-200/80 leading-snug" in:fade={{ duration: 180 }}>
                  Баннер доступен для ролей с сайта и персонала. После синхронизации роли откройте профиль снова.
                </p>
              {/if}

              <div class="space-y-1" in:fly={{ y: 6, duration: 220, delay: 60, easing: quintOut }}>
                <label class="text-[11px] text-[var(--text-secondary)]" for="jm-tab-bio">О себе</label>
                <textarea
                  id="jm-tab-bio"
                  bind:value={editBio}
                  maxlength="500"
                  rows="3"
                  placeholder="Короткое описание профиля…"
                  class="w-full rounded-xl px-3 py-2 text-sm border border-[var(--border)] bg-[var(--input-bg)] transition-colors focus:border-jm-accent/40 resize-y min-h-[4.5rem]"
                />
                <p class="text-[9px] text-white/35">{editBio.length}/500</p>
              </div>

              <div class="space-y-2" in:fly={{ y: 6, duration: 220, delay: 80, easing: quintOut }}>
                <label class="text-[11px] text-[var(--text-secondary)]" for="jm-tab-mc">Minecraft (ник в чате)</label>
                <input
                  id="jm-tab-mc"
                  type="text"
                  bind:value={editMc}
                  class="w-full rounded-xl px-3 py-2 text-sm border border-[var(--border)] bg-[var(--input-bg)] transition-colors focus:border-jm-accent/40"
                />
              </div>

              {#if chatProfileMcServer}
                <div class="space-y-1" in:fly={{ y: 6, duration: 220, delay: 90, easing: quintOut }}>
                  <label class="text-[11px] text-[var(--text-secondary)]" for="jm-tab-mchost"
                    >Хост сервера (для карточки профиля)</label
                  >
                  <input
                    id="jm-tab-mchost"
                    type="text"
                    bind:value={editMcHost}
                    placeholder="play.example.com"
                    class="w-full rounded-xl px-3 py-2 text-sm border border-[var(--border)] bg-[var(--input-bg)] font-mono text-xs transition-colors focus:border-jm-accent/40"
                  />
                </div>
              {/if}

            <div
              class="rounded-xl border border-white/10 bg-black/20 p-3 mb-3 space-y-3"
              in:fly={{ y: 6, duration: 220, delay: 90, easing: quintOut }}
            >
              <p class="text-[11px] font-bold text-[var(--text-secondary)]">Аватар</p>
              <p class="text-[10px] text-white/45">Загрузите файл с диска (обрежете в редакторе и сохраните).</p>
              <div class="flex flex-wrap gap-2 items-center">
                <label
                  class="inline-flex items-center gap-2 px-3 py-2 rounded-xl text-xs font-bold bg-jm-accent/15 text-jm-accent border border-jm-accent/35 cursor-pointer transition-all hover:bg-jm-accent/25 jm-tap-scale"
                >
                  <Upload size={16} />
                  Выбрать файл…
                  <input type="file" accept="image/*" class="hidden" on:change={onAvatarFilePick} />
                </label>
              </div>

              {#if avatarEditorOpen && avatarDraftUrl}
                <div
                  class="space-y-3 pt-2 border-t border-white/10"
                  transition:fade={{ duration: 200 }}
                >
                  <img
                    bind:this={avatarImgEl}
                    src={avatarDraftUrl}
                    alt=""
                    class="hidden"
                    on:load={onAvatarImgLoad}
                  />
                  <div class="flex flex-col sm:flex-row gap-4 items-center justify-center">
                    <div class="relative rounded-xl overflow-hidden border border-white/15 shadow-lg">
                      <canvas
                        bind:this={editorCv}
                        width="240"
                        height="240"
                        class="w-[200px] h-[200px] sm:w-[240px] sm:h-[240px] touch-none cursor-grab active:cursor-grabbing block"
                        on:pointerdown={onEditorPointerDown}
                        on:pointermove={onEditorPointerMove}
                        on:pointerup={onEditorPointerUp}
                        on:pointercancel={onEditorPointerUp}
                      />
                    </div>
                    <div class="flex flex-col items-center gap-2">
                      <p class="text-[10px] text-[var(--text-secondary)]">Предпросмотр</p>
                      <div
                        class="rounded-full overflow-hidden ring-2 ring-jm-accent/50 shadow-xl w-[112px] h-[112px] transition-transform hover:scale-105"
                      >
                        <canvas bind:this={previewCv} width="112" height="112" class="block w-full h-full" />
                      </div>
                    </div>
                  </div>
                  <div class="space-y-1">
                    <label class="text-[10px] text-[var(--text-secondary)]">Масштаб</label>
                    <input
                      type="range"
                      min="1"
                      max="3"
                      step="0.02"
                      bind:value={editorZoom}
                      on:input={syncEditorCanvases}
                      class="w-full accent-jm-accent"
                    />
                  </div>
                  <div class="flex gap-2">
                    <button
                      type="button"
                      disabled={avatarBusy}
                      on:click={applyAvatarCrop}
                      class="flex-1 py-2 rounded-xl text-xs font-bold bg-jm-accent text-black disabled:opacity-50 transition-transform active:scale-[0.98] jm-tap-scale"
                    >
                      {avatarBusy ? "…" : "Сохранить аватар"}
                    </button>
                    <button
                      type="button"
                      on:click={closeAvatarEditor}
                      class="p-2 rounded-xl bg-white/10 border border-white/15 transition-colors hover:bg-white/15"
                      title="Отмена"
                    >
                      <X size={18} />
                    </button>
                  </div>
                </div>
              {/if}
            </div>

            <button
              type="button"
              disabled={saveBusy}
              on:click={saveProfile}
              class="w-full py-2.5 rounded-xl font-bold text-sm bg-white/10 border border-white/15 hover:bg-white/15 disabled:opacity-50 transition-all jm-tap-scale mb-2"
            >
              {saveBusy ? "…" : "Сохранить профиль"}
            </button>

            <div
              class="border-t border-white/10 pt-3 flex flex-wrap items-center gap-2"
              in:fade={{ duration: 200, delay: 120 }}
            >
              <button
                type="button"
                on:click={createPat}
                class="inline-flex items-center gap-2 px-3 py-2 rounded-xl text-xs font-bold bg-white/5 border border-white/10 hover:bg-white/10 transition-all jm-tap-scale"
                title="Токен для API"
              >
                <KeyRound size={16} />
                PAT
              </button>
              {#if patJustCreated}
                <p
                  class="text-[9px] font-mono break-all text-jm-accent bg-black/40 p-2 rounded-lg flex-1 min-w-0 max-h-20 overflow-y-auto custom-scrollbar"
                  transition:fade
                >
                  {patJustCreated}
                </p>
              {/if}
            </div>
            </div>
          </div>
        {:else if tab === "friends"}
          <div
            class="ui-pane p-3 flex flex-col flex-1 min-h-0 w-full max-w-2xl mx-auto overflow-hidden"
          >
            <div class="flex gap-2 shrink-0 mb-3">
              <input
                type="text"
                bind:value={friendHandle}
                placeholder="handle"
                class="flex-1 rounded-xl px-3 py-2 text-sm border border-[var(--border)] bg-[var(--input-bg)]"
                on:keydown={(e) => e.key === "Enter" && requestFriend()}
              />
              <button
                type="button"
                disabled={friendsBusy}
                on:click={requestFriend}
                class="px-3 py-2 rounded-xl bg-jm-accent text-black transition-transform active:scale-95 disabled:opacity-50 jm-tap-scale"
              >
                <PlusCircle size={20} />
              </button>
            </div>
            <input
              type="search"
              bind:value={friendSearchQuery}
              placeholder="Поиск среди друзей…"
              class="w-full rounded-xl px-3 py-2 text-xs border border-[var(--border)] bg-[var(--input-bg)] mb-2 shrink-0"
            />
            <div class="flex-1 min-h-0 overflow-y-auto custom-scrollbar space-y-2 pr-1">
              {#if pendingTo.length}
                <p class="text-[10px] uppercase tracking-wide text-[var(--text-secondary)] px-1">
                  Ожидают ответа
                </p>
                {#each pendingTo as p (p.user_id)}
                  <div
                    class="flex items-center gap-2 justify-between bg-amber-500/10 rounded-xl p-2 border border-amber-500/20"
                    in:fly={{ x: -8, duration: 220, easing: quintOut }}
                    role="listitem"
                    on:contextmenu|preventDefault|stopPropagation={() => void openProfileById(p.user_id, false)}
                  >
                    <div class="flex items-center gap-2 min-w-0">
                      {#if avatarSrc(p.avatar_url)}
                        <img
                          src={avatarSrc(p.avatar_url)}
                          alt=""
                          class="w-9 h-9 rounded-full object-cover shrink-0 ring-1 ring-white/10"
                        />
                      {:else}
                        <div class="w-9 h-9 rounded-full bg-black/40 shrink-0" />
                      {/if}
                      <div class="min-w-0">
                        <p class="text-sm font-bold truncate">{p.display_name}</p>
                        <p class="text-[10px] text-jm-accent truncate">@{p.handle}</p>
                        {#if p.last_active_at != null}
                          <p class="text-[9px] text-white/40 truncate">
                            {formatPresence(p.last_active_at)}
                          </p>
                        {/if}
                      </div>
                    </div>
                    <span class="text-[10px] text-amber-200/80 shrink-0 px-1">Отправлено</span>
                  </div>
                {/each}
              {/if}
              {#if pendingFrom.length}
                <p class="text-[10px] uppercase tracking-wide text-[var(--text-secondary)] px-1">
                  Входящие
                </p>
              {/if}
              {#each pendingFrom as p (p.user_id)}
                <div
                  class="flex items-center gap-2 justify-between bg-white/5 rounded-xl p-2 transition-all hover:bg-white/10"
                  in:fly={{ x: -8, duration: 220, easing: quintOut }}
                  role="listitem"
                  on:contextmenu|preventDefault|stopPropagation={() => void openProfileById(p.user_id, false)}
                >
                  <div class="flex items-center gap-2 min-w-0">
                    {#if avatarSrc(p.avatar_url)}
                      <img
                        src={avatarSrc(p.avatar_url)}
                        alt=""
                        class="w-9 h-9 rounded-full object-cover shrink-0 ring-1 ring-white/10"
                      />
                    {:else}
                      <div class="w-9 h-9 rounded-full bg-black/40 shrink-0" />
                    {/if}
                    <div class="min-w-0">
                      <p class="text-sm font-bold truncate">{p.display_name}</p>
                      <p class="text-[10px] text-jm-accent truncate">@{p.handle}</p>
                      {#if p.last_active_at != null}
                        <p class="text-[9px] text-white/40 truncate">
                          {formatPresence(p.last_active_at)}
                        </p>
                      {/if}
                    </div>
                  </div>
                  <button
                    type="button"
                    class="p-2 rounded-lg bg-green-500/20 text-green-300 shrink-0 transition-transform hover:scale-110 jm-tap-scale"
                    on:click={() => acceptFriend(p.user_id)}
                  >
                    <Check size={18} />
                  </button>
                </div>
              {/each}
              {#each friendsFiltered as f (f.user_id)}
                <button
                  type="button"
                  class="w-full flex items-center gap-2 text-left bg-white/5 rounded-xl p-2 hover:bg-white/10 transition-all duration-200 hover:translate-x-0.5 jm-tap-scale"
                  on:click={() => openDm(f.user_id)}
                  on:contextmenu|preventDefault|stopPropagation={() => void openProfileById(f.user_id, false)}
                  in:fly={{ x: -8, duration: 220, easing: quintOut }}
                >
                  {#if avatarSrc(f.avatar_url)}
                    <img
                      src={avatarSrc(f.avatar_url)}
                      alt=""
                      class="w-10 h-10 rounded-full object-cover shrink-0 ring-1 ring-white/10"
                    />
                  {:else}
                    <div class="w-10 h-10 rounded-full bg-black/40 shrink-0" />
                  {/if}
                  <div class="min-w-0 flex-1">
                    <p class="text-sm font-bold truncate">{f.chat_primary_line || f.display_name}</p>
                    <p class="text-[10px] text-jm-accent truncate">{f.chat_secondary_line || ""}</p>
                    {#if f.public_id}
                      <p class="text-[9px] text-white/35 font-mono truncate" title="Публичный ID"
                        >{f.public_id}</p
                      >
                    {/if}
                    {#if f.last_active_at != null}
                      <p class="text-[9px] text-white/40 truncate">{formatPresence(f.last_active_at)}</p>
                    {/if}
                  </div>
                  <MessageCircle size={16} class="text-[var(--text-secondary)] shrink-0" />
                </button>
              {:else}
                {#if friendSearchLower}
                  <p class="text-xs text-[var(--text-secondary)] p-2 text-center opacity-70">
                    Никого не найдено
                  </p>
                {:else if !pendingFrom.length && !pendingTo.length}
                  <p class="text-xs text-[var(--text-secondary)] p-2 text-center opacity-70">Пусто</p>
                {/if}
              {/each}
            </div>
          </div>
        {:else if tab === "chats"}
          <div class="flex flex-col gap-2 flex-1 min-h-0 overflow-hidden">
            <!-- Создать группу — компактно в одну строку. -->
            <div class="flex gap-2 shrink-0 flex-wrap">
              <input
                type="text"
                bind:value={newGroupTitle}
                placeholder="Название группы"
                class="ui-input flex-1 min-w-[140px] text-xs"
              />
              <input
                type="text"
                bind:value={newGroupHandles}
                placeholder="@user1 @user2 …"
                class="ui-input flex-[2] min-w-[160px] text-xs"
              />
              <button
                type="button"
                on:click={createGroup}
                class="ui-btn ui-btn-subtle ui-btn-sm"
                title="Создать групповой чат"
              >
                <PlusCircle size={14} />
                Создать
              </button>
            </div>
            <div
              class="flex flex-col lg:flex-row gap-3 flex-1 min-h-0 overflow-hidden"
              style="min-height: 0;"
            >
              <!-- Левый sidebar: полноценный список диалогов с аватаром, именем, презенс-точкой. -->
              <div
                class="lg:w-[320px] shrink-0 flex flex-col min-h-[220px] max-h-[min(300px,40vh)] lg:max-h-none lg:h-full ui-card !p-0 overflow-hidden"
              >
                <div class="px-3 py-2.5 border-b border-[var(--border)] flex items-center justify-between gap-2 shrink-0">
                  <div class="flex items-center gap-2">
                    <MessageCircle size={14} class="text-[var(--accent-light)]" />
                    <span class="ui-section-title !mb-0 text-[11px]">
                      Диалоги {directSorted.length + groupConversations.length > 0 ? `· ${directSorted.length + groupConversations.length}` : ""}
                    </span>
                  </div>
                </div>
                <div class="flex-1 min-h-0 overflow-y-auto custom-scrollbar flex flex-col">
                  {#each directSorted as c (c.id)}
                    {@const isActive = activeCid === c.id}
                    {@const online =
                      c.direct_peer?.last_active_at != null &&
                      Date.now() / 1000 - Number(c.direct_peer.last_active_at) < 120}
                    <button
                      type="button"
                      class="relative w-full text-left px-3 py-2.5 border-b border-[var(--border)] transition-colors duration-150 flex items-center gap-3 {isActive
                        ? 'bg-[var(--accent-softer)]'
                        : 'hover:bg-[var(--surface-hover)]'}"
                      on:click={() => selectConv(c.id)}
                      on:contextmenu|preventDefault|stopPropagation={() => {
                        const uid = c.direct_peer?.user_id;
                        if (!uid) return;
                        const mine =
                          me?.id != null &&
                          String(me.id).toLowerCase() === String(uid).toLowerCase();
                        void openProfileById(String(uid), mine);
                      }}
                      in:fly={{ x: -6, duration: 200, easing: quintOut }}
                    >
                      {#if isActive}
                        <span class="absolute left-0 top-2 bottom-2 w-0.5 rounded-r-full bg-[var(--accent)]" aria-hidden="true"></span>
                      {/if}
                      <div class="relative shrink-0">
                        {#if avatarSrc(c.direct_peer?.avatar_url)}
                          <img
                            src={avatarSrc(c.direct_peer.avatar_url)}
                            alt=""
                            class="w-11 h-11 rounded-full object-cover ring-1 ring-[var(--border)]"
                          />
                        {:else}
                          <div class="w-11 h-11 rounded-full bg-[var(--surface-2)] ring-1 ring-[var(--border)] flex items-center justify-center text-[var(--text-secondary)]">
                            <User size={18} />
                          </div>
                        {/if}
                        {#if online}
                          <span
                            class="absolute bottom-0 right-0 w-3 h-3 rounded-full bg-emerald-400 ring-2 ring-[var(--surface-1)]"
                            aria-label="В сети"
                          ></span>
                        {/if}
                      </div>
                      <span class="min-w-0 flex-1">
                        <span class="font-semibold text-sm text-[var(--text)] block truncate">{directRowTitle(c)}</span>
                        {#if directRowSubtitle(c)}
                          <span class="text-[11px] text-[var(--accent-light)] truncate block">{directRowSubtitle(c)}</span>
                        {/if}
                        {#if c.direct_peer?.last_active_at != null}
                          <span class="text-[10px] text-[var(--text-secondary)] truncate block"
                            >{formatPresence(c.direct_peer.last_active_at)}</span
                          >
                        {/if}
                      </span>
                    </button>
                  {/each}
                  {#if groupConversations.length > 0}
                    {#if directSorted.length > 0}
                      <div class="px-3 pt-3 pb-1 text-[10px] uppercase tracking-wider text-[var(--text-secondary)] font-semibold shrink-0">
                        Группы
                      </div>
                    {/if}
                    {#each groupConversations as c (c.id)}
                      {@const isActive = activeCid === c.id}
                      <button
                        type="button"
                        class="relative w-full text-left px-3 py-2.5 border-b border-[var(--border)] transition-colors duration-150 flex items-center gap-3 {isActive
                          ? 'bg-[var(--accent-softer)]'
                          : 'hover:bg-[var(--surface-hover)]'}"
                        on:click={() => selectConv(c.id)}
                        in:fly={{ x: -6, duration: 200, easing: quintOut }}
                      >
                        {#if isActive}
                          <span class="absolute left-0 top-2 bottom-2 w-0.5 rounded-r-full bg-[var(--accent)]" aria-hidden="true"></span>
                        {/if}
                        <div class="w-11 h-11 rounded-full bg-[var(--accent-softer)] shrink-0 flex items-center justify-center text-[var(--accent)] ring-1 ring-[var(--accent)]/30">
                          <Users size={18} />
                        </div>
                        <span class="font-semibold text-sm text-[var(--text)] truncate min-w-0">{c.title || "Группа"}</span>
                      </button>
                    {/each}
                  {/if}
                  {#if directSorted.length === 0 && groupConversations.length === 0}
                    <div class="flex flex-col items-center justify-center py-8 px-4 text-center gap-2">
                      <div class="w-12 h-12 rounded-full bg-[var(--accent-softer)] flex items-center justify-center text-[var(--accent)]">
                        <MessageCircle size={22} />
                      </div>
                      <p class="text-xs text-[var(--text-secondary)]">Пока нет диалогов</p>
                      <p class="text-[10px] text-[var(--text-secondary)] opacity-70">Добавьте друзей во вкладке «Друзья»</p>
                    </div>
                  {/if}
                </div>
              </div>
              <div
                class="flex-1 min-w-0 flex flex-col min-h-[320px] ui-card !p-0 overflow-hidden relative"
              >
                {#if !activeCid}
                  <div class="absolute inset-0 flex flex-col items-center justify-center gap-3 px-6 text-center">
                    <div class="w-20 h-20 rounded-full bg-[var(--accent-softer)] flex items-center justify-center text-[var(--accent)]">
                      <MessageCircle size={40} strokeWidth={1.6} />
                    </div>
                    <p class="text-lg font-semibold text-[var(--text)]">Выберите диалог</p>
                    <p class="text-sm text-[var(--text-secondary)] max-w-xs">
                      Выберите чат слева или найдите собеседника во вкладке «Друзья», чтобы начать общение.
                    </p>
                    <button
                      type="button"
                      class="ui-btn ui-btn-subtle ui-btn-sm"
                      on:click={() => (tab = "friends")}
                    >
                      <Users size={14} /> Перейти к друзьям
                    </button>
                  </div>
                {/if}
                {#if activeCid}
                  <!-- Хедер диалога: аватар, имя, онлайн-статус и кнопка звонка. Непрозрачный фон, чтобы текст сообщений не просвечивался снизу. -->
                  <div
                    class="shrink-0 flex items-center gap-3 px-3 py-2.5 border-b border-[var(--border)] bg-[var(--surface-1)] z-10"
                  >
                    {#if isDirectDm && activeConversation?.direct_peer}
                      {@const peer = activeConversation.direct_peer}
                      {@const online =
                        peer.last_active_at != null &&
                        Date.now() / 1000 - Number(peer.last_active_at) < 120}
                      <!-- svelte-ignore a11y_click_events_have_key_events -->
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <div
                        class="relative shrink-0 cursor-pointer"
                        title="Открыть профиль"
                        on:click={() => {
                          const uid = peer?.user_id;
                          if (!uid) return;
                          const mine =
                            me?.id != null &&
                            String(me.id).toLowerCase() === String(uid).toLowerCase();
                          void openProfileById(String(uid), mine);
                        }}
                      >
                        {#if avatarSrc(peer.avatar_url)}
                          <img src={avatarSrc(peer.avatar_url)} alt="" class="w-9 h-9 rounded-full object-cover ring-1 ring-[var(--border)]" />
                        {:else}
                          <div class="w-9 h-9 rounded-full bg-[var(--surface-2)] ring-1 ring-[var(--border)] flex items-center justify-center text-[var(--text-secondary)]"><User size={14} /></div>
                        {/if}
                        {#if online}
                          <span class="absolute bottom-0 right-0 w-2.5 h-2.5 rounded-full bg-emerald-400 ring-2 ring-[var(--surface-1)]" aria-label="В сети"></span>
                        {/if}
                      </div>
                      <div class="min-w-0 flex-1">
                        <div class="font-semibold text-sm text-[var(--text)] truncate">{directRowTitle(activeConversation)}</div>
                        <div class="text-[11px] text-[var(--text-secondary)] truncate">
                          {online ? "в сети" : formatPresence(peer.last_active_at)}
                        </div>
                      </div>
                      <button
                        type="button"
                        class="ui-btn ui-btn-ghost ui-btn-icon"
                        title="Звонок"
                        aria-label="Позвонить"
                        disabled={callBusy}
                        on:click={() => void sendCallInvite()}
                      >
                        {#if callBusy}
                          <Loader2 size={16} class="animate-spin" />
                        {:else}
                          <Phone size={16} />
                        {/if}
                      </button>
                    {:else if activeConversation}
                      <div class="w-9 h-9 rounded-full bg-[var(--accent-softer)] text-[var(--accent)] flex items-center justify-center shrink-0 ring-1 ring-[var(--accent)]/30">
                        <Users size={16} />
                      </div>
                      <div class="min-w-0 flex-1">
                        <div class="font-semibold text-sm text-[var(--text)] truncate">{activeConversation.title || "Групповой чат"}</div>
                        <div class="text-[11px] text-[var(--text-secondary)] truncate">Групповой чат</div>
                      </div>
                    {/if}
                  </div>
                  {#if voiceCallHangup}
                    {@const connected =
                      voiceConnState === "connected" ||
                      voiceIceState === "connected" ||
                      voiceIceState === "completed"}
                    {@const errored =
                      voiceConnState === "failed" || voiceIceState === "failed"}
                    {@const reconnecting =
                      voiceIceState === "disconnected" && voiceConnState === "connected"}
                    <!-- Баннер состояния звонка — статически ниже хедера, чтобы не наезжал ни на сообщения, ни на композитор. -->
                    <div
                      class="shrink-0 mx-3 mt-2 flex items-center justify-center rounded-full border px-3 py-1.5 text-[11px] font-semibold leading-none gap-2 shadow-sm {errored
                        ? 'border-red-400/50 bg-red-500/15 text-red-100'
                        : reconnecting
                          ? 'border-amber-300/40 bg-amber-500/15 text-amber-100'
                          : connected
                            ? 'border-emerald-400/40 bg-emerald-500/15 text-emerald-100'
                            : 'border-[var(--border)] bg-[var(--surface-2)] text-[var(--text)]'}"
                      aria-live="polite"
                    >
                      <span
                        class="relative flex h-2 w-2"
                        aria-hidden="true"
                      >
                        <span
                          class="absolute inline-flex h-full w-full rounded-full opacity-60 {errored
                            ? 'bg-red-300'
                            : reconnecting
                              ? 'bg-amber-300 animate-ping'
                              : connected
                                ? 'bg-emerald-300'
                                : 'bg-white/70 animate-ping'}"
                        />
                        <span
                          class="relative inline-flex rounded-full h-2 w-2 {errored
                            ? 'bg-red-400'
                            : reconnecting
                              ? 'bg-amber-400'
                              : connected
                                ? 'bg-emerald-400'
                                : 'bg-white'}"
                        />
                      </span>
                      <span>{callStateLabel}</span>
                      {#if connected}
                        <span class="font-mono opacity-85">{formatCallElapsed(voiceCallElapsed)}</span>
                      {/if}
                      {#if voiceRttMs != null}
                        <span class="font-mono opacity-70">·&nbsp;{Math.round(voiceRttMs)} мс</span>
                      {/if}
                    </div>
                  {/if}
                  <div
                    bind:this={messagesScrollEl}
                    class="flex-1 min-h-0 overflow-y-auto custom-scrollbar p-3 space-y-2.5 flex flex-col"
                  >
                    {#each messages as m (m.id)}
                      {@const mine =
                        me?.id != null &&
                        String(m.sender_id ?? "").toLowerCase() === String(me.id).toLowerCase()}
                      {@const av = isDirectDm ? "w-7 h-7" : "w-9 h-9"}
                      {@const avWrap = isDirectDm ? "w-7" : "w-9"}
                      <div
                        class="flex w-full {mine ? 'justify-end' : 'justify-start'}"
                        in:fly={{ y: 8, duration: 240, easing: quintOut }}
                      >
                        <div
                          class="flex gap-2 max-w-[min(88%,44rem)] items-end group relative {mine
                            ? 'flex-row-reverse'
                            : 'flex-row'}"
                        >
                          <div
                            class="shrink-0 flex flex-col justify-end pb-0.5 {avWrap}"
                            role="button"
                            tabindex="0"
                            on:contextmenu|preventDefault|stopPropagation={() =>
                              void openProfileById(String(m.sender_id), mine)}
                            on:keydown={(e) => {
                              if (e.key === "Enter" || e.key === " ") {
                                e.preventDefault();
                                void openProfileById(String(m.sender_id), mine);
                              }
                            }}
                            title="Профиль (ПКМ или Enter)"
                          >
                            {#if avatarSrc(m.sender?.avatar_url)}
                              <img
                                src={avatarSrc(m.sender.avatar_url)}
                                alt=""
                                class="{av} rounded-full object-cover ring-1 {mine
                                  ? 'ring-jm-accent/35'
                                  : 'ring-white/10'}"
                              />
                            {:else}
                              <div class="{av} rounded-full {mine ? 'bg-jm-accent/20' : 'bg-white/10'}" />
                            {/if}
                          </div>
                          <div
                            class="relative min-w-0 max-w-full"
                            on:contextmenu={(e) => {
                              if (m.kind === "deleted") return;
                              openMessageMenuAtCursor(e, m.id);
                            }}
                          >
                            <div
                              class="rounded-2xl border shadow-sm min-w-0 {isDirectDm
                                ? 'px-2.5 py-1.5 text-sm'
                                : 'px-3.5 py-2.5 text-sm'} {mine
                                ? 'bg-jm-accent/15 border-jm-accent/35 rounded-br-md'
                                : 'bg-black/35 border-white/10 rounded-bl-md'}"
                            >
                            {#if !mine}
                              {#if isDirectDm}
                                <span
                                  class="text-[9px] uppercase tracking-wide text-white/40 mb-0.5 block truncate max-w-[13rem]"
                                  >{senderDmLabel(m.sender)}</span
                                >
                              {:else}
                                <div class="flex items-baseline gap-2 mb-1 flex-wrap">
                                  <span class="font-bold text-jm-accent-light text-xs"
                                    >{m.sender?.chat_primary_line || "—"}</span
                                  >
                                  {#if m.sender?.chat_secondary_line}
                                    <span class="text-[10px] text-[var(--text-secondary)]"
                                      >{m.sender.chat_secondary_line}</span
                                    >
                                  {/if}
                                </div>
                              {/if}
                            {/if}
                            {#if m.kind === "deleted"}
                              <p class="text-[var(--text-secondary)] italic text-sm">Сообщение удалено</p>
                            {:else if m.kind === "text"}
                              {#if editingMid === m.id}
                                <textarea
                                  bind:value={editDraft}
                                  rows={Math.min(8, Math.max(2, editDraft.split("\n").length))}
                                  class="w-full min-h-[3rem] rounded-lg px-2 py-1.5 text-sm bg-black/40 border border-jm-accent/40 text-[var(--text)] resize-y"
                                  on:keydown={(e) => {
                                    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                                      e.preventDefault();
                                      void saveEditMessage();
                                    }
                                  }}
                                ></textarea>
                                <div class="flex flex-wrap gap-1.5 mt-1.5">
                                  <button
                                    type="button"
                                    class="px-2.5 py-1 rounded-lg bg-jm-accent text-black text-xs font-bold"
                                    on:click={() => void saveEditMessage()}
                                  >
                                    Сохранить
                                  </button>
                                  <button
                                    type="button"
                                    class="px-2.5 py-1 rounded-lg bg-white/10 text-xs font-bold"
                                    on:click={cancelEditMessage}
                                  >
                                    Отмена
                                  </button>
                                  <span class="text-[10px] text-white/40 self-center">Ctrl+Enter — сохранить</span>
                                </div>
                              {:else}
                                {#if m.content?.reply_to}
                                  <div
                                    class="text-[10px] text-white/55 mb-1.5 pl-2 border-l-2 border-jm-accent/45 rounded-sm"
                                  >
                                    <span class="font-bold text-jm-accent-light/90 block truncate"
                                      >{m.content.reply_to.label || "Ответ"}</span
                                    >
                                    <span class="block truncate opacity-90"
                                      >{m.content.reply_to.preview || ""}</span
                                    >
                                  </div>
                                {/if}
                                {#if m.content?.forwarded_from}
                                  <div
                                    class="mb-2 text-sm text-white/88 whitespace-pre-wrap border-l-2 border-jm-accent/30 pl-2 py-1 bg-black/25 rounded-r-lg"
                                  >
                                    <span class="text-[10px] text-white/45 block mb-0.5"
                                      >Переслано · {m.content.forwarded_from.author_label || "—"}</span
                                    >
                                    {m.content.forwarded_from.text || ""}
                                  </div>
                                {/if}
                                {#if String(m.content?.text ?? "").trim()}
                                  <p class="text-[var(--text)] whitespace-pre-wrap leading-relaxed">
                                    {m.content.text}
                                  </p>
                                {/if}
                              {/if}
                            {:else if m.kind === "voice"}
                              <div class="flex items-center gap-2 min-w-[12rem]">
                                <audio
                                  controls
                                  preload="metadata"
                                  src={m.content?.url}
                                  class="flex-1 min-w-0 min-h-[40px]"
                                  on:loadedmetadata={(e) => handleAudioMeta(e, m.content?.url)}
                                />
                                <span
                                  class="text-[10px] font-mono tabular-nums text-white/70 shrink-0 min-w-[2.75rem] text-right"
                                  title="Длительность"
                                >
                                  {audioDurations[m.content?.url]
                                    ? formatAudioDuration(audioDurations[m.content?.url])
                                    : "…"}
                                </span>
                              </div>
                            {:else if m.kind === "image" || m.kind === "gif"}
                              <button
                                type="button"
                                class="block overflow-hidden rounded-xl ring-1 ring-white/10 hover:ring-jm-accent/40 transition-shadow"
                                on:click={() => openMediaLightbox("image", m.content?.url)}
                                title="Открыть во весь экран"
                              >
                                <img
                                  src={m.content?.url}
                                  alt=""
                                  loading="lazy"
                                  class="max-w-full rounded-xl max-h-80 object-cover block"
                                />
                              </button>
                            {:else if m.kind === "video"}
                              <div class="relative group/vid">
                                <video
                                  src={m.content?.url}
                                  controls
                                  preload="metadata"
                                  playsinline
                                  class="max-w-full max-h-80 rounded-xl ring-1 ring-white/10 block"
                                />
                                <button
                                  type="button"
                                  class="absolute top-2 right-2 rounded-lg bg-black/60 hover:bg-black/80 text-white text-[10px] font-semibold px-2 py-1 opacity-0 group-hover/vid:opacity-100 transition-opacity"
                                  on:click={() => openMediaLightbox("video", m.content?.url)}
                                  title="Открыть во весь экран"
                                >
                                  Во весь экран
                                </button>
                              </div>
                            {:else if m.kind === "call_invite"}
                              <p class="text-jm-accent font-bold flex items-center gap-2">
                                <Phone size={16} class="shrink-0 opacity-90" />
                                Аудиозвонок
                              </p>
                              <p class="text-[10px] text-[var(--text-secondary)] mt-1 leading-snug">
                                {#if mine}
                                  Ожидайте ответа собеседника. При необходимости завершите звонок кнопкой ниже.
                                {:else if m.content?.session_id}
                                  {senderDmLabel(m.sender)} звонит. Примите вызов во всплывающем окне или отклоните.
                                {:else}
                                  {senderDmLabel(m.sender)} предлагает созвониться. Обновите лаунчер для P2P-аудио.
                                {/if}
                              </p>
                            {:else if m.kind === "server_invite"}
                              <p class="text-jm-accent font-bold flex items-center gap-1.5">
                                <Server size={14} class="shrink-0 opacity-80" />
                                {m.content?.title || "Сервер"}
                              </p>
                              <p class="text-xs font-mono text-white/90 mt-1">{m.content?.server_ip || ""}</p>
                              {#if m.content?.instance_label}
                                <p class="text-[10px] text-[var(--text-secondary)] mt-0.5">
                                  Сборка: {m.content.instance_label}
                                </p>
                              {/if}
                              {#if m.content?.game_version}
                                <p class="text-[10px] text-[var(--text-secondary)]">
                                  Версия MC: {m.content.game_version}
                                </p>
                              {/if}
                              {#if !mine}
                                <button
                                  type="button"
                                  disabled={acceptBusy}
                                  class="mt-2 w-full min-h-[40px] rounded-xl bg-amber-500/20 hover:bg-amber-500/35 border border-amber-500/40 text-amber-100 text-xs font-bold transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
                                  on:click={() => acceptServerInviteFromMessage(m.content)}
                                >
                                  {#if acceptBusy}
                                    <Loader2 size={16} class="animate-spin" />
                                    Установка…
                                  {:else}
                                    Принять и зайти
                                  {/if}
                                </button>
                              {/if}
                            {:else if m.kind === "friend_invite"}
                              <p class="text-[var(--text-secondary)] opacity-70">Дружба</p>
                            {:else}
                              <pre class="text-[10px] opacity-50 overflow-x-auto">{JSON.stringify(
                                  m.content
                                )}</pre>
                            {/if}
                            </div>
                            {#if m.kind !== "deleted"}
                              <button
                                type="button"
                                data-msg-ctx-trigger
                                class="absolute -top-1 {mine ? 'right-0' : 'left-0'} p-1 rounded-lg text-white/50
                                  hover:text-white hover:bg-white/10 opacity-0 group-hover:opacity-100
                                  focus:opacity-100 transition-opacity z-10"
                                title="Меню"
                                aria-label="Меню сообщения"
                                on:click={(e) => openMessageMenuFromKebab(e, m.id, mine)}
                              >
                                <MoreVertical size={16} />
                              </button>
                            {/if}
                          </div>
                        </div>
                      </div>
                    {/each}
                  </div>
                  {#if voiceCallHangup}
                    {#if isDirectDm && activeConversation?.direct_peer?.user_id && me?.id}
                      <div
                        class="shrink-0 flex justify-center items-end gap-5 px-4 pt-2 pb-1 pointer-events-none"
                        transition:fade={{ duration: 140 }}
                      >
                        <div class="flex flex-col items-center gap-1">
                          <div
                            class="rounded-full transition-shadow duration-75 {voiceLocalLevel > 0.06
                              ? 'shadow-[0_0_0_3px_var(--jm-accent)]'
                              : 'shadow-[0_0_0_2px_rgba(255,255,255,0.12)]'}"
                          >
                            {#if avatarSrc(me.avatar_url)}
                              <img
                                src={avatarSrc(me.avatar_url)}
                                alt=""
                                class="w-11 h-11 rounded-full object-cover"
                              />
                            {:else}
                              <div class="w-11 h-11 rounded-full bg-jm-accent/25" />
                            {/if}
                          </div>
                          <span class="text-[9px] text-white/45">Вы</span>
                        </div>
                        <div class="flex flex-col items-center gap-1">
                          <div
                            class="rounded-full transition-shadow duration-75 {voiceRemoteLevel > 0.06
                              ? 'shadow-[0_0_0_3px_var(--jm-accent)]'
                              : 'shadow-[0_0_0_2px_rgba(255,255,255,0.12)]'}"
                          >
                            {#if avatarSrc(activeConversation.direct_peer.avatar_url)}
                              <img
                                src={avatarSrc(activeConversation.direct_peer.avatar_url)}
                                alt=""
                                class="w-11 h-11 rounded-full object-cover"
                              />
                            {:else}
                              <div class="w-11 h-11 rounded-full bg-white/15" />
                            {/if}
                          </div>
                          <span class="text-[9px] text-white/45 truncate max-w-[5.5rem]"
                            >{directRowTitle(activeConversation) || "Собеседник"}</span
                          >
                        </div>
                      </div>
                    {/if}
                    <div
                      class="shrink-0 flex justify-center px-2 pb-2.5 pt-1 relative pointer-events-none"
                      transition:fade={{ duration: 140 }}
                    >
                      <div
                        class="pointer-events-auto flex items-stretch gap-0.5 rounded-xl bg-[#2b2d31] border border-black/50 shadow-[0_10px_28px_rgba(0,0,0,0.55)] p-1.5 max-w-[min(96vw,24rem)]"
                        data-voice-bar-pop
                      >
                        <div class="flex items-center rounded-lg bg-[#1e1f22] overflow-hidden border border-black/30">
                          <button
                            type="button"
                            class="flex items-center justify-center w-11 h-11 rounded-l-lg hover:bg-white/[0.08] transition-colors"
                            title={voiceMicMuted ? "Включить микрофон" : "Выключить микрофон"}
                            aria-label="Микрофон"
                            on:click={toggleVoiceMic}
                          >
                            {#if voiceMicMuted}
                              <MicOff size={22} class="text-[#ed4245]" strokeWidth={2} />
                            {:else}
                              <Mic size={22} class="text-[#dbdee1]" strokeWidth={2} />
                            {/if}
                          </button>
                          <button
                            type="button"
                            class="w-8 h-11 flex items-center justify-center border-l border-white/[0.06] hover:bg-white/[0.08] text-[#b5bac1] transition-colors"
                            title="Микрофон для следующего звонка"
                            aria-label="Устройство ввода"
                            data-voice-bar-pop
                            on:click={() => {
                              voiceBarMicMenuOpen = !voiceBarMicMenuOpen;
                              voiceBarSpeakerMenuOpen = false;
                              void refreshMics();
                            }}
                          >
                            <ChevronDown size={17} />
                          </button>
                        </div>
                        <div class="flex items-center rounded-lg bg-[#1e1f22] overflow-hidden border border-black/30 shrink-0">
                          <button
                            type="button"
                            class="flex items-center justify-center w-11 h-11 rounded-lg hover:bg-white/[0.08] transition-colors text-[#dbdee1]"
                            title="Динамики (куда играет собеседник)"
                            aria-label="Динамики"
                            data-voice-bar-pop
                            on:click={() => {
                              voiceBarSpeakerMenuOpen = !voiceBarSpeakerMenuOpen;
                              voiceBarMicMenuOpen = false;
                              void refreshAudioDevices();
                            }}
                          >
                            <Volume2 size={20} strokeWidth={2} />
                          </button>
                        </div>
                        <div class="w-px bg-black/60 self-stretch my-1.5 mx-0.5 shrink-0"></div>
                        <div class="flex flex-col justify-center px-2 min-w-0 flex-1">
                          <span class="text-[11px] font-semibold text-[#f2f3f5] leading-tight truncate"
                            >Голосовой канал</span
                          >
                          <span class="text-[10px] text-[#949ba4] leading-tight truncate"
                            >{isDirectDm ? directRowTitle(activeConversation) || "Личный чат" : "Чат"}</span
                          >
                        </div>
                        <div class="w-px bg-black/60 self-stretch my-1.5 mx-0.5 shrink-0"></div>
                        <button
                          type="button"
                          class="flex items-center justify-center min-w-[52px] h-11 px-2.5 rounded-lg bg-[#ed4245] hover:bg-[#c93e41] active:bg-[#a63235] transition-colors"
                          title="Завершить звонок"
                          aria-label="Завершить звонок"
                          on:click={() => void endVoiceCall()}
                        >
                          <PhoneOff size={22} class="text-white" strokeWidth={2.25} />
                        </button>
                      </div>
                      {#if voiceBarMicMenuOpen}
                        <div
                          data-voice-bar-pop
                          class="pointer-events-auto absolute bottom-full left-1/2 -translate-x-1/2 mb-2 w-[min(18rem,calc(100vw-2rem))] max-h-44 overflow-y-auto custom-scrollbar rounded-lg border border-[#1e1f22] bg-[#111214] shadow-2xl py-1 z-[80] text-xs text-[#dbdee1]"
                          role="listbox"
                        >
                          <p class="px-3 py-1.5 text-[10px] uppercase tracking-wide text-[#949ba4]">
                            Устройство (следующий звонок)
                          </p>
                          <button
                            type="button"
                            class="w-full text-left px-3 py-2 hover:bg-[#2b2d31] transition-colors"
                            on:click={() => {
                              selectedMic = "";
                              voiceBarMicMenuOpen = false;
                            }}>По умолчанию</button
                          >
                          {#each audioInputs as d (d.deviceId)}
                            <button
                              type="button"
                              class="w-full text-left px-3 py-2 hover:bg-[#2b2d31] transition-colors truncate"
                              title={d.label || d.deviceId}
                              on:click={() => {
                                selectedMic = d.deviceId;
                                voiceBarMicMenuOpen = false;
                              }}>{d.label || d.deviceId.slice(0, 14) + "…"}</button
                            >
                          {/each}
                        </div>
                      {/if}
                      {#if voiceBarSpeakerMenuOpen}
                        <div
                          data-voice-bar-pop
                          class="pointer-events-auto absolute bottom-full left-1/2 -translate-x-1/2 mb-2 w-[min(18rem,calc(100vw-2rem))] max-h-44 overflow-y-auto custom-scrollbar rounded-lg border border-[#1e1f22] bg-[#111214] shadow-2xl py-1 z-[80] text-xs text-[#dbdee1]"
                          role="listbox"
                        >
                          <p class="px-3 py-1.5 text-[10px] uppercase tracking-wide text-[#949ba4]">
                            Выход звука (звонок)
                          </p>
                          <button
                            type="button"
                            class="w-full text-left px-3 py-2 hover:bg-[#2b2d31] transition-colors"
                            on:click={() => {
                              selectedSpeaker = "";
                              voiceBarSpeakerMenuOpen = false;
                              void applyVoiceOutputToCall();
                            }}>По умолчанию</button
                          >
                          {#each audioOutputs as d (d.deviceId)}
                            <button
                              type="button"
                              class="w-full text-left px-3 py-2 hover:bg-[#2b2d31] transition-colors truncate"
                              title={d.label || d.deviceId}
                              on:click={() => {
                                selectedSpeaker = d.deviceId;
                                voiceBarSpeakerMenuOpen = false;
                                void applyVoiceOutputToCall();
                              }}>{d.label || d.deviceId.slice(0, 14) + "…"}</button
                            >
                          {/each}
                        </div>
                      {/if}
                    </div>
                  {/if}
                  <div
                    class="border-t border-white/10 p-3 gap-3 shrink-0 bg-black/25 flex flex-col"
                    transition:fade={{ duration: 150 }}
                  >
                    {#if replyTo}
                      <div
                        class="flex items-start gap-2 px-3 py-2 rounded-xl bg-jm-accent/12 border border-jm-accent/25 text-xs text-[var(--text)]"
                      >
                        <CornerUpLeft size={16} class="shrink-0 mt-0.5 text-jm-accent" />
                        <div class="min-w-0 flex-1">
                          <span class="font-bold text-jm-accent-light block truncate"
                            >Ответ {replyTo.label}</span
                          >
                          <span class="text-white/55">{replyTo.preview}</span>
                        </div>
                        <button
                          type="button"
                          class="p-1 rounded-lg hover:bg-white/10 shrink-0"
                          aria-label="Отменить ответ"
                          on:click={clearReply}
                        >
                          <X size={16} />
                        </button>
                      </div>
                    {/if}
                    {#if forwardFrom}
                      <div
                        class="flex items-start gap-2 px-3 py-2 rounded-xl bg-white/8 border border-white/15 text-xs"
                      >
                        <Share2 size={16} class="shrink-0 mt-0.5 text-white/60" />
                        <div class="min-w-0 flex-1">
                          <span class="font-bold block truncate text-white/80"
                            >Пересылка · {forwardFrom.label}</span
                          >
                          <span class="text-white/45">{textSnippet(forwardFrom.text, 200)}</span>
                        </div>
                        <button
                          type="button"
                          class="p-1 rounded-lg hover:bg-white/10 shrink-0"
                          aria-label="Отменить пересылку"
                          on:click={clearForward}
                        >
                          <X size={16} />
                        </button>
                      </div>
                    {/if}
                    <div class="flex flex-wrap gap-2 items-stretch">
                      <div
                        class="relative shrink-0 min-w-[12rem] flex-1 sm:flex-none sm:max-w-[18rem]"
                        bind:this={micMenuEl}
                      >
                        <button
                          type="button"
                          class="w-full min-h-[44px] h-full px-3 py-2.5 rounded-xl bg-black/45 border border-white/15 text-left flex items-center justify-between gap-2 hover:border-jm-accent/40 transition-colors text-sm font-medium"
                          on:click={() => {
                            micMenuOpen = !micMenuOpen;
                            void refreshMics();
                          }}
                        >
                          <span class="truncate">{micLabel}</span>
                          <ChevronDown size={18} class="shrink-0 opacity-70" />
                        </button>
                        {#if micMenuOpen}
                          <div
                            class="absolute bottom-full left-0 right-0 mb-1.5 z-50 max-h-52 overflow-y-auto custom-scrollbar rounded-xl border border-jm-accent/35 bg-[var(--input-bg)] shadow-xl py-1"
                            role="listbox"
                          >
                            <button
                              type="button"
                              class="w-full text-left px-3 py-2.5 text-sm hover:bg-white/10 transition-colors"
                              on:click={() => {
                                selectedMic = "";
                                micMenuOpen = false;
                              }}>Системный по умолчанию</button
                            >
                            {#each audioInputs as d (d.deviceId)}
                              <button
                                type="button"
                                class="w-full text-left px-3 py-2.5 text-sm hover:bg-white/10 transition-colors truncate"
                                title={d.label || d.deviceId}
                                on:click={() => {
                                  selectedMic = d.deviceId;
                                  micMenuOpen = false;
                                }}>{d.label || d.deviceId.slice(0, 10) + "…"}</button
                              >
                            {/each}
                          </div>
                        {/if}
                      </div>
                      <button
                        type="button"
                        on:click={refreshMics}
                        class="min-h-[44px] min-w-[44px] px-3 rounded-xl bg-white/10 transition-colors hover:bg-white/15 jm-tap-scale flex items-center justify-center shrink-0"
                        title="Обновить устройства"
                      >
                        <RefreshCw size={18} />
                      </button>
                      <button
                        type="button"
                        disabled={callBusy || !isDirectDm}
                        on:click={() => void sendCallInvite()}
                        class="min-h-[44px] min-w-[44px] px-3 rounded-xl bg-jm-accent/20 border border-jm-accent/40 text-jm-accent-light transition-colors hover:bg-jm-accent/30 jm-tap-scale flex items-center justify-center shrink-0 disabled:opacity-50"
                        title={isDirectDm
                          ? "Аудиозвонок (P2P, как в Discord)"
                          : "Только в личных чатах"}
                      >
                        {#if callBusy}
                          <Loader2 size={18} class="animate-spin" />
                        {:else}
                          <Phone size={18} />
                        {/if}
                      </button>
                      <button
                        type="button"
                        on:click={toggleRecord}
                        class="min-h-[44px] min-w-[44px] px-3 rounded-xl transition-all duration-200 jm-tap-scale flex items-center justify-center shrink-0 {recording
                          ? 'bg-red-500/40 text-red-100'
                          : 'bg-jm-accent/25 text-jm-accent'}"
                        title="Запись"
                      >
                        {#if recording}
                          <Square size={20} />
                        {:else}
                          <Mic size={20} />
                        {/if}
                      </button>
                      <label
                        class="min-h-[44px] min-w-[44px] px-3 rounded-xl bg-white/10 cursor-pointer transition-colors hover:bg-white/15 jm-tap-scale flex items-center justify-center shrink-0"
                        title="Файл"
                      >
                        <ImagePlus size={20} />
                        <input
                          type="file"
                          accept="image/*,video/*,.gif"
                          class="hidden"
                          on:change={onPickMedia}
                        />
                      </label>
                      <button
                        type="button"
                        class="min-h-[44px] px-4 rounded-xl bg-amber-500/15 text-amber-200 transition-colors hover:bg-amber-500/25 jm-tap-scale text-xs font-bold shrink-0"
                        title="Приглашение на сервер"
                        on:click={openServerInviteModal}
                      >
                        Сервер
                      </button>
                    </div>
                    <div class="flex gap-2.5 items-stretch">
                      <input
                        type="text"
                        bind:value={msgText}
                        placeholder="Сообщение · Ctrl+V — фото или видео"
                        class="flex-1 min-h-[44px] rounded-xl px-3 py-2.5 text-sm border border-[var(--border)] bg-[var(--input-bg)]"
                        on:keydown={(e) => e.key === "Enter" && sendText()}
                        on:paste={onMsgPaste}
                      />
                      <button
                        type="button"
                        on:click={sendText}
                        disabled={msgBusy}
                        class="min-h-[44px] min-w-[44px] px-4 rounded-xl bg-jm-accent text-black disabled:opacity-50 transition-transform active:scale-95 jm-tap-scale flex items-center justify-center shrink-0"
                      >
                        <Send size={20} />
                      </button>
                    </div>
                    <div class="flex gap-2.5 items-stretch">
                      <input
                        type="text"
                        bind:value={inviteHandleInput}
                        placeholder="invite handle"
                        class="flex-1 min-h-[40px] rounded-xl px-3 py-2 text-xs border border-white/10 bg-black/35"
                      />
                      <button
                        type="button"
                        class="min-h-[40px] px-4 rounded-xl bg-white/10 text-xs font-bold transition-colors hover:bg-white/15 jm-tap-scale shrink-0"
                        on:click={() => {
                          void sendFriendInviteInChat(inviteHandleInput);
                          inviteHandleInput = "";
                        }}
                      >
                        →
                      </button>
                    </div>
                  </div>
                {:else}
                  <p class="p-8 text-sm text-[var(--text-secondary)] text-center opacity-50 flex-1 flex items-center justify-center">
                    Выберите чат
                  </p>
                {/if}
              </div>
            </div>
          </div>
        {:else if tab === "site"}
          <div
            class="ui-pane ui-pane-soft flex flex-col flex-1 min-h-0 overflow-hidden"
          >
            <div
              class="flex items-center gap-2 px-4 py-3 border-b border-[var(--border)] shrink-0 bg-[var(--surface-1)]"
            >
              <Globe size={18} class="text-jm-accent shrink-0" />
              <span class="text-xs font-mono truncate flex-1 min-w-0 text-[var(--text-secondary)]">{siteWebUrl()}</span>
              <button
                type="button"
                class="ui-btn ui-btn-subtle ui-btn-icon shrink-0"
                title="Открыть в браузере"
                aria-label="Открыть в браузере"
                on:click={() => void openUrl(siteWebUrl())}
              >
                <ExternalLink size={18} />
              </button>
            </div>
            <iframe
              title="JentleMemes — сайт"
              src={siteWebUrl()}
              class="flex-1 w-full min-h-[260px] border-0 bg-[#111]"
              sandbox="allow-same-origin allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox allow-downloads"
            ></iframe>
            <p class="ui-hint px-4 py-3 border-t border-[var(--border)] shrink-0 leading-snug">
              Некоторые страницы запрещают фрейм — тогда откройте сайт кнопкой выше.
            </p>
          </div>
        {/if}
      </div>
    {/key}
  {/if}

  {#if serverInviteOpen && me}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="fixed inset-0 z-[120] bg-black/75 backdrop-blur-sm flex items-center justify-center p-4"
      transition:fade={{ duration: 180 }}
      on:click={() => !invExportBusy && (serverInviteOpen = false)}
      role="presentation"
    >
      <div
        class="bg-jm-card border border-jm-accent/30 rounded-2xl w-full max-w-lg max-h-[90vh] overflow-y-auto custom-scrollbar shadow-2xl p-6"
        transition:scale={{ duration: 260, start: 0.96, easing: quintOut }}
        on:click|stopPropagation
        role="dialog"
        aria-modal="true"
      >
        <div class="flex items-center justify-between gap-2 mb-4">
          <h3 class="text-lg font-bold text-white flex items-center gap-2">
            <Server size={20} class="text-jm-accent" />
            Приглашение на сервер
          </h3>
          <button
            type="button"
            disabled={invExportBusy}
            class="p-2 rounded-lg text-[var(--text-secondary)] hover:bg-white/10 disabled:opacity-40"
            on:click={() => (serverInviteOpen = false)}
            aria-label="Закрыть"
          >
            <X size={20} />
          </button>
        </div>

        <div class="space-y-4">
          <div>
            <label class="text-xs text-[var(--text-secondary)] mb-1 block" for="jm-inv-server">Адрес сервера</label>
            <input
              id="jm-inv-server"
              bind:value={invServerIp}
              placeholder="play.example.com или 127.0.0.1:25565"
              class="w-full rounded-xl px-3 py-2.5 text-sm border border-[var(--border)] bg-[var(--input-bg)]"
            />
          </div>
          <div>
            <label class="text-xs text-[var(--text-secondary)] mb-1 block" for="jm-inv-title">Название приглашения</label>
            <input
              id="jm-inv-title"
              bind:value={invTitle}
              placeholder="Например, наш ванильный сервер"
              class="w-full rounded-xl px-3 py-2.5 text-sm border border-[var(--border)] bg-[var(--input-bg)]"
            />
          </div>

          <div class="flex gap-2">
            <button
              type="button"
              class="flex-1 py-2 rounded-xl text-xs font-bold border transition-colors {invMode === 'instance'
                ? 'bg-jm-accent/20 border-jm-accent text-jm-accent-light'
                : 'bg-black/30 border-white/10 text-[var(--text-secondary)]'}"
              on:click={() => (invMode = 'instance')}
            >
              Сборка из лаунчера
            </button>
            <button
              type="button"
              class="flex-1 py-2 rounded-xl text-xs font-bold border transition-colors {invMode === 'version_only'
                ? 'bg-jm-accent/20 border-jm-accent text-jm-accent-light'
                : 'bg-black/30 border-white/10 text-[var(--text-secondary)]'}"
              on:click={() => (invMode = 'version_only')}
            >
              Только версия MC
            </button>
          </div>

          {#if invMode === "instance"}
            <div>
              <label class="text-xs text-[var(--text-secondary)] mb-1 block" for="jm-inv-inst">Сборка</label>
              <select
                id="jm-inv-inst"
                bind:value={invInstanceId}
                on:change={() => void loadInvFoldersFor(invInstanceId)}
                class="w-full rounded-xl px-3 py-2.5 text-sm border border-[var(--border)] bg-[var(--input-bg)]"
              >
                {#each myInstances as inst (inst.id)}
                  <option value={inst.id}>{inst.name} · {inst.game_version}</option>
                {:else}
                  <option value="">Нет сборок</option>
                {/each}
              </select>
            </div>
            <div>
              <div class="flex items-center justify-between mb-2">
                <span class="text-xs text-[var(--text-secondary)]">Папки в .jentlepack</span>
                <div class="flex gap-2 text-[10px]">
                  <button type="button" class="text-jm-accent font-bold" on:click={() => toggleInvAll(true)}
                    >Все</button
                  >
                  <button
                    type="button"
                    class="text-[var(--text-secondary)] font-bold"
                    on:click={() => toggleInvAll(false)}>Нет</button
                  >
                </div>
              </div>
              <div class="grid grid-cols-2 gap-2">
                {#each exportCommonFolders.filter((k) => k in invFolders) as key (key)}
                  <label
                    class="flex items-center gap-2 cursor-pointer p-2.5 bg-black/30 rounded-xl border border-white/5"
                  >
                    <input
                      type="checkbox"
                      checked={invFolders[key]}
                      on:change={() => toggleInvFolder(key)}
                      class="w-4 h-4 accent-jm-accent"
                    />
                    <span class="text-xs font-bold text-white">{exportFolderLabels[key] || key}</span>
                  </label>
                {/each}
              </div>
              {#if invAdvancedFolders.length > 0}
                <button
                  type="button"
                  class="text-xs text-jm-accent font-bold mt-2"
                  on:click={() => (invShowAdvanced = !invShowAdvanced)}
                >
                  {invShowAdvanced ? "Скрыть" : "Показать"} расширенные ({invAdvancedFolders.length})
                </button>
                {#if invShowAdvanced}
                  <div class="grid grid-cols-2 gap-2 mt-2">
                    {#each invAdvancedFolders as key (key)}
                      <label class="flex items-center gap-2 cursor-pointer p-2 bg-black/20 rounded-lg border border-white/5">
                        <input
                          type="checkbox"
                          checked={invFolders[key]}
                          on:change={() => toggleInvFolder(key)}
                          class="w-3.5 h-3.5 accent-jm-accent"
                        />
                        <span class="text-[11px] font-bold text-white">{exportFolderLabels[key] || key}</span>
                      </label>
                    {/each}
                  </div>
                {/if}
              {/if}
            </div>
          {:else}
            <div>
              <label class="text-xs text-[var(--text-secondary)] mb-1 block" for="jm-inv-gv">Версия Minecraft</label>
              <input
                id="jm-inv-gv"
                bind:value={invGameVersion}
                placeholder="1.20.1"
                class="w-full rounded-xl px-3 py-2.5 text-sm border border-[var(--border)] bg-[var(--input-bg)]"
              />
              <p class="text-[10px] text-[var(--text-secondary)] mt-1">
                Без экспорта .jentlepack — другой игрок создаст сборку сам по версии.
              </p>
            </div>
          {/if}

          {#if err}
            <p class="text-xs text-red-400">{err}</p>
          {/if}

          <div class="flex gap-2 pt-2">
            <button
              type="button"
              disabled={invExportBusy}
              class="flex-1 py-3 rounded-xl bg-white/10 text-sm font-bold hover:bg-white/15 disabled:opacity-50"
              on:click={() => (serverInviteOpen = false)}
            >
              Отмена
            </button>
            <button
              type="button"
              disabled={invExportBusy}
              class="flex-1 py-3 rounded-xl bg-jm-accent text-black text-sm font-bold hover:bg-jm-accent-light disabled:opacity-50 flex items-center justify-center gap-2"
              on:click={() => void submitServerInviteModal()}
            >
              {#if invExportBusy}
                <Loader2 size={18} class="animate-spin" />
                Экспорт…
              {:else}
                Отправить
              {/if}
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  {#if msgMenu}
    {@const mid = msgMenu.messageId}
    {@const mm = messages.find((x) => x.id === mid)}
    {#if mm}
      {@const menuMine =
        me?.id != null &&
        String(mm.sender_id ?? "").toLowerCase() === String(me.id).toLowerCase()}
      <div
        data-msg-rctx
        role="menu"
        class="fixed z-[300] w-56 rounded-lg border border-[#1e1f22] bg-[#111214] shadow-[0_16px_48px_rgba(0,0,0,0.65)] py-1 text-sm text-[#dbdee1]"
        style="left: {msgMenu.x}px; top: {msgMenu.y}px;"
        on:click|stopPropagation
        transition:scale={{ duration: 100, easing: quintOut, start: 0.97 }}
      >
        {#if mm.kind === "text"}
          <button
            type="button"
            class="w-full flex items-center gap-2.5 px-3 py-2.5 text-left hover:bg-[#2b2d31] transition-colors rounded-none"
            on:click={() => beginReply(mm)}
          >
            <CornerUpLeft size={16} class="shrink-0 text-[#b5bac1]" />
            Ответить
          </button>
        {/if}
        {#if mm.kind === "text" && String(mm.content?.text ?? "").trim()}
          <button
            type="button"
            class="w-full flex items-center gap-2.5 px-3 py-2.5 text-left hover:bg-[#2b2d31] transition-colors rounded-none"
            on:click={() => beginForward(mm)}
          >
            <Share2 size={16} class="shrink-0 text-[#b5bac1]" />
            Переслать
          </button>
        {/if}
        {#if String(mm.sender?.handle ?? "")
          .replace(/^@+/g, "")
          .trim()}
          <button
            type="button"
            class="w-full flex items-center gap-2.5 px-3 py-2.5 text-left hover:bg-[#2b2d31] transition-colors rounded-none"
            on:click={() => insertPingFromMessage(mm)}
          >
            <AtSign size={16} class="shrink-0 text-[#b5bac1]" />
            Упомянуть
          </button>
        {/if}
        {#if mm.kind === "text" && String(mm.content?.text ?? "").length > 0}
          <button
            type="button"
            class="w-full flex items-center gap-2.5 px-3 py-2.5 text-left hover:bg-[#2b2d31] transition-colors rounded-none"
            on:click={() => void copyMessageText(String(mm.content?.text ?? ""))}
          >
            <Copy size={16} class="shrink-0 text-[#b5bac1]" />
            Копировать текст
          </button>
        {/if}
        {#if mm.kind === "text" && menuMine}
          <button
            type="button"
            class="w-full flex items-center gap-2.5 px-3 py-2.5 text-left hover:bg-[#2b2d31] transition-colors rounded-none"
            on:click={() => beginEditMessage(mm)}
          >
            <Pencil size={16} class="shrink-0 text-[#b5bac1]" />
            Редактировать
          </button>
        {/if}
        {#if mm.kind === "text"}
          <div class="h-px bg-[#1e1f22] my-1 mx-2"></div>
        {/if}
        <button
          type="button"
          class="w-full flex items-center gap-2.5 px-3 py-2.5 text-left hover:bg-[#2b2d31] transition-colors rounded-none"
          on:click={() => void deleteChatMessage(mm.id, "me")}
        >
          Удалить у себя
        </button>
        {#if menuMine}
          <button
            type="button"
            class="w-full flex items-center gap-2.5 px-3 py-2.5 text-left hover:bg-[#2b2d31] text-[#ed4245] transition-colors rounded-none"
            on:click={() => void deleteChatMessage(mm.id, "all")}
          >
            Удалить у всех
          </button>
        {/if}
      </div>
    {/if}
  {/if}

  {#if profileModal}
    <div
      class="fixed inset-0 z-[280] flex items-center justify-center bg-black/75 p-4"
      role="dialog"
      aria-modal="true"
      transition:fade={{ duration: 120 }}
      on:click={(e) => e.target === e.currentTarget && closeProfileModal()}
    >
      {#if profileModalBusy && !profileModalData}
        <div
          class="rounded-xl bg-[#2b2d31] p-8 border border-[#1e1f22] flex items-center gap-3 text-[#dbdee1]"
        >
          <Loader2 class="animate-spin shrink-0" size={22} />
          Профиль…
        </div>
      {:else if profileModalData}
        <div
          class="w-full max-w-[22rem] rounded-2xl overflow-hidden border border-[#1e1f22] bg-[#111214] shadow-[0_24px_64px_rgba(0,0,0,0.75)] max-h-[min(90vh,560px)] flex flex-col overflow-y-auto custom-scrollbar"
          transition:scale={{ duration: 160, easing: quintOut, start: 0.97 }}
          on:click|stopPropagation
        >
          {#if resolveSiteMediaUrl(apiBaseUrl, profileModalData.profile_banner_url)}
            <div
              class="h-28 w-full bg-cover bg-center shrink-0"
              style:background-image={`url('${String(resolveSiteMediaUrl(apiBaseUrl, profileModalData.profile_banner_url)).replace(/'/g, "%27")}')`}
            />
          {:else}
            <div
              class="h-16 w-full shrink-0 bg-gradient-to-br from-indigo-950/90 via-[#1e1f22] to-purple-950/70"
            />
          {/if}
          <div class="px-4 pb-4 pt-0 -mt-9 flex flex-col gap-2 relative">
            <div class="flex gap-3 items-end">
              {#if avatarSrc(profileModalData.avatar_url)}
                <img
                  src={avatarSrc(profileModalData.avatar_url)}
                  alt=""
                  class="w-[4.5rem] h-[4.5rem] rounded-full object-cover ring-4 ring-[#111214] shrink-0"
                />
              {:else}
                <div class="w-[4.5rem] h-[4.5rem] rounded-full bg-white/10 ring-4 ring-[#111214] shrink-0" />
              {/if}
              <div class="min-w-0 flex-1 pb-1">
                <h2 class="text-lg font-bold text-[#f2f3f5] truncate">
                  {profileModalData.chat_primary_line || profileModalData.display_name || "—"}
                </h2>
                {#if profileModalData.handle}
                  <p class="text-xs text-[#949ba4] truncate">
                    @{String(profileModalData.handle).replace(/^@+/, "")}
                  </p>
                {/if}
              </div>
            </div>
            {#if profileModalData.profile_role_label}
              <span
                class="inline-flex self-start px-2 py-0.5 rounded-md text-[11px] font-semibold bg-pink-500/20 text-pink-200 border border-pink-500/35"
                >{profileModalData.profile_role_label}</span
              >
            {/if}
            <p class="text-xs text-[#949ba4]">{formatPresence(profileModalData.last_active_at)}</p>
            {#if profileModalData.profile_bio}
              <p class="text-[12px] text-[#b5bac1] leading-snug whitespace-pre-wrap border-l-2 border-white/10 pl-2.5">
                {profileModalData.profile_bio}
              </p>
            {/if}
            {#if profileModalData.public_id}
              <p class="text-[10px] text-[#949ba4] font-mono break-all opacity-90">
                Публичный ID: {profileModalData.public_id}
              </p>
            {/if}
            {#if profileModal.isSelf}
              <p class="text-[10px] text-[#949ba4] leading-snug">
                Ник, баннер, описание и сервер Minecraft редактируются во вкладке «Профиль».
              </p>
            {/if}
            {#if chatProfileMcServer}
              {@const mcHost = String(profileModalData.minecraft_server_host || "").trim()}
              {#if mcHost}
                <div class="rounded-xl border border-[#1e1f22] bg-[#2b2d31] p-3 text-xs text-[#dbdee1] space-y-1">
                  <p class="font-bold text-[#f2f3f5] flex items-center gap-1.5">
                    <Server size={14} class="text-jm-accent shrink-0" />
                    Minecraft
                  </p>
                  <p class="font-mono text-[11px] break-all opacity-90">{mcHost}</p>
                  {#if mcProbe.loading}
                    <p class="text-[#949ba4] flex items-center gap-1">
                      <Loader2 size={12} class="animate-spin shrink-0" /> Запрос статуса…
                    </p>
                  {:else if mcProbe.err}
                    <p class="text-red-300/90">{mcProbe.err}</p>
                  {:else if mcProbe.json}
                    {@const j = mcProbe.json}
                    <p class="text-[#b5bac1]">
                      {#if j.online}
                        Онлайн · {j.players?.online ?? "?"} / {j.players?.max ?? "?"}
                      {:else}
                        Офлайн
                      {/if}
                    </p>
                    {#if j.version}
                      <p class="text-[10px] text-[#949ba4]">
                        Версия: {typeof j.version === "string"
                          ? j.version
                          : j.version?.name || JSON.stringify(j.version)}
                      </p>
                    {/if}
                    {#if j.motd && (j.motd.chat != null || (Array.isArray(j.motd.clean) && j.motd.clean.length))}
                      <div class="mt-1">
                        <ServerMotdBlock
                          motdChat={j.motd.chat}
                          motdLines={Array.isArray(j.motd.clean)
                            ? j.motd.clean
                            : j.motd.clean
                              ? [String(j.motd.clean)]
                              : []}
                          samples={Array.isArray(j.players?.list) ? j.players.list : []}
                          compact={true}
                          showFaces={true}
                          tone="neutral"
                        />
                      </div>
                    {/if}
                  {/if}
                </div>
              {/if}
            {/if}
            <div class="flex flex-col gap-2 pt-2 border-t border-[#1e1f22] mt-1">
              {#if !profileModal.isSelf}
                <button
                  type="button"
                  class="w-full py-2.5 rounded-xl bg-[#248046] hover:bg-[#1a6334] text-white text-sm font-semibold transition-colors"
                  on:click={() => {
                    const id = profileModal?.userId;
                    closeProfileModal();
                    if (id) void openDm(id);
                  }}
                >
                  Написать
                </button>
              {/if}
              <button
                type="button"
                class="w-full flex items-center justify-center gap-2 py-2.5 rounded-xl bg-[#2b2d31] hover:bg-[#35373c] text-[#dbdee1] text-sm transition-colors"
                on:click={() =>
                  void copyPlain(
                    String(profileModalData.public_id || profileModalData.id || ""),
                    "ID скопирован",
                  )}
              >
                <Copy size={16} />
                Копировать публичный ID
              </button>
              <button
                type="button"
                class="w-full py-2.5 rounded-xl text-sm text-[#949ba4] hover:text-[#dbdee1] hover:bg-[#2b2d31] transition-colors"
                on:click={closeProfileModal}
              >
                Закрыть
              </button>
            </div>
          </div>
        </div>
      {/if}
    </div>
  {/if}

  {#if me && incomingCallPrompt}
    <div
      class="fixed inset-0 z-[220] flex items-center justify-center bg-black/70 p-4"
      transition:fade={{ duration: 140 }}
      role="dialog"
      aria-modal="true"
      aria-labelledby="jm-incoming-call-title"
    >
      <div
        class="rounded-xl border border-[#1e1f22] bg-[#2b2d31] p-6 max-w-sm w-full shadow-[0_24px_64px_rgba(0,0,0,0.75)]"
        transition:scale={{ duration: 180, easing: quintOut, start: 0.96 }}
      >
        <p id="jm-incoming-call-title" class="font-bold text-[#f2f3f5] text-lg">Входящий звонок</p>
        <p class="text-xs text-[#b5bac1] mt-1.5 leading-relaxed">
          Личный чат · голосовой канал (WebRTC)
        </p>
        <div class="flex gap-2 mt-6">
          <button
            type="button"
            class="flex-1 min-h-[44px] rounded-lg bg-[#248046] hover:bg-[#1a6334] text-white text-sm font-semibold transition-colors"
            on:click={() => void acceptIncomingCall()}
          >
            Принять
          </button>
          <button
            type="button"
            class="flex-1 min-h-[44px] rounded-lg bg-[#1e1f22] hover:bg-[#2b2d31] border border-[#1e1f22] text-[#dbdee1] text-sm font-semibold transition-colors"
            on:click={declineIncomingCall}
          >
            Отклонить
          </button>
        </div>
      </div>
    </div>
  {/if}

  {#if me}
    <audio bind:this={voiceRemoteEl} autoplay playsinline class="sr-only" aria-hidden="true"></audio>
  {/if}

  {#if mediaLightbox}
    <div
      class="fixed inset-0 z-[260] flex items-center justify-center bg-black/90 backdrop-blur-sm p-4"
      role="dialog"
      aria-modal="true"
      aria-label="Просмотр медиа"
      transition:fade={{ duration: 140 }}
      on:click|self={closeMediaLightbox}
      on:keydown={(e) => {
        if (e.key === "Escape") closeMediaLightbox();
      }}
    >
      <button
        type="button"
        class="absolute top-4 right-4 w-10 h-10 rounded-full bg-white/10 hover:bg-white/20 text-white flex items-center justify-center transition-colors z-10"
        aria-label="Закрыть"
        on:click={closeMediaLightbox}
      >
        <X size={22} />
      </button>
      <button
        type="button"
        class="absolute top-4 left-4 rounded-lg bg-white/10 hover:bg-white/20 text-white text-xs font-semibold px-3 py-2 flex items-center gap-2 transition-colors z-10"
        on:click={(e) => {
          e.stopPropagation();
          if (mediaLightbox?.url) void openUrl(mediaLightbox.url);
        }}
        title="Открыть в браузере"
      >
        <ExternalLink size={14} />
        Открыть во внешнем браузере
      </button>
      {#if mediaLightbox.kind === "image"}
        <img
          src={mediaLightbox.url}
          alt=""
          class="max-w-[96vw] max-h-[92vh] object-contain rounded-xl shadow-2xl"
          transition:scale={{ duration: 180, easing: quintOut, start: 0.96 }}
        />
      {:else}
        <video
          src={mediaLightbox.url}
          controls
          autoplay
          playsinline
          class="max-w-[96vw] max-h-[92vh] rounded-xl shadow-2xl bg-black"
        />
      {/if}
    </div>
  {/if}
</div>
