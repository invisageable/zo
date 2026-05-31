import { readdir, readFile, writeFile, mkdir } from "node:fs/promises";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = dirname(fileURLToPath(import.meta.url));
const SRC = join(ROOT, "..", "site", "src", "content", "initiation", "en");
const OUT = join(ROOT, "..", "site", "public", "docs", "llms-full.txt");

const HEADER = `# zo programming language — full documentation

> Aggregated documentation for LLM ingestion. Generated from \`apps/site/src/content/initiation/en/*.md\`. For the curated index with link-only entries, see [/llms.txt](https://zo.compilords.house/llms.txt).

`;

const FOOTER = `
---

## further reading

- [Grammar (EBNF)](https://github.com/invisageable/zo/blob/main/crates/compiler/zo-notes/public/grammar/zo.ebnf)
- [guidelines](https://github.com/invisageable/zo/tree/main/crates/compiler/zo-notes/public/guidelines)
- [Discord](https://discord.gg/JaNc4Nk5xw)
- [GitHub Issues](https://github.com/invisageable/zo/issues)
`;

// Lifts the `order` frontmatter field for sorting, returns body without
// the YAML block. Frontmatter parsing is intentionally tiny — only `order`
// is needed; we don't pull a yaml dep for two integers.
function splitFrontmatter(raw) {
  if (!raw.startsWith("---\n")) return { order: Number.POSITIVE_INFINITY, body: raw };
  const end = raw.indexOf("\n---\n", 4);
  if (end === -1) return { order: Number.POSITIVE_INFINITY, body: raw };
  const yaml = raw.slice(4, end);
  const body = raw.slice(end + 5);
  const orderMatch = yaml.match(/^order:\s*(-?\d+)/m);
  const order = orderMatch ? Number(orderMatch[1]) : Number.POSITIVE_INFINITY;
  return { order, body: body.trimStart() };
}

const entries = (await readdir(SRC))
  .filter((name) => name.endsWith(".md"))
  .map((name) => ({ name, path: join(SRC, name) }));

const loaded = await Promise.all(
  entries.map(async (entry) => {
    const raw = await readFile(entry.path, "utf8");
    return { ...entry, ...splitFrontmatter(raw) };
  }),
);

loaded.sort((a, b) => {
  if (a.order !== b.order) return a.order - b.order;
  return a.name.localeCompare(b.name);
});

const aggregated = HEADER + loaded
  .map((entry) => entry.body.trim())
  .join("\n\n---\n\n") + FOOTER;

await mkdir(dirname(OUT), { recursive: true });
await writeFile(OUT, aggregated, "utf8");

console.log(`[llms-full] wrote ${OUT} (${loaded.length} entries, ${aggregated.length} bytes)`);
