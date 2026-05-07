import { getCollection, type CollectionEntry } from "astro:content";
import { baseLocale } from "../paraglide/runtime.js";
import type { Item } from "../components/navigation/navbar.astro";

export interface SpeechMeta {
  slug: string;
  season: number;
  episode: number;
  date: Date;
  subtitle: string | null;
}

export interface LoadedSpeech {
  entry: CollectionEntry<"speeches">;
  meta: SpeechMeta;
}

export const pad2 = (n: number) => String(n).padStart(2, "0");
export const formatEpisodeId = (season: number, episode: number) =>
  `S${pad2(season)}E${pad2(episode)}`;
export const formatDate = (date: Date) => date.toISOString().slice(0, 10);

// Filename pattern: S{season}E{episode}-DD-MM-YYYY (e.g. S02E15-15-01-2026).
// Subtitle pattern: first markdown blockquote `> *Theme.*`.
export function parseSpeech(entry: CollectionEntry<"speeches">): SpeechMeta | null {
  const filename = entry.id.split("/").pop() ?? "";
  const match = filename.match(/^s(\d+)e(\d+)-(\d{2})-(\d{2})-(\d{4})$/i);

  if (!match) return null;

  const [, season, episode, dd, mm, yyyy] = match;

  return {
    slug: filename.toLowerCase(),
    season: Number(season),
    episode: Number(episode),
    date: new Date(`${yyyy}-${mm}-${dd}`),
    subtitle: parseSubtitle(entry.body ?? ""),
  };
}

function parseSubtitle(body: string): string | null {
  const match = body.match(/^>\s*\*(.+?)\*\s*$/m);
  return match ? match[1] : null;
}

// Cached per-locale. Entry id is `<locale>/S0X/<filename>` — we group by
// (season, episode, filename) and pick the requested locale's entry, or
// fall back to baseLocale when that locale doesn't have it translated yet.
const cache = new Map<string, Promise<LoadedSpeech[]>>();

export function loadSpeeches(locale: string): Promise<LoadedSpeech[]> {
  const existing = cache.get(locale);
  if (existing) return existing;

  const promise = (async () => {
    const all = await getCollection("speeches");

    // Key on (season-folder, filename) to dedupe the same speech across
    // locales; entry.id starts with the locale segment.
    const byKey = new Map<string, Map<string, CollectionEntry<"speeches">>>();
    for (const entry of all) {
      const segments = entry.id.split("/");
      const entryLocale = segments[0];
      const key = segments.slice(1).join("/");
      if (!key) continue;
      let byLocale = byKey.get(key);
      if (!byLocale) {
        byLocale = new Map();
        byKey.set(key, byLocale);
      }
      byLocale.set(entryLocale, entry);
    }

    const result: LoadedSpeech[] = [];
    for (const [, byLocale] of byKey) {
      const entry = byLocale.get(locale) ?? byLocale.get(baseLocale);
      if (!entry) continue;
      const meta = parseSpeech(entry);
      if (!meta) continue;
      result.push({ entry, meta });
    }

    return result.sort(
      (a, b) => a.meta.date.getTime() - b.meta.date.getTime(),
    );
  })();

  cache.set(locale, promise);
  return promise;
}

// `hashPrefix` is "" on the list page (in-page filter via location.hash) and
// "/speeches" on detail pages so clicks navigate back and filter on landing.
export async function getSpeechNavItems(
  locale: string,
  hashPrefix: string = "",
): Promise<Item[]> {
  const speeches = await loadSpeeches(locale);
  const seasons = [...new Set(speeches.map((speech) => speech.meta.season))].sort();

  return [
    { href: `${hashPrefix}#all`, label: "All" },
    ...seasons.map((season) => ({
      href: `${hashPrefix}#s${pad2(season)}`,
      label: `S${pad2(season)}`,
    })),
  ];
}
