import { getCollection, type CollectionEntry } from "astro:content";
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

// Loads + parses + sorts the whole speech collection once. Cached at module
// scope so detail pages (rendered N times during build) don't re-parse 190
// entries each — a single shared promise services every page.
let cached: Promise<LoadedSpeech[]> | null = null;

export function loadSpeeches(): Promise<LoadedSpeech[]> {
  if (cached) return cached;

  cached = (async () => {
    const entries = await getCollection("speeches");
    return entries
      .map((entry) => ({ entry, meta: parseSpeech(entry) }))
      .filter((speech): speech is LoadedSpeech => speech.meta !== null)
      .sort((a, b) => a.meta.date.getTime() - b.meta.date.getTime());
  })();

  return cached;
}

// `hashPrefix` is "" on the list page (in-page filter via location.hash) and
// "/speeches" on detail pages so clicks navigate back and filter on landing.
export async function getSpeechNavItems(hashPrefix: string = ""): Promise<Item[]> {
  const speeches = await loadSpeeches();
  const seasons = [...new Set(speeches.map((speech) => speech.meta.season))].sort();

  return [
    { href: `${hashPrefix}#all`, label: "All" },
    ...seasons.map((season) => ({
      href: `${hashPrefix}#s${pad2(season)}`,
      label: `S${pad2(season)}`,
    })),
  ];
}
