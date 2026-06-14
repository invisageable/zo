import { render, type CollectionEntry } from "astro:content";
import { baseLocale } from "../paraglide/runtime.js";
import type { Item } from "../components/navigation/navbar.astro";

// Single-document pages (spec, faq) keep the same per-locale layout as
// the collections — `<locale>/<slug>.md` — but resolve to one entry.
// Pick the entry for `locale`, falling back to baseLocale.
export function pickLocale<T extends { id: string }>(
  entries: T[],
  locale: string,
): T | undefined {
  const byLocale = new Map<string, T>();
  for (const entry of entries) {
    byLocale.set(entry.id.split("/")[0], entry);
  }
  return byLocale.get(locale) ?? byLocale.get(baseLocale);
}

// Flat table of contents from a page's h2–h3 headings, for the NavBar.
export async function getPageNavItems(
  entry: CollectionEntry<"spec"> | CollectionEntry<"faq">,
): Promise<Item[]> {
  const { headings } = await render(entry);

  return headings
    .filter((h) => h.depth >= 2 && h.depth <= 3)
    .map((h) => ({ href: `#${h.slug}`, label: h.text }));
}
