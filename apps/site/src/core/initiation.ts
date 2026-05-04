import { getCollection, render, type CollectionEntry } from "astro:content";
import type { Item } from "../components/navigation/navbar.astro";

export interface LoadedInitiation {
  entry: CollectionEntry<"initiation">;
  slug: string;
  label: string;
}

let cached: Promise<LoadedInitiation[]> | null = null;

export function loadInitiations(): Promise<LoadedInitiation[]> {
  if (cached) return cached;
  cached = (async () => {
    const entries = await getCollection("initiation");
    return entries
      .map((entry) => ({
        entry,
        slug: entry.id,
        label: entry.data.title ?? humanize(entry.id),
      }))
      .sort((a, b) => {
        const oa = a.entry.data.order ?? Number.MAX_SAFE_INTEGER;
        const ob = b.entry.data.order ?? Number.MAX_SAFE_INTEGER;
        if (oa !== ob) return oa - ob;
        return a.slug.localeCompare(b.slug);
      });
  })();
  return cached;
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
export async function getInitiationNavItems(): Promise<Item[]> {
  const initiations = await loadInitiations();

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
