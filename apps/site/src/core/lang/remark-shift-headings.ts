import type { Root } from "mdast";
import type { VFile } from "vfile";
import { visit } from "unist-util-visit";

// Shifts all heading levels by +1 (h1 → h2, ..., h5 → h6) in initiation
// markdown entries. The page template owns the single page-level h1 in
// `<Initiation>`; entry headings nest below it. Writers can still start
// each `.md` with `# title` naturally — no convention to remember.
export function remarkShiftHeadings() {
  return (tree: Root, file: VFile) => {
    if (!file.path?.includes("/content/initiation/")) return;

    visit(tree, "heading", (node) => {
      node.depth = Math.min(node.depth + 1, 6) as 1 | 2 | 3 | 4 | 5 | 6;
    });
  };
}
