<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { MessageCircle } from "lucide-svelte";

  export let apiBase: string;
  /** Вкладка «Чат» включена в настройках */
  export let chatTabEnabled = false;

  const TOKEN_KEY = "jm_social_access_token";
  const CID_KEY = "jm_overlay_chat_active_cid";

  let token = "";
  let messages: { id: string; sender_id?: string; kind?: string; content?: { text?: string } }[] =
    [];
  let title = "";
  let err = "";
  let poll: ReturnType<typeof setInterval> | null = null;

  function base() {
    return String(apiBase || "https://jentlememes.ru").replace(/\/$/, "");
  }

  function msgLine(m: (typeof messages)[0]): string {
    const k = String(m.kind || "text");
    if (k === "call_invite") return "📞 Звонок";
    const t = m.content?.text;
    if (typeof t === "string" && t.trim()) return t.trim();
    return `(${k})`;
  }

  async function load() {
    if (!chatTabEnabled) {
      err = "";
      messages = [];
      return;
    }
    try {
      token = localStorage.getItem(TOKEN_KEY) || "";
    } catch {
      token = "";
    }
    if (!token) {
      err = "Войдите во вкладке «Чат» в лаунчере.";
      messages = [];
      return;
    }
    let cid = "";
    try {
      cid = localStorage.getItem(CID_KEY) || "";
    } catch {
      cid = "";
    }
    try {
      const h = new Headers();
      h.set("Authorization", `Bearer ${token}`);
      if (!cid) {
        const rc = await fetch(`${base()}/api/v1/conversations`, { headers: h });
        if (!rc.ok) throw new Error(`conversations ${rc.status}`);
        const cj = await rc.json();
        const list = (cj.conversations || cj || []) as { id: string }[];
        cid = list[0]?.id || "";
        if (!cid) {
          err = "Нет бесед — откройте чат в лаунчере.";
          messages = [];
          return;
        }
      }
      const rm = await fetch(
        `${base()}/api/v1/conversations/${encodeURIComponent(cid)}/messages?limit=40`,
        { headers: h },
      );
      if (!rm.ok) throw new Error(`messages ${rm.status}`);
      const mj = await rm.json();
      messages = (mj.messages || []).slice(-18);
      title = cid.slice(0, 8) + "…";
      err = "";
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
      messages = [];
    }
  }

  onMount(() => {
    void load();
    poll = setInterval(() => void load(), 3200);
  });

  onDestroy(() => {
    if (poll) clearInterval(poll);
  });
</script>

<div class="space-y-2 text-xs">
  <p class="text-[10px] uppercase tracking-wide opacity-55 flex items-center gap-1.5">
    <MessageCircle size={14} class="shrink-0" style="color: var(--accent, #86a886);" />
    Чат {#if title}<span class="font-mono opacity-70">{title}</span>{/if}
  </p>
  {#if !chatTabEnabled}
    <p class="text-amber-200/90 text-[11px] leading-relaxed">
      Включите «Показывать вкладку чата» в расширенных настройках лаунчера.
    </p>
  {:else if err}
    <p class="text-red-300/90 text-[11px]">{err}</p>
  {:else if messages.length === 0}
    <p class="opacity-60 text-[11px]">Нет сообщений.</p>
  {:else}
    <div
      class="max-h-40 overflow-y-auto custom-scrollbar space-y-1 pr-1 text-[11px] leading-snug"
    >
      {#each messages as m (m.id)}
        <p class="opacity-90 border-l-2 pl-2 py-0.5" style="border-color: var(--accent, #86a886);">
          {msgLine(m)}
        </p>
      {/each}
    </div>
  {/if}
</div>
