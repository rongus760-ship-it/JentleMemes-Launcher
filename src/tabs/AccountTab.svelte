<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { emit, listen } from "@tauri-apps/api/event";
  import {
    User,
    Mail,
    Key,
    MonitorSmartphone,
    Trash2,
    CheckCircle2,
    Plus,
    UserPlus,
    Copy,
    ExternalLink,
    Loader2,
  } from "lucide-svelte";
  import { fade, fly } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import SkinHeadAvatar from "../components/SkinHeadAvatar.svelte";
  import Card from "../components/ui/Card.svelte";
  import Button from "../components/ui/Button.svelte";

  let profiles: {
    accounts: any[];
    active_account_id: string;
    skin_presets: any[];
  } = { accounts: [], active_account_id: "", skin_presets: [] };

  let authType: "offline" | "elyby" | "microsoft" = "microsoft";
  let status = "";
  let statusKind: "info" | "error" | "success" = "info";
  let offlineName = "";
  let elyEmail = "";
  let elyPass = "";
  let msCodeData: any = null;
  let msMiniPoll: ReturnType<typeof setInterval> | null = null;
  let profilesUnlisten: (() => void) | undefined;
  let avatarByAccountId: Record<string, string> = {};
  let avatarHeadFromTexture: Record<string, boolean> = {};

  let busyAdd = false;

  const authTabs: Array<{ id: "offline" | "elyby" | "microsoft"; label: string }> = [
    { id: "microsoft", label: "Microsoft" },
    { id: "elyby", label: "Ely.by" },
    { id: "offline", label: "Пиратка" },
  ];

  function setStatus(msg: string, kind: "info" | "error" | "success" = "info") {
    status = msg;
    statusKind = kind;
  }

  function accountTypeLabel(t: string): string {
    if (t === "microsoft") return "Microsoft";
    if (t === "elyby") return "Ely.by";
    if (t === "offline") return "Offline";
    return t;
  }

  function inferAccType(acc: any): string {
    if (acc.acc_type) return acc.acc_type;
    if (acc.id?.startsWith("ms-")) return "microsoft";
    if (acc.id?.startsWith("elyby-")) return "elyby";
    if (acc.id?.startsWith("offline-")) return "offline";
    return "offline";
  }

  async function rebuildAvatarUrls() {
    const urls: Record<string, string> = {};
    const headTex: Record<string, boolean> = {};
    for (const acc of profiles.accounts) {
      const uname = encodeURIComponent(String(acc.username || "Steve").trim() || "Steve");
      let fallback = `https://minotar.net/helm/${uname}/48.png`;
      headTex[acc.id] = false;
      if (acc.active_skin_id && profiles.skin_presets) {
        const skin = profiles.skin_presets.find((p: any) => p.id === acc.active_skin_id);
        if (skin?.skin_type === "local" && skin.skin_data) {
          urls[acc.id] = skin.skin_data;
          headTex[acc.id] = true;
          continue;
        }
      }
      const t = inferAccType(acc);
      const uuid = String(acc.uuid || "").replace(/-/g, "");
      if ((t === "elyby" || t === "microsoft") && uuid.length === 32) {
        try {
          const raw: any = await invoke("resolve_session_skin", {
            uuid: acc.uuid,
            accountType: t,
            username: String(acc.username || "").trim(),
          });
          if (raw?.url) {
            urls[acc.id] = String(raw.url);
            headTex[acc.id] = true;
            continue;
          }
        } catch {
          /* helm */
        }
      }
      urls[acc.id] = fallback;
    }
    avatarByAccountId = urls;
    avatarHeadFromTexture = headTex;
  }

  function selectAuthTab(t: "offline" | "elyby" | "microsoft") {
    authType = t;
    setStatus("");
    msCodeData = null;
  }

  onMount(() => {
    void (async () => {
      const data: unknown = await invoke("load_profiles");
      profiles = data as typeof profiles;
      await rebuildAvatarUrls();
    })();
    listen("profiles_updated", async () => {
      const data: unknown = await invoke("load_profiles");
      profiles = data as typeof profiles;
      await rebuildAvatarUrls();
    }).then((u) => (profilesUnlisten = u));
  });

  onDestroy(() => {
    if (msMiniPoll) clearInterval(msMiniPoll);
    profilesUnlisten?.();
  });

  async function saveAndEmit(newProfiles: typeof profiles) {
    profiles = newProfiles;
    await invoke("save_profiles", { profiles: newProfiles });
    await emit("profiles_updated");
  }

  async function handleAddAccount(acc: any) {
    await saveAndEmit({
      ...profiles,
      accounts: [...profiles.accounts, acc],
      active_account_id: acc.id,
    });
  }

  async function handleDelete(id: string) {
    const newAccounts = profiles.accounts.filter((a) => a.id !== id);
    await saveAndEmit({
      ...profiles,
      accounts: newAccounts,
      active_account_id:
        profiles.active_account_id === id
          ? newAccounts.length > 0
            ? newAccounts[0].id
            : ""
          : profiles.active_account_id,
    });
  }

  async function handleSetActive(id: string) {
    await saveAndEmit({ ...profiles, active_account_id: id });
  }

  async function handleOfflineLogin() {
    if (!offlineName.trim()) {
      setStatus("Введите никнейм", "error");
      return;
    }
    busyAdd = true;
    setStatus("Создание оффлайн-аккаунта…");
    try {
      const acc = await invoke("login_offline", { username: offlineName });
      await handleAddAccount(acc);
      setStatus("Аккаунт добавлен", "success");
      offlineName = "";
    } catch (e) {
      setStatus(`Ошибка: ${e}`, "error");
    } finally {
      busyAdd = false;
    }
  }

  async function handleElybyLogin() {
    if (!elyEmail || !elyPass) {
      setStatus("Введите email и пароль", "error");
      return;
    }
    busyAdd = true;
    setStatus("Авторизация в Ely.by…");
    try {
      const acc = await invoke("login_elyby", { email: elyEmail, password: elyPass });
      await handleAddAccount(acc);
      setStatus("Успешный вход через Ely.by", "success");
      elyEmail = "";
      elyPass = "";
    } catch (e) {
      setStatus(`Ошибка: ${e}`, "error");
    } finally {
      busyAdd = false;
    }
  }

  async function handleMicrosoftInit() {
    busyAdd = true;
    setStatus("Связь с серверами Microsoft…");
    try {
      const data: any = await invoke("ms_init_device_code");
      msCodeData = data;
      setStatus("Откройте ссылку и введите код. Ждём подтверждения…");
      const acc = await invoke("ms_login_poll", {
        deviceCode: data.device_code,
        interval: data.interval,
      });
      await handleAddAccount(acc);
      setStatus("Успешный вход через Microsoft", "success");
      msCodeData = null;
    } catch (e) {
      setStatus(`Ошибка: ${e}`, "error");
      msCodeData = null;
    } finally {
      busyAdd = false;
    }
  }

  async function handleMicrosoftMiniBrowser() {
    if (msMiniPoll) {
      setStatus("Вход уже выполняется в мини-окне");
      return;
    }
    busyAdd = true;
    setStatus("Открывается мини-браузер…");
    try {
      // Бэкенд сам создаёт окно WebView (нужен перехват навигации для Microsoft nativeclient-redirect).
      await invoke("ms_oauth_prepare_interactive");
      msMiniPoll = setInterval(async () => {
        try {
          const payload = (await invoke("ms_oauth_try_take_account")) as {
            ok: boolean;
            account?: any;
            error?: string;
          } | null;
          if (payload == null) return;
          if (msMiniPoll) {
            clearInterval(msMiniPoll);
            msMiniPoll = null;
          }
          busyAdd = false;
          if (payload.ok && payload.account) {
            await handleAddAccount(payload.account);
            setStatus("Успешный вход через Microsoft", "success");
          } else {
            setStatus(`Ошибка: ${payload.error ?? "неизвестно"}`, "error");
          }
        } catch {
          /* ignore */
        }
      }, 400);
      setTimeout(() => {
        if (msMiniPoll) {
          clearInterval(msMiniPoll);
          msMiniPoll = null;
          busyAdd = false;
        }
      }, 900_000);
    } catch (e) {
      setStatus(`Ошибка: ${e}`, "error");
      busyAdd = false;
    }
  }

  async function copyMsCode() {
    if (!msCodeData?.user_code) return;
    try {
      await navigator.clipboard.writeText(msCodeData.user_code);
      setStatus("Код скопирован", "success");
    } catch {
      /* ignore */
    }
  }

  function avatarUrl(acc: any): string {
    const uname = encodeURIComponent(String(acc.username || "Steve").trim() || "Steve");
    let url = `https://minotar.net/helm/${uname}/48.png`;
    if (acc.active_skin_id && profiles.skin_presets) {
      const skin = profiles.skin_presets.find((p: any) => p.id === acc.active_skin_id);
      if (skin?.skin_type === "local" && skin.skin_data) {
        url = skin.skin_data;
      }
    }
    return url;
  }

  $: activeAccount = profiles.accounts.find((a) => a.id === profiles.active_account_id);
