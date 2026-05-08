import { getCollection, type CollectionEntry } from "astro:content";
import { baseLocale } from "../paraglide/runtime.js";
import type { Item } from "../components/navigation/navbar.astro";

export interface NewsMeta {
  slug: string;
  season: number;
  episode: number;
  date: Date;
  subtitle: string | null;
}

export interface LoadedNews {
  entry: CollectionEntry<"news">;
  meta: NewsMeta;
}

export const pad2 = (n: number) => String(n).padStart(2, "0");
export const formatEpisodeId = (season: number, episode: number) =>
  `S${pad2(season)}E${pad2(episode)}`;
export const formatDate = (date: Date) => date.toISOString().slice(0, 10);

// Filename pattern: S{season}E{episode}-DD-MM-YYYY (e.g. S01E03-15-01-2026).
// Subtitle pattern: first markdown blockquote `> *Theme.*`.
export function parseNews(entry: CollectionEntry<"news">): NewsMeta | null {
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

const cache = new Map<string, Promise<LoadedNews[]>>();

export function loadNews(locale: string): Promise<LoadedNews[]> {
  const existing = cache.get(locale);
  if (existing) return existing;

  const promise = (async () => {
    const all = await getCollection("news");

    const byKey = new Map<string, Map<string, CollectionEntry<"news">>>();
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

    const result: LoadedNews[] = [];
    for (const [, byLocale] of byKey) {
      const entry = byLocale.get(locale) ?? byLocale.get(baseLocale);
      if (!entry) continue;
      const meta = parseNews(entry);
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

export async function getNewsNavItems(
  locale: string,
  hashPrefix: string = "",
): Promise<Item[]> {
  const news = await loadNews(locale);
  const seasons = [...new Set(news.map((entry) => entry.meta.season))].sort();

  return [
    { href: `${hashPrefix}#all`, label: "All" },
    ...seasons.map((season) => ({
      href: `${hashPrefix}#s${pad2(season)}`,
      label: `S${pad2(season)}`,
    })),
  ];
}
