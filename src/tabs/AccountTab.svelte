<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { emit, listen } from "@tauri-apps/api/event";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { User, Mail, Key, MonitorSmartphone, Trash2, CheckCircle2 } from "lucide-svelte";
  import SkinHeadAvatar from "../components/SkinHeadAvatar.svelte";

  let profiles: {
    accounts: any[];
    active_account_id: string;
    skin_presets: any[];
  } = { accounts: [], active_account_id: "", skin_presets: [] };

  let authType: "offline" | "elyby" | "microsoft" = "offline";
  let status = "";
  let offlineName = "";
  let elyEmail = "";
  let elyPass = "";
  let msCodeData: any = null;
  let msMiniPoll: ReturnType<typeof setInterval> | null = null;
  let profilesUnlisten: (() => void) | undefined;
  /** Кэш src для списка аккаунтов (сессия Ely/MS или helm / локальный скин) */
  let avatarByAccountId: Record<string, string> = {};
  /** Локальный PNG или URL сессии — рисуем голову, не целую развёртку */
  let avatarHeadFromTexture: Record<string, boolean> = {};

  const authTabs: Array<"offline" | "elyby" | "microsoft"> = ["offline", "elyby", "microsoft"];

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
    status = "";
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
      status = "Введите никнейм!";
      return;
    }
    status = "Создание оффлайн аккаунта...";
    try {
      const acc = await invoke("login_offline", { username: offlineName });
      await handleAddAccount(acc);
      status = "Аккаунт добавлен!";
      offlineName = "";
    } catch (e) {
      status = `Ошибка: ${e}`;
    }
  }

  async function handleElybyLogin() {
    if (!elyEmail || !elyPass) {
      status = "Введите email и пароль!";
      return;
    }
    status = "Авторизация в Ely.by...";
    try {
      const acc = await invoke("login_elyby", { email: elyEmail, password: elyPass });
      await handleAddAccount(acc);
      status = "Успешный вход через Ely.by!";
      elyEmail = "";
      elyPass = "";
    } catch (e) {
      status = `Ошибка: ${e}`;
    }
  }

  async function handleMicrosoftInit() {
    status = "Связь с серверами Microsoft...";
    try {
      const data: any = await invoke("ms_init_device_code");
      msCodeData = data;
      status = "Ожидание авторизации в браузере... Перейдите по ссылке и введите код!";
      const acc = await invoke("ms_login_poll", {
        deviceCode: data.device_code,
        interval: data.interval,
      });
      await handleAddAccount(acc);
      status = "Успешный вход через Microsoft!";
      msCodeData = null;
    } catch (e) {
      status = `Ошибка: ${e}`;
      msCodeData = null;
    }
  }

  async function handleMicrosoftMiniBrowser() {
    if (msMiniPoll) {
      status = "Вход уже выполняется в мини-окне.";
      return;
    }
    status = "Открывается мини-браузер… Войдите в Microsoft.";
    try {
      const url = (await invoke("ms_oauth_prepare_interactive")) as string;
      const existing = await WebviewWindow.getByLabel("jentle-ms-oauth");
      if (existing) await existing.close();
      const win = new WebviewWindow("jentle-ms-oauth", {
        url,
        title: "Вход Microsoft / Xbox",
        width: 520,
        height: 820,
        center: true,
      });
      win.once("tauri://error", (e) => {
        status = `Ошибка окна: ${e}`;
      });
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
          if (payload.ok && payload.account) {
            await handleAddAccount(payload.account);
            status = "Успешный вход через Microsoft!";
          } else {
            status = `Ошибка: ${payload.error ?? "неизвестно"}`;
          }
        } catch {
          /* ignore */
        }
      }, 400);
      setTimeout(() => {
        if (msMiniPoll) {
          clearInterval(msMiniPoll);
          msMiniPoll = null;
        }
      }, 900_000);
    } catch (e) {
      status = `Ошибка: ${e}`;
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
</script>

<div class="flex flex-col md:flex-row gap-4 w-full max-w-6xl mx-auto items-start">
  <div
    class="w-full md:w-1/2 bg-jm-card p-4 rounded-2xl border border-white/10 shadow-xl card-hover-subtle jm-reveal backdrop-blur-sm"
    style="animation-delay: 0.04s"
  >
    <h2 class="text-lg md:text-xl font-bold text-jm-accent-light mb-4 flex items-center gap-2">
      <User size={18} /> Ваши аккаунты
    </h2>
    {#if profiles.accounts.length === 0}
      <div class="text-[var(--text-secondary)] text-center py-10">Нет добавленных аккаунтов</div>
    {:else}
      <div class="space-y-3">
        {#each profiles.accounts as acc (acc.id)}
          {@const active = profiles.active_account_id === acc.id}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            on:click={() => handleSetActive(acc.id)}
            class="flex items-center justify-between p-3 rounded-xl border transition-all cursor-pointer {active
              ? 'bg-jm-accent/20 border-jm-accent shadow-[0_0_15px_rgba(134,168,134,0.1)]'
              : 'bg-black/40 border-white/5 hover:border-jm-accent/30'}"
          >
            <div class="flex items-center gap-3 min-w-0">
              {#if avatarHeadFromTexture[acc.id]}
                <SkinHeadAvatar
                  src={avatarByAccountId[acc.id] ?? avatarUrl(acc)}
                  size={40}
                  alt="Аватар"
                  wrapperClass="rounded-lg"
                />
              {:else}
                <img
                  src={avatarByAccountId[acc.id] ?? avatarUrl(acc)}
                  alt="avatar"
                  class="w-10 h-10 rounded-lg object-cover shrink-0"
                  style:image-rendering="pixelated"
                />
              {/if}
              <div class="min-w-0">
                <div class="font-bold text-sm text-white flex items-center gap-1.5 truncate">
                  {acc.username}{#if active}<CheckCircle2 size={14} class="text-jm-accent shrink-0" />{/if}
                </div>
                <div class="text-[10px] text-[var(--text-secondary)] uppercase tracking-wider">
                  {acc.acc_type}
                </div>
              </div>
            </div>
            <button
              type="button"
              on:click|stopPropagation={() => void handleDelete(acc.id)}
              class="p-1.5 text-[var(--text-secondary)] hover:text-red-500 hover:bg-red-500/10 rounded-lg transition-colors shrink-0"
            >
              <Trash2 size={16} />
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <div
    class="w-full md:w-1/2 bg-jm-card p-4 rounded-2xl border border-white/10 shadow-xl card-hover-subtle jm-reveal backdrop-blur-sm"
    style="animation-delay: 0.1s"
  >
    <h2 class="text-lg md:text-xl font-bold text-jm-accent-light mb-4">Добавить аккаунт</h2>
    <div class="flex gap-1 mb-4 bg-black/30 p-0.5 rounded-lg border border-white/5">
      {#each authTabs as type (type)}
        <button
          type="button"
          on:click={() => selectAuthTab(type)}
          class="flex-1 py-1.5 rounded-md text-xs font-bold transition-all {authType === type
            ? 'bg-jm-accent text-black shadow-md'
            : 'text-[var(--text-secondary)] hover:text-white'}"
        >
          {#if type === "offline"}Пиратка{:else if type === "elyby"}Ely.by{:else}Microsoft{/if}
        </button>
      {/each}
    </div>

    {#if authType === "offline"}
      <div class="space-y-4">
        <div>
          <label class="text-sm text-[var(--text-secondary)] mb-1 block" for="jm-offline-name">Никнейм</label>
          <div class="relative">
            <User class="absolute left-3 top-3 text-[var(--text-secondary)]" size={18} />
            <input
              id="jm-offline-name"
              type="text"
              bind:value={offlineName}
              placeholder="Steve"
              class="w-full bg-[var(--input-bg)] border border-white/10 rounded-xl pl-10 pr-4 py-2.5 text-white focus:border-jm-accent outline-none"
            />
          </div>
        </div>
        <button
          type="button"
          on:click={() => void handleOfflineLogin()}
          class="w-full bg-jm-accent hover:bg-jm-accent-light text-black font-bold py-3 rounded-xl transition-colors"
        >
          Добавить пиратку
        </button>
      </div>
    {:else if authType === "elyby"}
      <div class="space-y-4">
        <div>
          <label class="text-sm text-[var(--text-secondary)] mb-1 block" for="jm-ely-email">Email или Ник</label>
          <div class="relative">
            <Mail class="absolute left-3 top-3 text-[var(--text-secondary)]" size={18} />
            <input
              id="jm-ely-email"
              type="text"
              bind:value={elyEmail}
              class="w-full bg-[var(--input-bg)] border border-white/10 rounded-xl pl-10 pr-4 py-2.5 text-white focus:border-jm-accent outline-none"
            />
          </div>
        </div>
        <div>
          <label class="text-sm text-[var(--text-secondary)] mb-1 block" for="jm-ely-pass">Пароль</label>
          <div class="relative">
            <Key class="absolute left-3 top-3 text-[var(--text-secondary)]" size={18} />
            <input
              id="jm-ely-pass"
              type="password"
              bind:value={elyPass}
              class="w-full bg-[var(--input-bg)] border border-white/10 rounded-xl pl-10 pr-4 py-2.5 text-white focus:border-jm-accent outline-none"
            />
          </div>
        </div>
        <button
          type="button"
          on:click={() => void handleElybyLogin()}
          class="w-full bg-[#0078D7] hover:bg-[#1f8ce1] text-white font-bold py-3 rounded-xl transition-colors"
        >
          Войти через Ely.by
        </button>
      </div>
    {:else}
      <div class="space-y-4 text-center">
        {#if !msCodeData}
          <MonitorSmartphone class="mx-auto text-[var(--text-secondary)] mb-4" size={48} />
          <p class="text-[var(--text-secondary)] text-sm mb-4">
            Встроенное окно (рекомендуется) или код во внешнем браузере.
          </p>
          <button
            type="button"
            on:click={() => void handleMicrosoftMiniBrowser()}
            class="w-full bg-[#107C10] hover:bg-[#149614] text-white font-bold py-3 rounded-xl transition-colors mb-3"
          >
            Войти в мини-браузере
          </button>
          <button
            type="button"
            on:click={() => void handleMicrosoftInit()}
            class="w-full bg-white/10 hover:bg-white/15 text-white font-bold py-3 rounded-xl transition-colors border border-white/10"
          >
            Получить код (внешний браузер)
          </button>
          <p class="text-[10px] text-[var(--text-secondary)] mt-3 leading-snug text-left">
            Если мини-браузер выдаёт ошибку redirect: в Azure (приложение) → Authentication →
            добавьте платформу «Mobile and desktop» и URI вида
            <span class="font-mono text-jm-accent/90">http://127.0.0.1</span>
            (loopback).
          </p>
        {:else}
          <div class="bg-black/40 p-6 rounded-xl border border-jm-accent/30">
            <p class="text-sm text-[var(--text-secondary)] mb-2">1. Откройте эту ссылку:</p>
            <a
              href={msCodeData.verification_uri}
              target="_blank"
              rel="noreferrer"
              class="text-jm-accent-light font-bold text-lg hover:underline block mb-6"
              >{msCodeData.verification_uri}</a
            >
            <p class="text-sm text-[var(--text-secondary)] mb-2">2. Введите код:</p>
            <div
              class="bg-black text-white text-3xl font-mono tracking-widest py-4 rounded-lg border border-white/10 mb-4"
            >
              {msCodeData.user_code}
            </div>
            <p class="text-xs text-jm-accent animate-pulse">⏳ Ожидание авторизации...</p>
          </div>
        {/if}
      </div>
    {/if}

    {#if status}
      <div
        class="mt-6 text-sm text-center text-jm-accent-light bg-black/40 px-4 py-3 rounded-lg border border-jm-accent/20"
      >
        {status}
      </div>
    {/if}
  </div>
</div>
