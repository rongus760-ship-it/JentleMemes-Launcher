import DOMPurify from "dompurify";

/** Убирает битые <img>, санитизирует HTML. */
export function sanitizeProjectBody(body: string): string {
  if (!body || typeof body !== "string") return "";
  let out = body.replace(/<img\s[^>]*>/gi, (tag) => {
    const m = tag.match(/src\s*=\s*["']([^"']*)["']/i);
    const src = m ? m[1].trim() : "";
    if (!/^https?:\/\//i.test(src)) return "";
    return tag;
  });
  out = DOMPurify.sanitize(out, {
    ALLOWED_TAGS: [
      "p", "br", "ul", "ol", "li", "strong", "em", "b", "i", "a", "h1", "h2", "h3", "h4",
      "img", "div", "span", "sup", "code", "pre", "blockquote", "hr", "table", "thead",
      "tbody", "tr", "th", "td",
    ],
    ALLOWED_ATTR: ["href", "src", "target", "rel", "alt"],
    ADD_ATTR: ["target"],
  });
  return out;
}

export function looksLikeHtml(text: string): boolean {
  if (!text || typeof text !== "string") return false;
  return /<\s*[a-z][^>]*>/i.test(text);
}

export const CATEGORY_MAP: Record<string, string[]> = {
  mod: [
    "optimization", "magic", "technology", "adventure", "decoration", "worldgen", "storage",
    "combat", "utility",
  ],
  modpack: [
    "optimization", "adventure", "combat", "multiplayer", "quests", "technology", "vanilla-plus",
  ],
  resourcepack: ["16x", "32x", "64x", "128x", "realistic", "stylized", "gui", "animated"],
  shader: ["realistic", "fantasy", "performance", "vanilla-like"],
  datapack: ["worldgen", "utility", "adventure", "combat", "decoration"],
};

export const GAME_VERSION_OPTIONS = [
  "1.21.5", "1.21.4", "1.21.3", "1.21.2", "1.21.1", "1.21", "1.20.6", "1.20.4", "1.20.2",
  "1.20.1", "1.20", "1.19.4", "1.19.2", "1.18.2", "1.17.1", "1.16.5", "1.12.2", "1.8.9", "1.7.10",
];

/** Mojang `version_manifest_v2`: релизы всегда; снапшоты и old_alpha/old_beta — по флагам. */
export function filterMojangManifestVersions(
  versions: { id?: string; type?: string }[],
  snapshots: boolean,
  legacyAlphaBeta: boolean,
): string[] {
  const out: string[] = [];
  for (const raw of versions || []) {
    const id = raw?.id;
    if (!id || typeof id !== "string") continue;
    const t = String(raw?.type || "").toLowerCase();
    if (t === "release") out.push(id);
    else if (snapshots && t === "snapshot") out.push(id);
    else if (legacyAlphaBeta && (t === "old_alpha" || t === "old_beta")) out.push(id);
  }
  return out;
}

/** Объединить статический список и динамический (Modrinth), сортировка id по убыванию. */
export function mergeGameVersionOptionStrings(staticList: string[], dynamic: string[]): string[] {
  const set = new Set<string>();
  for (const s of staticList) {
    if (s && typeof s === "string") set.add(s);
  }
  for (const s of dynamic) {
    if (s && typeof s === "string") set.add(s);
  }
  return Array.from(set).sort((a, b) => b.localeCompare(a, undefined, { numeric: true }));
}

export function titleInitials(title: string): string {
  const t = (title || "?")
    .split(/\s+/)
    .map((w) => w[0])
    .join("")
    .slice(0, 2)
    .toUpperCase();
  return t || "?";
}
