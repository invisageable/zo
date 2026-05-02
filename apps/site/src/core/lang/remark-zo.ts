import type { Root } from "mdast";
import { visit } from "unist-util-visit";
import { highlight } from "./highlighter";

// Remark plugin: replaces code blocks tagged with a `*zo`-suffixed language
// (e.g. `zo`, `css:zo`, `js:zo`) with raw HTML produced by our zo highlighter.
// Bypasses Shiki entirely for these blocks — Shiki keeps owning everything else.
export function remarkZo() {
  return (tree: Root) => {
    visit(tree, "code", (node, index, parent) => {
      if (typeof index !== "number" || !parent) return;
      if (!node.lang || !/(^|[:|])zo$/.test(node.lang)) return;

      const inner = highlight(node.value)
        .map((span) =>
          span.kind === null
            ? escapeHtml(span.text)
            : `<span class="${span.kind}">${escapeHtml(span.text)}</span>`,
        )
        .join("");

      parent.children[index] = {
        type: "html",
        value: `<pre class="zo-block"><code>${inner}</code></pre>`,
      };
    });
  };
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}
