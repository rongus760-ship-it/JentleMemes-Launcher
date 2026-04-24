<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { instanceIconSrc } from "../utils/instanceIcon";
  import ServerMotdBlock from "../components/ServerMotdBlock.svelte";
  import Card from "../components/ui/Card.svelte";
  import Button from "../components/ui/Button.svelte";
  import { showToast } from "../lib/jmEvents";
  import {
    Server,
    Users,
    ArrowRight,
    Plus,
    Play,
    X,
    Globe,
    Download,
    Loader2,
    Import,
  } from "lucide-svelte";
  import { fade } from "svelte/transition";
  import { portal } from "../lib/portalAction";

  export let setActiveTab: (tab: string) => void;
  export let openInstance: ((id: string) => void) | undefined = undefined;
  export let onLaunchWithServer: ((instanceId: string, serverIp: string) => void) | undefined =
    undefined;
  export let onLaunchWorld: ((instanceId: string, worldName: string) => void) | undefined =
    undefined;

  let recommendedPacks: any[] = [];
  let servers: any[] = [];
  let myInstances: any[] = [];
  let serverMenu: {
    ip: string;
    name: string;
    motd?: string;
    motd_chat?: unknown;
    motd_lines?: string[];
    samples?: { name?: string; skin_url?: string | null }[];
    players?: number;
    max?: number;
    online?: boolean;
  } | null = null;
  let serverMenuInstances: any[] = [];
  let addServerModal = false;
  let addServerIp = "";
  let addServerName = "";
  let addServerError = "";
  let lastWorld: {
    instance_id: string;
    instance_name: string;
    world_name: string;
    last_played?: number;
  } | null = null;
  let launcherNews: any[] = [];
  let showNews = true;
  let discoverTab: "packs" | "news" = "packs";

  let packModal: {
    projectId: string;
    title: string;
    author?: string;
    icon_url?: string;
    description?: string;
  } | null = null;
  let packModalLoading = false;
  let packModalDetails: any = null;
  let packModalVersions: any[] = [];
  let packInstallBusy = false;
  let newsModal: any | null = null;


  function loadServers(instanceId?: string) {
    invoke("load_servers", { instanceId: instanceId ?? null })
      .then((serversList: unknown) => {
        let saved = Array.isArray(serversList) ? (serversList as any[]) : [];
        if (saved.length === 0) {
          saved.push({
            ip: "play.jentlememes.ru",
            name: "JentleMemes Main",
            last_played: 0,
            playtime_hours: 0,
          });
        }
        const srvs = saved.map((s: any) => ({
          ...s,
          online: false,
          players: 0,
          max: 0,
          motd: "Загрузка...",
          motd_chat: null as unknown,
          motd_lines: [] as string[],
          samples: [] as { name?: string; skin_url?: string | null }[],
          icon: s.icon || null,
        }));
        servers = srvs;
        srvs.forEach((srv: any) => {
          const targetIp = String(srv.ip || "");
          invoke("ping_server", { ip: targetIp })
            .then((data: any) => {
              if (data?.online) {
                const iconVal = data.icon;
                const icon =
                  typeof iconVal === "string" && iconVal
                    ? iconVal.startsWith("data:")
                      ? iconVal
                      : `data:image/png;base64,${iconVal}`
                    : null;
                const clean = Array.isArray(data.motd?.clean) ? data.motd.clean : [];
                const list = Array.isArray(data.players?.list) ? data.players.list : [];
                servers = servers.map((row) =>
                  String(row.ip) === targetIp
                    ? {
                        ...row,
                        online: true,
                        players: data.players?.online || 0,
                        max: data.players?.max || 0,
                        motd: clean[0] || "Сервер работает",
                        motd_chat: data.motd?.chat ?? null,
                        motd_lines: clean,
                        samples: list,
                        icon,
                      }
                    : row
                );
              } else {
                const errLine =
                  typeof data?.error === "string" && data.error.trim()
                    ? data.error.trim().split("\n")[0]
                    : "";
                const motdOff = errLine ? `Недоступен: ${errLine}` : "Сервер выключен или недоступен";
                servers = servers.map((row) =>
                  String(row.ip) === targetIp
                    ? {
                        ...row,
                        online: false,
                        motd: motdOff,
                        motd_chat: null,
                        motd_lines: errLine ? [motdOff] : [],
                        samples: [],
                      }
                    : row
                );
              }
            })
            .catch((e) => {
              const msg = String(e ?? "Ошибка подключения");
              const short = msg.includes("\n") ? msg.split("\n")[0] : msg;
              servers = servers.map((row) =>
                String(row.ip) === targetIp
                  ? {
                      ...row,
                      online: false,
                      motd: short.slice(0, 200) || "Ошибка подключения",
                      motd_chat: null,
                      motd_lines: [],
                      samples: [],
                    }
                  : row
              );
            });
        });
      })
      .catch(() => {
        servers = [
          {
            ip: "play.jentlememes.ru",
            name: "JentleMemes Main",
            last_played: 0,
            playtime_hours: 0,
            online: false,
            players: 0,
            max: 0,
            motd: "Ошибка загрузки",
            icon: null,
          },
        ];
      });
  }

  function agoLabel(ts: number) {
    const h = Math.floor((Date.now() / 1000 - ts) / 3600);
    if (h < 1) return "Играл недавно";
    if (h < 24) return `Играл ${h} ч назад`;
    return `Играл ${Math.floor(h / 24)} дн. назад`;
  }

  let instancesChangedUnlisten: (() => void) | undefined;

  async function refreshHomeInstances() {
    try {
      const insts = ((await invoke("get_instances")) as any[]) || [];
      myInstances = insts.slice(0, 4);
    } catch {
      myInstances = [];
    }
  }

  onMount(() => {
    fetch(
      "https://api.modrinth.com/v2/search?limit=4&facets=[[%22project_type:modpack%22]]"
    )
      .then((res) => res.json())
      .then((data) => (recommendedPacks = data.hits || []))
      .catch(console.error);
    void refreshHomeInstances();
    void listen("instances_changed", () => {
      void refreshHomeInstances();
    }).then((f) => {
      instancesChangedUnlisten = f;
    });
    loadServers();
    invoke("get_last_world", { instanceId: null })
      .then((w: any) => (lastWorld = w?.instance_id ? w : null))
      .catch(() => (lastWorld = null));
    invoke("load_settings")
      .then((s: any) => (showNews = s?.show_news !== false))
      .catch(() => {});
    invoke("fetch_launcher_news")
      .then((items: unknown) => (launcherNews = ((items as any[]) || []).slice(0, 3)))
      .catch(() => {});
  });

  onDestroy(() => {
    instancesChangedUnlisten?.();
  });

  $: if (serverMenu && onLaunchWithServer) {
    invoke("get_instances")
      .then((insts: unknown) => (serverMenuInstances = (insts as any[]) || []))
      .catch(() => (serverMenuInstances = []));
  }

  function clickOpenLastWorld() {
    if (lastWorld == null || openInstance == null) return;
    openInstance(lastWorld.instance_id);
  }

  function playLastWorld(e: MouseEvent) {
    e.stopPropagation();
    if (lastWorld == null || onLaunchWorld == null) return;
    onLaunchWorld(lastWorld.instance_id, lastWorld.world_name);
    setActiveTab("library");
  }

  function launchInstanceWithServerMenu(instId: string) {
    if (serverMenu == null || onLaunchWithServer == null) return;
    onLaunchWithServer(instId, serverMenu.ip);
    serverMenu = null;
    setActiveTab("library");
  }

  /** Инициалы для плейсхолдера без TS в шаблоне (svelte-check) */
  function packInitials(title: string | undefined): string {
    const t = (title || "?")
      .split(/\s+/)
      .map((w: string) => w[0])
      .join("")
      .slice(0, 2)
      .toUpperCase();
    return t || "?";
  }

  async function openHomePackModal(pack: any) {
    const pid = pack.project_id || pack.slug;
    if (!pid) {
      setActiveTab("discover");
      return;
    }
    packModal = {
      projectId: String(pid),
      title: pack.title || "",
      author: pack.author,
      icon_url: pack.icon_url,
      description: pack.description,
    };
    packModalLoading = true;
    packModalDetails = null;
    packModalVersions = [];
    try {
      const details: any = await invoke("get_modrinth_project", { id: String(pid) });
      packModalDetails = details;
      const versions: any = await invoke("get_modrinth_versions", { id: String(pid) });
      packModalVersions = Array.isArray(versions) ? versions : [];
    } catch (e) {
      console.error(e);
      showToast(`Ошибка: ${e}`);
    } finally {
      packModalLoading = false;
    }
  }

  async function installHomePackModal() {
    const title = packModalDetails?.title || packModal?.title || "Modpack";
    const v = packModalVersions[0];
    if (!v) {
      showToast("Нет версий для установки");
      return;
    }
    const file = v?.files?.find((f: any) => f.primary) || v?.files?.[0];
    if (!file?.url) {
      showToast("Файл не найден");
      return;
    }
    packInstallBusy = true;
    try {
      showToast(`Установка сборки ${title}...`);
      await invoke("install_mrpack_from_url", {
        url: file.url,
        name: title,
        modrinthProjectId: packModal?.projectId || null,
        modrinthVersionId: v?.id != null ? String(v.id) : null,
        curseforgeProjectId: null,
        curseforgeFileId: null,
      });
      showToast("Сборка успешно установлена!");
      packModal = null;
    } catch (e) {
      showToast(`Ошибка: ${e}`);
    } finally {
      packInstallBusy = false;
    }
  }