</script>

<div class="jm-container flex flex-col md:flex-row gap-6 items-start pt-6">
  <!-- Accounts list -->
  <div class="w-full md:w-1/2 flex flex-col gap-4">
    <div class="flex items-baseline justify-between px-1">
      <div>
        <h2 class="text-lg font-semibold">Аккаунты</h2>
        <p class="text-[11px]" style:color="var(--text-secondary)">
          {profiles.accounts.length === 0
            ? "Нет добавленных аккаунтов"
            : `Всего: ${profiles.accounts.length}${activeAccount ? ` · активный: ${activeAccount.username}` : ""}`}
        </p>
      </div>
    </div>

    {#if profiles.accounts.length === 0}
      <Card padding="p-8">
        <div class="flex flex-col items-center gap-3 text-center">
          <div class="w-12 h-12 rounded-full border border-[var(--border)] flex items-center justify-center" style:background="var(--surface-1)">
            <UserPlus size={20} class="text-[var(--text-secondary)]" strokeWidth={2} />
          </div>
          <div>
            <h3 class="text-sm font-semibold">Пока пусто</h3>
            <p class="ui-hint mt-1">Добавьте первый аккаунт справа, чтобы запускать игру.</p>
          </div>
        </div>
      </Card>
    {:else}
      <div class="flex flex-col gap-2">
        {#each profiles.accounts as acc (acc.id)}
          {@const active = profiles.active_account_id === acc.id}
          {@const t = inferAccType(acc)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            on:click={() => handleSetActive(acc.id)}
            class="ui-card ui-card-interactive flex items-center gap-3 p-3 pr-2"
            class:is-active={active}
            style:border-color={active ? "var(--accent)" : undefined}
            style:background={active ? "var(--accent-softer)" : undefined}
          >
            <div class="shrink-0">
              {#if avatarHeadFromTexture[acc.id]}
                <SkinHeadAvatar
                  src={avatarByAccountId[acc.id] ?? avatarUrl(acc)}
                  size={44}
                  alt="Аватар"
                  wrapperClass="rounded-[var(--radius-sm)]"
                />
              {:else}
                <img
                  src={avatarByAccountId[acc.id] ?? avatarUrl(acc)}
                  alt="avatar"
                  class="w-11 h-11 rounded-[var(--radius-sm)] object-cover"
                  style:image-rendering="pixelated"
                />
              {/if}
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-1.5">
                <span class="text-sm font-semibold truncate">{acc.username}</span>
                {#if active}
                  <CheckCircle2 size={13} class="text-[var(--accent-light)] shrink-0" strokeWidth={2.5} />
                {/if}
              </div>
              <div class="flex items-center gap-1.5 mt-0.5">
                <span class="ui-chip" class:ui-chip-accent={active}>
                  {accountTypeLabel(t)}
                </span>
                {#if acc.uuid}
                  <span class="text-[10px] font-mono truncate" style:color="var(--text-secondary)">
                    {String(acc.uuid).slice(0, 8)}
                  </span>
                {/if}
              </div>
            </div>
            <button
              type="button"
              on:click|stopPropagation={() => void handleDelete(acc.id)}
              class="ui-btn ui-btn-ghost ui-btn-icon shrink-0"
              title="Удалить"
              aria-label="Удалить аккаунт"
            >
              <Trash2 size={14} strokeWidth={2.2} />
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Add account panel -->
  <div class="w-full md:w-1/2 flex flex-col gap-4">
    <Card>
      <svelte:fragment slot="header">
        <h3 class="ui-heading flex items-center gap-2">
          <Plus size={16} strokeWidth={2.2} /> Добавить аккаунт
        </h3>
        <p class="ui-hint mt-0.5">Выберите способ входа</p>
      </svelte:fragment>

      <div class="ui-seg w-full grid grid-cols-3 mb-4">
        {#each authTabs as tab (tab.id)}
          <button
            type="button"
            class="ui-seg-item"
            class:is-active={authType === tab.id}
            on:click={() => selectAuthTab(tab.id)}
          >
            {tab.label}
          </button>
        {/each}
      </div>

      {#if authType === "microsoft"}
        <div in:fade={{ duration: 160 }}>
          {#if !msCodeData}
            <div class="flex flex-col gap-3">
              <div class="flex items-start gap-3 p-3 rounded-[var(--radius)]" style:background="var(--surface-1)">
                <MonitorSmartphone size={20} class="shrink-0 mt-0.5 text-[var(--accent-light)]" strokeWidth={2} />
                <div class="min-w-0 flex-1">
                  <p class="text-sm font-medium">Официальный вход</p>
                  <p class="ui-hint mt-0.5">
                    Аккаунт Microsoft / Xbox. Лицензия Minecraft подтверждается на серверах Mojang.
                  </p>
                </div>
              </div>

              <Button variant="primary" size="md" disabled={busyAdd} on:click={() => void handleMicrosoftMiniBrowser()}>
                {#if busyAdd}
                  <Loader2 size={14} class="animate-spin" strokeWidth={2.2} />
                {/if}
                Войти в мини-браузере
              </Button>
              <Button variant="subtle" size="md" disabled={busyAdd} on:click={() => void handleMicrosoftInit()}>
                <ExternalLink size={13} strokeWidth={2.2} /> Получить код (внешний браузер)
              </Button>
              <p class="ui-hint leading-relaxed">
                Если мини-браузер выдаёт ошибку redirect: в Azure → Authentication → добавьте платформу
                «Mobile and desktop» с URI <code class="font-mono">http://127.0.0.1</code> (loopback).
              </p>
            </div>
          {:else}
            <div
              in:fly={{ y: 6, duration: 220, easing: quintOut }}
              class="p-4 rounded-[var(--radius-lg)] border border-[var(--border-strong)] flex flex-col gap-3"
              style:background="var(--surface-1)"
            >
              <div class="flex items-center justify-between gap-2">
                <span class="ui-section-title">Ссылка для входа</span>
                <a
                  href={msCodeData.verification_uri}
                  target="_blank"
                  rel="noreferrer"
                  class="ui-btn ui-btn-ghost ui-btn-sm"
                >
                  <ExternalLink size={12} strokeWidth={2.2} /> Открыть
                </a>
              </div>
              <a
                href={msCodeData.verification_uri}
                target="_blank"
                rel="noreferrer"
                class="text-sm font-mono break-all hover:underline"
                style:color="var(--accent-light)"
              >
                {msCodeData.verification_uri}
              </a>

              <div class="flex items-center justify-between gap-2 mt-2">
                <span class="ui-section-title">Код</span>
                <Button variant="ghost" size="sm" on:click={() => void copyMsCode()}>
                  <Copy size={12} strokeWidth={2.2} /> Скопировать
                </Button>
              </div>
              <div
                class="px-4 py-3 rounded-[var(--radius)] text-2xl font-mono tracking-[0.4em] text-center border border-[var(--border)]"
                style:background="var(--bg)"
              >
                {msCodeData.user_code}
              </div>
              <div class="flex items-center justify-center gap-2 text-xs" style:color="var(--accent-light)">
                <Loader2 size={12} class="animate-spin" strokeWidth={2.2} />
                <span>Ожидание подтверждения…</span>
              </div>
            </div>
          {/if}
        </div>
      {:else if authType === "elyby"}
        <div class="flex flex-col gap-3" in:fade={{ duration: 160 }}>
          <div>
            <label for="jm-ely-email" class="block text-sm font-medium mb-1">Email или ник</label>
            <div class="relative">
              <Mail class="absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none text-[var(--text-secondary)]" size={14} strokeWidth={2} />
              <input
                id="jm-ely-email"
                type="text"
                bind:value={elyEmail}
                placeholder="you@example.com"
                class="ui-input pl-9"
                autocomplete="username"
              />
            </div>
          </div>
          <div>
            <label for="jm-ely-pass" class="block text-sm font-medium mb-1">Пароль</label>
            <div class="relative">
              <Key class="absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none text-[var(--text-secondary)]" size={14} strokeWidth={2} />
              <input
                id="jm-ely-pass"
                type="password"
                bind:value={elyPass}
                class="ui-input pl-9"
                autocomplete="current-password"
              />
            </div>
          </div>
          <Button variant="primary" size="md" disabled={busyAdd} on:click={() => void handleElybyLogin()}>
            {#if busyAdd}<Loader2 size={14} class="animate-spin" strokeWidth={2.2} />{/if}
            Войти через Ely.by
          </Button>
        </div>
      {:else}
        <div class="flex flex-col gap-3" in:fade={{ duration: 160 }}>
          <div>
            <label for="jm-offline-name" class="block text-sm font-medium mb-1">Никнейм</label>
            <div class="relative">
              <User class="absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none text-[var(--text-secondary)]" size={14} strokeWidth={2} />
              <input
                id="jm-offline-name"
                type="text"
                bind:value={offlineName}
                placeholder="Steve"
                class="ui-input pl-9"
                autocomplete="off"
                on:keydown={(e) => e.key === "Enter" && !busyAdd && handleOfflineLogin()}
              />
            </div>
            <p class="ui-hint mt-1">Без лицензии Mojang — не работает на серверах с античитом/онлайн-проверкой.</p>
          </div>
          <Button variant="primary" size="md" disabled={busyAdd} on:click={() => void handleOfflineLogin()}>
            {#if busyAdd}<Loader2 size={14} class="animate-spin" strokeWidth={2.2} />{/if}
            Добавить пиратку
          </Button>
        </div>
      {/if}

      {#if status}
        <div
          in:fade={{ duration: 160 }}
          class="mt-4 text-xs px-3 py-2 rounded-[var(--radius)] border"
          class:border-[var(--border)]={statusKind === "info"}
          style:color={statusKind === "error" ? "#fca5a5" : statusKind === "success" ? "#86efac" : "var(--text-secondary)"}
          style:background={statusKind === "error"
            ? "color-mix(in srgb, #ef4444 12%, transparent)"
            : statusKind === "success"
            ? "color-mix(in srgb, #22c55e 12%, transparent)"
            : "var(--surface-1)"}
          style:border-color={statusKind === "error"
            ? "color-mix(in srgb, #ef4444 35%, transparent)"
            : statusKind === "success"
            ? "color-mix(in srgb, #22c55e 35%, transparent)"
            : "var(--border)"}
        >
          {status}
        </div>
      {/if}
    </Card>
  </div>
</div>

<style>
  :global(.ui-card.is-active) {
    box-shadow: 0 0 0 1px var(--accent);
  }
</style>
