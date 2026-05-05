import type { Root, Parent, RootContent, PhrasingContent } from "mdast";
import { visit } from "unist-util-visit";

// Strips inline `[prev](...) — [next](...)` paragraphs from speech markdown.
// Prev/next nav is rendered by the page template instead so it stays in sync
// with the sorted collection — no per-file maintenance when episodes shift.
export function remarkSpeechNav() {
  return (tree: Root) => {
    const removals: Array<[Parent, number]> = [];

    visit(tree, "paragraph", (node, index, parent) => {
      if (typeof index !== "number" || !parent) return;

      const text = phrasingText(node.children);
      if (/^\s*\[prev\]\([^)]+\)\s*[—-]\s*\[next\]\([^)]+\)\s*$/i.test(text)) {
        removals.push([parent as Parent, index]);
      }
    });

    // Reverse so earlier indices stay valid as we splice.
    for (const [parent, index] of removals.reverse()) {
      parent.children.splice(index, 1);
    }
  };
}

function phrasingText(nodes: PhrasingContent[] | RootContent[]): string {
  let out = "";

  for (const node of nodes) {
    if (node.type === "text") {
      out += node.value;
    } else if (node.type === "link") {
      out += `[${phrasingText(node.children)}](${node.url})`;
    } else if ("children" in node) {
      out += phrasingText(node.children as PhrasingContent[]);
    }
  }

  return out;
}
