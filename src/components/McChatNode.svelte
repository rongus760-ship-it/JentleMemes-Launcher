<script lang="ts">
  /**
   * Рекурсивный рендер Minecraft chat JSON + строк с § (legacy).
   * Только безопасные span/style — без {@html} из ответа сервера.
   */
  export let node: unknown;

  const NAMED: Record<string, string> = {
    black: "#000000",
    dark_blue: "#0000AA",
    dark_green: "#00AA00",
    dark_aqua: "#00AAAA",
    dark_red: "#AA0000",
    dark_purple: "#AA00AA",
    gold: "#FFAA00",
    gray: "#AAAAAA",
    dark_gray: "#555555",
    blue: "#5555FF",
    green: "#55FF55",
    aqua: "#55FFFF",
    red: "#FF5555",
    light_purple: "#FF55FF",
    yellow: "#FFFF55",
    white: "#FFFFFF",
  };

  const DIGIT: Record<string, string> = {
    "0": "#000000",
    "1": "#0000AA",
    "2": "#00AA00",
    "3": "#00AAAA",
    "4": "#AA0000",
    "5": "#AA00AA",
    "6": "#FFAA00",
    "7": "#AAAAAA",
    "8": "#555555",
    "9": "#5555FF",
    a: "#55FF55",
    b: "#55FFFF",
    c: "#FF5555",
    d: "#FF55FF",
    e: "#FFFF55",
    f: "#FFFFFF",
  };

  type LegacySeg = { t: string; color?: string; bold?: boolean; italic?: boolean; under?: boolean; strike?: boolean };

  function parseLegacy(s: string): LegacySeg[] {
    let color: string | undefined;
    let bold = false;
    let italic = false;
    let under = false;
    let strike = false;
    let buf = "";
    const out: LegacySeg[] = [];
    const flush = () => {
      if (!buf) return;
      out.push({
        t: buf,
        color,
        bold,
        italic,
        under,
        strike,
      });
      buf = "";
    };
    for (let i = 0; i < s.length; i++) {
      const c = s[i];
      if (c === "§" && i + 1 < s.length) {
        const code = s[i + 1].toLowerCase();
        i++;
        flush();
        if (DIGIT[code]) {
          color = DIGIT[code];
          continue;
        }
        if (code === "r") {
          color = undefined;
          bold = italic = under = strike = false;
          continue;
        }
        if (code === "l") bold = true;
        else if (code === "o") italic = true;
        else if (code === "n") under = true;
        else if (code === "m") strike = true;
        continue;
      }
      buf += c;
    }
    flush();
    return out;
  }

  function stylesFromObj(n: Record<string, unknown>): string {
    let s = "";
    const col = n.color;
    if (typeof col === "string") {
      const hex = NAMED[col] || (col.startsWith("#") ? col : "");
      if (hex) s += `color:${hex};`;
    }
    if (n.bold === true) s += "font-weight:700;";
    if (n.italic === true) s += "font-style:italic;";
    const dec: string[] = [];
    if (n.underlined === true) dec.push("underline");
    if (n.strikethrough === true) dec.push("line-through");
    if (dec.length) s += `text-decoration:${dec.join(" ")};`;
    return s;
  }

  $: isObj = node !== null && typeof node === "object" && !Array.isArray(node);
  $: obj = isObj ? (node as Record<string, unknown>) : null;
  $: extraArr =
    obj && Array.isArray(obj.extra) ? (obj.extra as unknown[]) : ([] as unknown[]);
</script>

{#if node === null || node === undefined}
  <!-- empty -->
{:else if typeof node === "string"}
  {#each parseLegacy(node) as seg, i (i)}
    <span
      class="mc-chat-bit"
      style:color={seg.color || "var(--jm-motd-fg, rgba(255,255,255,0.92))"}
      style:font-weight={seg.bold ? "700" : undefined}
      style:font-style={seg.italic ? "italic" : undefined}
      style:text-decoration={seg.under && seg.strike
        ? "underline line-through"
        : seg.under
          ? "underline"
          : seg.strike
            ? "line-through"
            : undefined}>{seg.t}</span
    >
  {/each}
{:else if Array.isArray(node)}
  {#each node as item, idx (idx)}
    <svelte:self node={item} />
  {/each}
{:else if isObj && obj}
  {#if typeof obj.text === "string" && obj.text.length > 0}
    <span class="mc-chat-bit" style={stylesFromObj(obj)}>{obj.text}</span>
  {/if}
  {#each extraArr as ex, j (j)}
    <svelte:self node={ex} />
  {/each}
{/if}

<style>
  .mc-chat-bit {
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.55);
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
