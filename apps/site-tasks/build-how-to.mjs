// Generates the `how-to` content collection from the single source of
// truth at `crates/compiler/zo-how-to/`. Each example is a `.zo` (code)
// plus an optional sibling `.md` (explanation). The site never reads the
// crate directly — this prebuild step mirrors every example into
// `apps/site/src/content/how-to/<category>/<group?>/<name>.md`, embedding
// the raw code as a YAML literal block scalar and the explanation as the
// body. Run from `prebuild`.

import { readdir, readFile, writeFile, mkdir, rm } from "node:fs/promises";
import { existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = dirname(fileURLToPath(import.meta.url));
const HOWTO = join(ROOT, "..", "..", "crates", "compiler", "zo-how-to");
const OUT = join(ROOT, "..", "site", "src", "content", "how-to");

// Subdirs that may sit next to sources but aren't examples (build
// artifacts, shared fixtures) — never walked.
const SKIP_DIRS = new Set(["deps", "samples"]);

// `001_hello` -> { order: 1, name: "hello" }. No prefix keeps the bare
// stem and sorts last. The prefix is for on-disk ordering only; it never
// reaches the slug or title.
function parsePrefix(stem) {
  const match = stem.match(/^(\d+)_(.+)$/);

  if (match) return { order: Number(match[1]), name: match[2] };

  return { order: Number.POSITIVE_INFINITY, name: stem };
}

// Indent every line by 2 spaces so the raw `.zo` is a valid YAML literal
// block scalar — consistent indentation greater than the `code:` key is
// all YAML needs, so blank lines and `--` comments can't break the parse.
function indentCode(source) {
  return source
    .replace(/\s+$/, "")
    .split("\n")
    .map((line) => `  ${line}`)
    .join("\n");
}

// Lifts a `title:` from the explanation's optional frontmatter and
// returns the body without it. Tiny on purpose — only `title` matters.
function splitExplanation(raw) {
  if (!raw.startsWith("---\n")) return { title: null, body: raw.trim() };

  const end = raw.indexOf("\n---\n", 4);

  if (end === -1) return { title: null, body: raw.trim() };

  const yaml = raw.slice(4, end);
  const body = raw.slice(end + 5).trim();
  const match = yaml.match(/^title:\s*(.+)$/m);

  return { title: match ? match[1].trim() : null, body };
}

function humanize(name) {
  return name.replace(/[_-]+/g, " ");
}

// Collect `{ category, group, name, order, title, code, body }` for every
// `.zo` under each category root and its one level of group subdirs.
async function collectExamples() {
  const examples = [];

  // Top-level domains are whatever directories sit under `zo-how-to`
  // (`zo`, `zsx`, `providers`, …) — discovered, not hardcoded, so a
  // restructure needs no generator change.
  const roots = await readdir(HOWTO, { withFileTypes: true });
  const categories = roots
    .filter((entry) => entry.isDirectory() && !SKIP_DIRS.has(entry.name))
    .map((entry) => entry.name)
    .sort();

  for (const category of categories) {
    const categoryDir = join(HOWTO, category);
    const sources = [];

    for (const entry of await readdir(categoryDir, { withFileTypes: true })) {
      if (entry.isFile() && entry.name.endsWith(".zo")) {
        sources.push({ dir: categoryDir, group: null, file: entry.name });
      } else if (entry.isDirectory() && !SKIP_DIRS.has(entry.name)) {
        const groupDir = join(categoryDir, entry.name);

        for (const sub of await readdir(groupDir, { withFileTypes: true })) {
          if (sub.isFile() && sub.name.endsWith(".zo")) {
            sources.push({ dir: groupDir, group: entry.name, file: sub.name });
          }
        }
      }
    }

    for (const source of sources) {
      const stem = source.file.slice(0, -".zo".length);
      const { order, name } = parsePrefix(stem);
      const code = await readFile(join(source.dir, source.file), "utf8");

      const explanationPath = join(source.dir, `${stem}.md`);
      let title = null;
      let body = "";

      if (existsSync(explanationPath)) {
        const explanation = splitExplanation(
          await readFile(explanationPath, "utf8"),
        );

        title = explanation.title;
        body = explanation.body;
      }

      examples.push({
        category,
        group: source.group,
        name,
        order,
        title: title ?? humanize(name),
        code,
        body,
      });
    }
  }

  return examples;
}

const examples = await collectExamples();

// Rebuild from scratch so deleted/renamed examples never linger.
await rm(OUT, { recursive: true, force: true });

for (const example of examples) {
  const dir = example.group
    ? join(OUT, example.category, example.group)
    : join(OUT, example.category);

  await mkdir(dir, { recursive: true });

  const frontmatter = [
    "---",
    `category: ${example.category}`,
    example.group ? `group: ${example.group}` : null,
    `title: ${JSON.stringify(example.title)}`,
    Number.isFinite(example.order) ? `order: ${example.order}` : null,
    "code: |",
    indentCode(example.code),
    "---",
    "",
    example.body,
    "",
  ]
    .filter((line) => line !== null)
    .join("\n");

  await writeFile(join(dir, `${example.name}.md`), frontmatter);
}

console.log(`[how-to] generated ${examples.length} example(s) -> ${OUT}`);