</script>


<div class="jm-container flex flex-col h-full gap-5 pt-5 pb-8 overflow-y-auto custom-scrollbar">
  {#if lastWorld}
    <section>
      <p class="ui-section-title mb-2">Продолжить играть</p>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="ui-card ui-card-interactive p-4 flex items-center gap-4 cursor-pointer"
        on:click={clickOpenLastWorld}
      >
        <div
          class="w-12 h-12 rounded-[var(--radius)] bg-[var(--surface-1)] flex items-center justify-center shrink-0"
        >
          <Globe size={22} strokeWidth={1.8} style="color: var(--accent-light)" />
        </div>
        <div class="flex-grow min-w-0">
          <h2 class="text-base font-semibold truncate leading-tight">{lastWorld.world_name}</h2>
          <p class="text-xs mt-0.5 truncate" style:color="var(--text-secondary)">
            {lastWorld.instance_name}{(lastWorld.last_played ?? 0) > 0
              ? ` · ${agoLabel(lastWorld.last_played ?? 0)}`
              : ""}
          </p>
        </div>
        {#if onLaunchWorld}
          <Button variant="primary" on:click={playLastWorld}>
            <Play size={13} strokeWidth={2.2} fill="currentColor" />
            Играть
          </Button>
        {/if}
      </div>
    </section>
  {:else if myInstances.length > 0}
    {@const hero = myInstances[0]}
    {@const heroIcon = instanceIconSrc(hero.icon)}
    <section>
      <p class="ui-section-title mb-2">Последняя сборка</p>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="ui-card ui-card-interactive p-4 flex items-center gap-4 cursor-pointer"
        on:click={() => (openInstance ? openInstance(hero.id) : setActiveTab("library"))}
      >
        {#if heroIcon}
          <img
            src={heroIcon}
            alt=""
            class="w-12 h-12 rounded-[var(--radius)] object-cover shrink-0"
          />
        {:else}
          <div
            class="w-12 h-12 rounded-[var(--radius)] bg-[var(--surface-1)] flex items-center justify-center text-base font-semibold shrink-0"
            style:color="var(--text-secondary)"
          >
            {hero.name?.charAt(0)?.toUpperCase() || "?"}
          </div>
        {/if}
        <div class="flex-grow min-w-0">
          <h2 class="text-base font-semibold truncate leading-tight">{hero.name}</h2>
          <p class="text-xs mt-0.5 truncate" style:color="var(--text-secondary)">
            {hero.loader} · {hero.game_version}
          </p>
        </div>
        <Button variant="primary" on:click={() => setActiveTab("library")}>
          <Play size={13} strokeWidth={2.2} fill="currentColor" />
          Играть
        </Button>
      </div>
    </section>
  {:else}
    <section>
      <div class="ui-card p-6 flex flex-col items-center text-center gap-3">
        <h2 class="text-lg font-semibold">Добро пожаловать в JentleMemes</h2>
        <p class="text-sm max-w-md" style:color="var(--text-secondary)">
          Создайте первую сборку или найдите готовую в каталоге — и играйте за несколько кликов.
        </p>
        <div class="flex gap-2 mt-1">
          <Button variant="primary" on:click={() => setActiveTab("library")}>
            <Plus size={14} strokeWidth={2.2} />
            Новая сборка
          </Button>
          <Button variant="subtle" on:click={() => setActiveTab("discover")}>Каталог</Button>
        </div>
      </div>
    </section>
  {/if}

  <!-- Две колонки: сборки + серверы -->
  <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
    <Card>
      <svelte:fragment slot="header">
        <h3 class="ui-heading">Ваши сборки</h3>
        <p class="ui-hint mt-0.5">
          {myInstances.length > 0 ? `${myInstances.length} активных` : "Ещё нет сборок"}
        </p>
      </svelte:fragment>
      <svelte:fragment slot="action">
        <button
          type="button"
          on:click={() => setActiveTab("library")}
          class="ui-btn ui-btn-ghost ui-btn-sm"
        >
          Все <ArrowRight size={12} strokeWidth={2.2} />
        </button>
      </svelte:fragment>
      {#if myInstances.length === 0}
        <div class="text-sm text-center py-6" style:color="var(--text-secondary)">
          Сборок пока нет.
          <button
            type="button"
            class="underline hover:no-underline ml-1"
            style:color="var(--accent-light)"
            on:click={() => setActiveTab("library")}
          >
            Создать
          </button>
        </div>
      {:else}
        <div class="flex flex-col -mx-2">
          {#each myInstances as inst (inst.id)}
            {@const homeIcon = instanceIconSrc(inst.icon)}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              on:click={() => (openInstance ? openInstance(inst.id) : setActiveTab("library"))}
              class="flex items-center gap-3 px-2 py-2 rounded-[var(--radius)] cursor-pointer transition-colors hover:bg-[var(--surface-hover)] group"
            >
              {#if homeIcon}
                <img
                  src={homeIcon}
                  alt=""
                  class="w-9 h-9 rounded-[var(--radius-sm)] object-cover border border-[var(--border)] shrink-0"
                />
              {:else}
                <div
                  class="w-9 h-9 rounded-[var(--radius-sm)] bg-[var(--surface-1)] border border-[var(--border)] flex items-center justify-center text-sm font-medium shrink-0"
                  style:color="var(--text-secondary)"
                >
                  {inst.name?.charAt(0)?.toUpperCase() || "?"}
                </div>
              {/if}
              <div class="min-w-0 flex-1">
                <div class="font-medium truncate text-sm">{inst.name}</div>
                <div class="text-xs truncate" style:color="var(--text-secondary)">
                  {inst.loader} · {inst.game_version}
                </div>
              </div>
              <ArrowRight
                size={14}
                strokeWidth={2}
                class="opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
                style="color: var(--accent-light)"
              />
            </div>
          {/each}
        </div>
      {/if}
    </Card>

    <Card>
      <svelte:fragment slot="header">
        <h3 class="ui-heading">Серверы</h3>
        <p class="ui-hint mt-0.5">
          {servers.filter((s) => s.online).length} онлайн из {servers.length}
        </p>
      </svelte:fragment>
      <svelte:fragment slot="action">
        <button
          type="button"
          title="Импорт из servers.dat"
          on:click={async () => {
            try {
              const n = await invoke("import_servers_from_dat", { instanceId: null });
              showToast(n ? `Импортировано серверов: ${n}` : "Нет новых серверов");
              loadServers();
            } catch (e) {
              showToast(`Ошибка: ${e}`);
            }
          }}
          class="ui-btn ui-btn-ghost ui-btn-icon"
          aria-label="Импорт"
        >
          <Import size={14} strokeWidth={2} />
        </button>
        <button
          type="button"
          on:click={() => {
            addServerModal = true;
            addServerIp = "";
            addServerName = "";
            addServerError = "";
          }}
          class="ui-btn ui-btn-subtle ui-btn-sm"
        >
          <Plus size={12} strokeWidth={2.2} /> Добавить
        </button>
      </svelte:fragment>
      <div class="flex flex-col -mx-2">
        {#each servers as srv, i (srv.ip + i)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            on:click={() => onLaunchWithServer && (serverMenu = { ...srv })}
            class="flex items-center gap-3 px-2 py-2 rounded-[var(--radius)] transition-colors {onLaunchWithServer
              ? 'cursor-pointer hover:bg-[var(--surface-hover)] group'
              : ''}"
          >
            {#if srv.icon}
              <img
                src={srv.icon}
                alt=""
                class="w-9 h-9 rounded-[var(--radius-sm)] object-cover border border-[var(--border)] shrink-0"
                style:image-rendering="pixelated"
              />
            {:else}
              <div
                class="w-9 h-9 rounded-[var(--radius-sm)] bg-[var(--surface-1)] border border-[var(--border)] flex items-center justify-center shrink-0"
              >
                <Server size={16} strokeWidth={1.8} style="color: var(--text-secondary)" />
              </div>
            {/if}
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <span class="font-medium truncate text-sm">{srv.name}</span>
                <span class="ui-chip {srv.online ? 'ui-chip-success' : 'ui-chip-danger'}">
                  {srv.online ? "ON" : "OFF"}
                </span>
              </div>
              <div class="text-xs truncate" style:color="var(--text-secondary)">{srv.ip}</div>
            </div>
            {#if srv.online}
              <div
                class="flex items-center gap-1 text-xs shrink-0 tabular-nums"
                style:color="var(--text-secondary)"
              >
                <Users size={12} strokeWidth={2} />
                {srv.players}/{srv.max}
              </div>
            {/if}
          </div>
        {/each}
        {#if servers.length === 0}
          <div class="text-sm text-center py-6" style:color="var(--text-secondary)">
            Нет серверов
          </div>
        {/if}
      </div>
    </Card>
  </div>

  <section>
    <div class="flex items-center justify-between mb-3 gap-3 flex-wrap">
      <div class="flex items-center gap-3">
        <h3 class="ui-heading">Открытия</h3>
        <div class="ui-seg" role="tablist">
          <button
            type="button"
            role="tab"
            class="ui-seg-item"
            class:is-active={discoverTab === "packs"}
            on:click={() => (discoverTab = "packs")}
          >
            Сборки
          </button>
          {#if showNews && launcherNews.length > 0}
            <button
              type="button"
              role="tab"
              class="ui-seg-item"
              class:is-active={discoverTab === "news"}
              on:click={() => (discoverTab = "news")}
            >
              Новости
            </button>
          {/if}
        </div>
      </div>
      <button
        type="button"
        class="ui-btn ui-btn-ghost ui-btn-sm"
        on:click={() => setActiveTab(discoverTab === "news" ? "news" : "discover")}
      >
        {discoverTab === "news" ? "Все новости" : "Каталог"}
        <ArrowRight size={12} strokeWidth={2.2} />
      </button>
    </div>

    {#if discoverTab === "packs"}
      <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
        {#each recommendedPacks as pack (pack.project_id)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            on:click={() => openHomePackModal(pack)}
            class="ui-card ui-card-interactive p-3 flex flex-col"
          >
            {#if pack.icon_url}
              <img
                src={pack.icon_url}
                alt=""
                class="w-10 h-10 rounded-[var(--radius-sm)] object-cover border border-[var(--border)] mb-2"
              />
            {:else}
              <div
                class="w-10 h-10 rounded-[var(--radius-sm)] bg-[var(--surface-1)] border border-[var(--border)] flex items-center justify-center text-xs font-medium mb-2"
                style:color="var(--text-secondary)"
              >
                {packInitials(pack?.title)}
              </div>
            {/if}
            <h4 class="font-medium text-sm truncate">{pack.title}</h4>
            <p class="text-xs truncate" style:color="var(--text-secondary)">от {pack.author}</p>
            <p class="text-xs mt-1 line-clamp-2" style:color="var(--text-secondary)">
              {pack.description}
            </p>
          </div>
        {/each}
        {#if recommendedPacks.length === 0}
          <div class="col-span-full text-center text-sm py-6" style:color="var(--text-secondary)">
            Загрузка рекомендаций…
          </div>
        {/if}
      </div>
    {:else}
      <div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
        {#each launcherNews as item (item.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            on:click={() => (newsModal = item)}
            class="ui-card ui-card-interactive overflow-hidden"
          >
            {#if item.image}
              <div class="h-24 overflow-hidden">
                <img src={item.image} alt="" class="w-full h-full object-cover" />
              </div>
            {/if}
            <div class="p-3">
              <div class="flex items-center gap-2 mb-1">
                {#if item.tag}
                  <span class="ui-chip ui-chip-accent">{item.tag}</span>
                {/if}
                <span class="text-[10px]" style:color="var(--text-secondary)">
                  {item.date ? new Date(item.date).toLocaleDateString("ru") : ""}
                </span>
              </div>
              <h4 class="font-medium text-sm truncate">{item.title}</h4>
              <p class="text-xs mt-1 line-clamp-2" style:color="var(--text-secondary)">
                {item.body}
              </p>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  {#if addServerModal}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      use:portal
      class="fixed inset-0 z-[10055] flex items-center justify-center p-4 bg-black/70"
      on:click={() => (addServerModal = false)}
      transition:fade={{ duration: 140 }}
    >
      <div
        class="ui-card p-6 w-full max-w-md"
        on:click={(e) => e.stopPropagation()}
        role="dialog"
      >
        <div class="flex justify-between items-center mb-4">
          <h3 class="text-lg font-semibold">Добавить сервер</h3>
          <button
            type="button"
            on:click={() => (addServerModal = false)}
            class="ui-btn ui-btn-ghost ui-btn-icon"
            aria-label="Закрыть"
          >
            <X size={16} strokeWidth={2} />
          </button>
        </div>
        <div class="space-y-3">
          <div>
            <label class="block text-xs mb-1" style:color="var(--text-secondary)">
              IP или адрес
            </label>
            <input
              bind:value={addServerIp}
              placeholder="play.example.com или 192.168.1.1:25565"
              class="ui-input"
            />
          </div>
          <div>
            <label class="block text-xs mb-1" style:color="var(--text-secondary)">
              Название (необязательно)
            </label>
            <input bind:value={addServerName} placeholder="Мой сервер" class="ui-input" />
          </div>
          {#if addServerError}
            <p class="text-sm" style:color="#f87171">{addServerError}</p>
          {/if}
        </div>
        <div class="flex gap-2 mt-5 justify-end">
          <Button variant="ghost" on:click={() => (addServerModal = false)}>Отмена</Button>
          <Button
            variant="primary"
            on:click={async () => {
              addServerError = "";
              try {
                await invoke("add_recent_server", {
                  ip: addServerIp,
                  name: addServerName,
                  instanceId: null,
                });
                showToast("Сервер добавлен!");
                addServerModal = false;
                loadServers();
              } catch (e) {
                addServerError =
                  e instanceof Error ? e.message : typeof e === "string" ? e : String(e);
              }
            }}
          >
            Добавить
          </Button>
        </div>
      </div>
    </div>
  {/if}

  {#if serverMenu && onLaunchWithServer}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      use:portal
      class="fixed inset-0 z-[10055] flex items-center justify-center p-4 bg-black/70"
      on:click={() => (serverMenu = null)}
      transition:fade={{ duration: 140 }}
    >
      <div
        class="ui-card p-6 w-full max-w-md"
        on:click={(e) => e.stopPropagation()}
        role="dialog"
      >
        <div class="flex justify-between items-center mb-4">
          <h3 class="text-lg font-semibold">Играть на сервере</h3>
          <button
            type="button"
            on:click={() => (serverMenu = null)}
            class="ui-btn ui-btn-ghost ui-btn-icon"
            aria-label="Закрыть"
          >
            <X size={16} strokeWidth={2} />
          </button>
        </div>
        <div
          class="mb-4 p-3 rounded-[var(--radius)] border border-[var(--border)]"
          style:background="var(--surface-1)"
        >
          <div class="flex items-center justify-between mb-1">
            <strong class="text-sm">{serverMenu.name}</strong>
            <span class="ui-chip {serverMenu.online ? 'ui-chip-success' : 'ui-chip-danger'}">
              {serverMenu.online ? "Online" : "Offline"}
            </span>
          </div>
          <p class="text-xs mb-1" style:color="var(--text-secondary)">{serverMenu.ip}</p>
          {#if serverMenu.online && (serverMenu.motd_chat != null || (serverMenu.motd_lines && serverMenu.motd_lines.length))}
            <div class="mt-2 min-w-0">
              <ServerMotdBlock
                motdChat={serverMenu.motd_chat}
                motdLines={serverMenu.motd_lines || []}
                samples={serverMenu.samples || []}
                compact={false}
                showFaces={true}
              />
            </div>
          {:else if serverMenu.motd}
            <p class="text-xs" style:color="var(--text-secondary)">{serverMenu.motd}</p>
          {/if}
          {#if serverMenu.online && serverMenu.players !== undefined}
            <p class="text-xs mt-1" style:color="var(--accent-light)">
              Игроков: {serverMenu.players}/{serverMenu.max ?? "?"}
            </p>
          {/if}
        </div>
        <p class="ui-section-title mb-2">Выберите сборку</p>
        <div class="flex flex-col gap-1 max-h-64 overflow-y-auto custom-scrollbar -mx-1 px-1">
          {#each serverMenuInstances as inst (inst.id)}
            {@const srvIcon = instanceIconSrc(inst.icon)}
            <button
              type="button"
              on:click={() => launchInstanceWithServerMenu(inst.id)}
              class="flex items-center gap-3 px-2 py-2 rounded-[var(--radius)] transition-colors hover:bg-[var(--surface-hover)] text-left group"
            >
              {#if srvIcon}
                <img
                  src={srvIcon}
                  alt=""
                  class="w-9 h-9 rounded-[var(--radius-sm)] object-cover border border-[var(--border)] shrink-0"
                />
              {:else}
                <div
                  class="w-9 h-9 rounded-[var(--radius-sm)] bg-[var(--surface-1)] border border-[var(--border)] flex items-center justify-center text-sm font-medium shrink-0"
                  style:color="var(--text-secondary)"
                >
                  {inst.name?.charAt(0)?.toUpperCase() || "?"}
                </div>
              {/if}
              <div class="min-w-0 flex-1">
                <div class="text-sm font-medium truncate">{inst.name}</div>
                <div class="text-xs truncate" style:color="var(--text-secondary)">
                  {inst.loader} · {inst.game_version}
                </div>
              </div>
              <Play
                size={14}
                strokeWidth={2}
                class="opacity-0 group-hover:opacity-100 shrink-0"
                style="color: var(--accent-light)"
              />
            </button>
          {/each}
          {#if serverMenuInstances.length === 0}
            <div class="text-sm text-center py-6" style:color="var(--text-secondary)">
              Нет сборок. Создайте сборку во вкладке «Сборки».
            </div>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  {#if packModal}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      use:portal
      class="fixed inset-0 z-[10055] flex items-center justify-center p-4 bg-black/70"
      transition:fade={{ duration: 140 }}
      on:click={() => (packModal = null)}
    >
      <div
        class="ui-card p-6 w-full max-w-md max-h-[85vh] overflow-y-auto custom-scrollbar"
        on:click={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <div class="flex justify-between items-start gap-3 mb-4">
          <div class="flex items-center gap-3 min-w-0">
            {#if packModal.icon_url}
              <img
                src={packModal.icon_url}
                alt=""
                class="w-12 h-12 rounded-[var(--radius)] object-cover border border-[var(--border)] shrink-0"
              />
            {:else}
              <div
                class="w-12 h-12 rounded-[var(--radius)] bg-[var(--surface-1)] border border-[var(--border)] flex items-center justify-center text-sm font-medium shrink-0"
                style:color="var(--text-secondary)"
              >
                {packInitials(packModal.title)}
              </div>
            {/if}
            <div class="min-w-0">
              <h3 class="text-base font-semibold truncate">
                {packModalDetails?.title || packModal.title}
              </h3>
              {#if packModal.author || packModalDetails?.author}
                <p class="text-xs truncate" style:color="var(--text-secondary)">
                  от {packModalDetails?.author || packModal.author}
                </p>
              {/if}
            </div>
          </div>
          <button
            type="button"
            on:click={() => (packModal = null)}
            class="ui-btn ui-btn-ghost ui-btn-icon"
            aria-label="Закрыть"
          >
            <X size={16} strokeWidth={2} />
          </button>
        </div>
        {#if packModalLoading}
          <div class="flex justify-center py-10">
            <Loader2 size={28} class="animate-spin" style="color: var(--accent-light)" />
          </div>
        {:else}
          <p class="text-sm line-clamp-4 mb-5" style:color="var(--text-secondary)">
            {packModalDetails?.description || packModal.description || "Нет описания"}
          </p>
          <button
            type="button"
            disabled={packInstallBusy || packModalVersions.length === 0}
            on:click={installHomePackModal}
            class="ui-btn ui-btn-primary w-full"
          >
            {#if packInstallBusy}
              <Loader2 size={14} class="animate-spin shrink-0" />
              Загрузка…
            {:else}
              <Download size={14} strokeWidth={2.2} class="shrink-0" />
              Загрузить сборку
            {/if}
          </button>
        {/if}
      </div>
    </div>
  {/if}

  {#if newsModal}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      use:portal
      class="fixed inset-0 z-[10055] bg-black/70 flex items-center justify-center p-4"
      transition:fade={{ duration: 140 }}
      on:click={() => (newsModal = null)}
    >
      <div
        class="ui-card max-w-lg w-full max-h-[80vh] overflow-y-auto custom-scrollbar overflow-hidden"
        on:click|stopPropagation
        role="dialog"
        aria-modal="true"
      >
        {#if newsModal.image}
          <img src={newsModal.image} alt="" class="w-full h-40 object-cover" />
        {/if}
        <div class="p-6">
          <div class="flex items-center gap-2 mb-3 flex-wrap">
            {#if newsModal.tag}
              <span class="ui-chip ui-chip-accent">{newsModal.tag}</span>
            {/if}
            <span class="text-xs" style:color="var(--text-secondary)">
              {newsModal.date ? new Date(newsModal.date).toLocaleDateString("ru") : ""}
            </span>
          </div>
          <h2 class="text-lg font-semibold mb-3">{newsModal.title}</h2>
          <p class="text-sm leading-relaxed whitespace-pre-wrap" style:color="var(--text-secondary)">
            {newsModal.body}
          </p>
          <div class="flex gap-2 mt-5 justify-end">
            <Button
              variant="ghost"
              on:click={() => {
                newsModal = null;
                setActiveTab("news");
              }}
            >
              Все новости
            </Button>
            <Button variant="primary" on:click={() => (newsModal = null)}>Закрыть</Button>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>
