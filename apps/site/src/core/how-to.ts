import { getCollection, type CollectionEntry } from "astro:content";
import type { Item } from "../components/navigation/navbar.astro";

export interface HowToExample {
  entry: CollectionEntry<"howto">;
  // The collection id (`zo/basic/hello`, `2d/snake`) — also the
  // details-page slug.
  slug: string;
  category: string;
  group: string | null;
  title: string;
  order: number;
}

export interface HowToGroup {
  name: string | null;
  examples: HowToExample[];
}

export interface HowToCategory {
  name: string;
  groups: HowToGroup[];
}

// Display order for the top-level domains; anything unlisted trails.
const CATEGORY_ORDER = ["zo", "zsx", "providers"];

function categoryRank(name: string): number {
  const index = CATEGORY_ORDER.indexOf(name);

  return index === -1 ? CATEGORY_ORDER.length : index;
}

function byOrderThenTitle(a: HowToExample, b: HowToExample): number {
  if (a.order !== b.order) return a.order - b.order;

  return a.title.localeCompare(b.title);
}

// Group columns sort alphabetically, but a digit-leading name (`7guis`)
// trails the lettered ones rather than leading them — `localeCompare`
// alone would put `7` before `b`. Ungrouped (`null` → "") stays first.
function compareGroups(a: string | null, b: string | null): number {
  const left = a ?? "";
  const right = b ?? "";
  const leftDigit = /^\d/.test(left);
  const rightDigit = /^\d/.test(right);

  if (leftDigit !== rightDigit) return leftDigit ? 1 : -1;

  return left.localeCompare(right);
}

// Flat list of every example, sorted by `order` then title.
export async function loadHowTo(): Promise<HowToExample[]> {
  const all = await getCollection("howto");

  return all
    .map((entry) => ({
      entry,
      slug: entry.id,
      category: entry.data.category,
      group: entry.data.group ?? null,
      title: entry.data.title ?? entry.id.split("/").at(-1) ?? entry.id,
      order: entry.data.order ?? Number.MAX_SAFE_INTEGER,
    }))
    .sort(byOrderThenTitle);
}

// Nested catalog: category → group → [examples]. Ungrouped examples
// (group === null) sort first within their category.
export async function getHowToTree(): Promise<HowToCategory[]> {
  const examples = await loadHowTo();
  const byCategory = new Map<string, Map<string | null, HowToExample[]>>();

  for (const example of examples) {
    let groups = byCategory.get(example.category);

    if (!groups) {
      groups = new Map();
      byCategory.set(example.category, groups);
    }

    let bucket = groups.get(example.group);

    if (!bucket) {
      bucket = [];
      groups.set(example.group, bucket);
    }

    bucket.push(example);
  }

  return [...byCategory.entries()]
    .sort((a, b) => categoryRank(a[0]) - categoryRank(b[0]))
    .map(([name, groups]) => ({
      name,
      groups: [...groups.entries()]
        .sort((a, b) => compareGroups(a[0], b[0]))
        .map(([groupName, groupExamples]) => ({
          name: groupName,
          examples: groupExamples,
        })),
    }));
}

function anchor(category: string, group?: string | null): string {
  return group ? `#${category}-${group}` : `#${category}`;
}

// Sidebar tree for `NavBar`: category → group → example. Categories and
// groups anchor to catalog sections; examples link to their details page.
export async function getHowToNavItems(): Promise<Item[]> {
  const tree = await getHowToTree();

  return tree.map((category) => ({
    href: anchor(category.name),
    label: category.name,
    children: category.groups.flatMap((group) => {
      const leaves: Item[] = group.examples.map((example) => ({
        href: `/how-to/${example.slug}`,
        label: example.title,
      }));

      if (!group.name) return leaves;

      return [{
        href: anchor(category.name, group.name),
        label: group.name,
        children: leaves,
      }];
    }),
  }));
}
