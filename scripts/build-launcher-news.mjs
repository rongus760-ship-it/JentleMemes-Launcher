#!/usr/bin/env node
/**
 * Сборка единого news.json из нескольких JSON-файлов (источники на сайте / выгрузки из БД).
 *
 * Использование:
 *   node scripts/build-launcher-news.mjs
 *   node scripts/build-launcher-news.mjs --fetch   # подтянуть текущий с jentlememes.ru и смёржить
 *
 * Деплой на сервер (пример):
 *   scp scripts/launcher-news/dist/news.json user@jentlememes.ru:/var/www/.../launcher/news.json
 *   ssh user@jentlememes.ru 'sudo systemctl reload nginx'   # или restart вашего WSGI
 */

import { readdir, readFile, writeFile, mkdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "launcher-news");
const SOURCES_DIR = join(ROOT, "sources");
const DIST_DIR = join(ROOT, "dist");
const OUT_FILE = join(DIST_DIR, "news.json");
const LIVE_URL = "https://jentlememes.ru/launcher/news.json";

function asArray(raw) {
  if (Array.isArray(raw)) return raw;
  if (raw && typeof raw === "object") return [raw];
  return [];
}

function sortNews(items) {
  return [...items].sort((a, b) => {
    const pa = !!a.pinned;
    const pb = !!b.pinned;
    if (pa !== pb) return pb ? 1 : -1;
    return String(b.date || "").localeCompare(String(a.date || ""));
  });
}

function dedupeById(items) {
  const map = new Map();
  for (const item of items) {
    const id = String(item?.id ?? "");
    if (!id) continue;
    const prev = map.get(id);
    if (!prev || String(item.date || "") > String(prev.date || "")) {
      map.set(id, item);
    }
  }
  return [...map.values()];
}

async function loadSourceFiles() {
  let names = [];
  try {
    names = (await readdir(SOURCES_DIR)).filter((f) => f.endsWith(".json")).sort();
  } catch {
    return [];
  }
  const all = [];
  for (const name of names) {
    const text = await readFile(join(SOURCES_DIR, name), "utf8");
    const parsed = JSON.parse(text);
    all.push(...asArray(parsed));
  }
  return all;
}

async function maybeFetchLive() {
  if (!process.argv.includes("--fetch")) return [];
  try {
    const res = await fetch(LIVE_URL);
    if (!res.ok) return [];
    const data = await res.json();
    return asArray(data);
  } catch {
    return [];
  }
}

async function main() {
  await mkdir(SOURCES_DIR, { recursive: true });
  await mkdir(DIST_DIR, { recursive: true });

  const fromDisk = await loadSourceFiles();
  const fromLive = await maybeFetchLive();
  const merged = dedupeById([...fromLive, ...fromDisk]);
  const sorted = sortNews(merged);

  await writeFile(OUT_FILE, JSON.stringify(sorted, null, 2), "utf8");
  console.log(`Wrote ${sorted.length} item(s) → ${OUT_FILE}`);
  if (sorted.length === 0) {
    console.warn("Пусто: добавьте scripts/launcher-news/sources/*.json или запустите с --fetch");
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
