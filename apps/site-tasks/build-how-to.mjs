// Generates the `how-to` content collection from the single source of
// truth at `crates/compiler/zo-how-to/`. Each example is a `.zo` (code),
// an optional sibling `.md` (explanation), an optional
// `-- EXPECTED OUTPUT:` block (stdout programs), and an optional sibling
// image (visual programs). The site never reads the crate directly —
// this prebuild step mirrors everything into
// `apps/site/src/content/how-to/...` and copies sibling images into
// `public/how-to/...`. Run from `prebuild` / `predev`.

import {
  readdir,
  readFile,
  writeFile,
  mkdir,
  rm,
  copyFile,
} from "node:fs/promises";
import { existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = dirname(fileURLToPath(import.meta.url));
const HOWTO = join(ROOT, "..", "..", "crates", "compiler", "zo-how-to");
const OUT = join(ROOT, "..", "site", "src", "content", "how-to");
const PUBLIC = join(ROOT, "..", "site", "public", "how-to");

// Subdirs that may sit next to sources but aren't examples (build
// artifacts, shared fixtures) — never walked.
const SKIP_DIRS = new Set(["deps", "samples"]);

// Sibling preview formats for visual examples, in preference order.
const IMAGE_EXTS = ["gif", "webp", "png", "jpg", "jpeg", "svg"];

// `001_hello` -> { order: 1, name: "hello" }. No prefix keeps the bare
// stem and sorts last.
function parsePrefix(stem) {
  const match = stem.match(/^(\d+)_(.+)$/);

  if (match) return { order: Number(match[1]), name: match[2] };

  return { order: Number.POSITIVE_INFINITY, name: stem };
}

// Indent every line by 2 spaces so a multi-line string is a valid YAML
// literal block scalar — consistent indent is all YAML needs, so blank
// lines and `--` comments can't break the parse.
function indentBlock(source) {
  return source
    .replace(/\s+$/, "")
    .split("\n")
    .map((line) => `  ${line}`)
    .join("\n");
}

// Split a `.zo` into its code and the `-- EXPECTED OUTPUT:` block (the
// same directive the test runner verifies). The output lines are `-- `
// comments; strip the prefix. The block is removed from the code so the
// rendered source never shows the trailer.
function splitOutput(raw) {
  const expectedIdx = raw.indexOf("-- EXPECTED OUTPUT:");
  const stdinIdx = raw.indexOf("-- @stdin:");

  // The shown code stops at the first trailing test directive
  // (`-- @stdin:` or `-- EXPECTED OUTPUT:`) — both are runner-only and
  // shouldn't appear in the code pane.
  const markers = [expectedIdx, stdinIdx].filter((idx) => idx !== -1);
  const code = markers.length ? raw.slice(0, Math.min(...markers)) : raw;

  if (expectedIdx === -1) return { code, output: null };

  const output = raw
    .slice(expectedIdx)
    .split("\n")
    .slice(1)
    .map((line) => line.replace(/^\s*--\s?/, ""))
    .join("\n")
    .replace(/^\n+/, "")
    .replace(/\n+$/, "");

  return { code, output: output.length ? output : null };
}

// Lifts a `title:` from the explanation's optional frontmatter and
// returns the body without it.
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

// A sibling `<stem>.<ext>` image, copied to `public/how-to/<rel>.<ext>`
// and returned as a site URL. `null` when none exists.
async function siblingImage(dir, stem, rel) {
  for (const ext of IMAGE_EXTS) {
    const source = join(dir, `${stem}.${ext}`);

    if (existsSync(source)) {
      const dest = join(PUBLIC, `${rel}.${ext}`);

      await mkdir(dirname(dest), { recursive: true });
      await copyFile(source, dest);

      return `/how-to/${rel}.${ext}`;
    }
  }

  return null;
}

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
      const rel = source.group
        ? `${category}/${source.group}/${name}`
        : `${category}/${name}`;

      const { code, output } = splitOutput(
        await readFile(join(source.dir, source.file), "utf8"),
      );

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

      const image = await siblingImage(source.dir, stem, rel);

      examples.push({
        category,
        group: source.group,
        name,
        order,
        title: title ?? humanize(name),
        code,
        output,
        image,
        body,
      });
    }
  }

  return examples;
}

// Rebuild both trees from scratch so deleted/renamed examples and their
// images never linger. Clear BEFORE collecting — collectExamples copies
// sibling images into PUBLIC, so wiping it afterwards would delete them.
await rm(OUT, { recursive: true, force: true });
await rm(PUBLIC, { recursive: true, force: true });

const examples = await collectExamples();

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
    example.image ? `image: ${JSON.stringify(example.image)}` : null,
    "code: |",
    indentBlock(example.code),
    example.output ? "output: |" : null,
    example.output ? indentBlock(example.output) : null,
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
