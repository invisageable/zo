import { getCollection, render, type CollectionEntry } from "astro:content";
import { baseLocale } from "../paraglide/runtime.js";
import type { Item } from "../components/navigation/navbar.astro";

export interface LoadedInitiation {
  entry: CollectionEntry<"initiation">;
  slug: string;
  label: string;
}

// Cached per-locale: each locale picks a different subset of entries
// (with fallback to baseLocale), so they can't share a single promise.
const cache = new Map<string, Promise<LoadedInitiation[]>>();

export function loadInitiations(locale: string): Promise<LoadedInitiation[]> {
  const existing = cache.get(locale);
  if (existing) return existing;

  const promise = (async () => {
    const all = await getCollection("initiation");

    // Group entries by their bare slug (the part after `<locale>/`),
    // then for each slug pick the requested locale or fall back to en.
    const bySlug = new Map<string, Map<string, CollectionEntry<"initiation">>>();
    for (const entry of all) {
      const [entryLocale, ...rest] = entry.id.split("/");
      const slug = rest.join("/");
      if (!slug) continue;
      let byLocale = bySlug.get(slug);
      if (!byLocale) {
        byLocale = new Map();
        bySlug.set(slug, byLocale);
      }
      byLocale.set(entryLocale, entry);
    }

    const result: LoadedInitiation[] = [];
    for (const [slug, byLocale] of bySlug) {
      const entry = byLocale.get(locale) ?? byLocale.get(baseLocale);
      if (!entry) continue;
      result.push({
        entry,
        slug,
        label: entry.data.title ?? humanize(slug),
      });
    }

    return result.sort((a, b) => {
      const oa = a.entry.data.order ?? Number.MAX_SAFE_INTEGER;
      const ob = b.entry.data.order ?? Number.MAX_SAFE_INTEGER;
      if (oa !== ob) return oa - ob;
      return a.slug.localeCompare(b.slug);
    });
  })();

  cache.set(locale, promise);
  return promise;
}

// Builds the navbar tree for the single-page Initiation:
//   Initiation
//   ├ Install            → #install
//   │  ├ <h2>...         → #<heading-slug>
//   │  └ <h3>...
//   ├ Hello              → #hello
//   │  └ ...
// All entries live on /initiation; clicks scroll to anchors via NavBar's
// findSection (matches `[data-section]`) or default browser anchor jump.
export async function getInitiationNavItems(locale: string): Promise<Item[]> {
  const initiations = await loadInitiations(locale);

  return Promise.all(
    initiations.map(async (initiation) => {
      const { headings } = await render(initiation.entry);
      return {
        href: `#${initiation.slug}`,
        label: initiation.label,
        children: headings
          .filter((h) => h.depth >= 3 && h.depth <= 4)
          .map((h) => ({
            href: `#${h.slug}`,
            label: h.text,
          })),
      };
    }),
  );
}

function humanize(slug: string): string {
  return slug
    .replace(/^\d+[-_]/, "")
    .replace(/^\w/, (c) => c.toUpperCase());
}
