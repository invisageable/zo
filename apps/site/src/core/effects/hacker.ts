// Ported from codrops' ClipHoverEffect:
//   https://github.com/codrops/ClipHoverEffect
//
// Same alphabet as the reference: lowercase a–z plus a sprinkle of
// symbols. No digits, no uppercase — keeps the shuffle visually
// consistent with the source effect.
const LETTERS_AND_SYMBOLS = [
  "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
  "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
  "!", "@", "#", "$", "%", "^", "&", "*", "-", "_", "+", "=", ";",
  ":", "<", ">", ",",
];

interface AttachOptions {
  iterations?: number;    // shuffles per char before settling (default 3)
  charDelayMs?: number;   // ms between shuffles for one char (default 30)
  staggerMs?: number;     // ms between each char's animation start (default 0)
  trigger?: "hover" | "manual";
}

/**
 * Mr. Robot / Matrix-style hover decode effect. Splits an element's text
 * into per-character spans, then on hover cycles each through random
 * glyphs N times before settling back to the original.
 *
 * One Hacker instance can attach to many elements; per-element tokens
 * keep concurrent runs from overwriting each other.
 */
export class Hacker {
  private tokens = new WeakMap<HTMLElement, number>();

  // Idempotent. Re-splits the current text (so callers that mutate
  // textContent can call again without losing the hover behavior) and
  // binds the hover listener once per element.
  attach(element: HTMLElement, options: AttachOptions = {}): void {
    this.split(element);
    if ((options.trigger ?? "hover") !== "hover") return;
    if (element.dataset.hackerBound === "1") return;
    element.dataset.hackerBound = "1";
    element.addEventListener("mouseenter", () => this.shuffle(element, options));
  }

  // Wrap each character of the current text content in a `.hacker-char`
  // span. Idempotent — calling again replaces the previous split with a
  // fresh one based on the current textContent.
  split(element: HTMLElement): void {
    this.splitChars(element);
  }

  // Manual trigger for callers that want to drive the effect themselves
  // (e.g., on viewport-enter, on page load, on a custom event).
  play(element: HTMLElement, options: AttachOptions = {}): void {
    this.shuffle(element, options);
  }

  // Walks the subtree depth-first. Text nodes get split into per-char
  // spans; element nodes (e.g. `<b class="green-100">`) are kept intact
  // and recursed into so styling on inner spans survives the split.
  private splitChars(root: HTMLElement): HTMLElement[] {
    const chars: HTMLElement[] = [];
    const visit = (node: Node) => {
      // Snapshot child list — we mutate during iteration.
      const kids = Array.from(node.childNodes);
      for (const child of kids) {
        if (child.nodeType === Node.TEXT_NODE) {
          const text = child.textContent ?? "";
          const frag = document.createDocumentFragment();
          for (const ch of text) {
            if (ch === " " || ch === "\n" || ch === "\t") {
              frag.appendChild(document.createTextNode(ch));
              continue;
            }
            const span = document.createElement("span");
            span.className = "hacker-char";
            span.dataset.initial = ch;
            span.textContent = ch;
            frag.appendChild(span);
            chars.push(span);
          }
          child.parentNode?.replaceChild(frag, child);
        } else if (child.nodeType === Node.ELEMENT_NODE) {
          visit(child);
        }
      }
    };
    visit(root);
    return chars;
  }

  private shuffle(
    owner: HTMLElement,
    options: AttachOptions,
  ): void {
    // Query the DOM at trigger time so the effect survives textContent
    // swaps that wipe and rewrite the spans (e.g. carousel title swaps).
    const chars = Array.from(
      owner.querySelectorAll<HTMLElement>(".hacker-char"),
    );
    if (chars.length === 0) return;

    const iterations = options.iterations ?? 3;
    const charDelayMs = options.charDelayMs ?? 30;
    const staggerMs = options.staggerMs ?? 0;
    const myToken = (this.tokens.get(owner) ?? 0) + 1;
    this.tokens.set(owner, myToken);

    chars.forEach((char, i) => {
      let count = 0;
      const tick = () => {
        if (myToken !== this.tokens.get(owner)) return;
        if (count < iterations) {
          char.textContent =
            LETTERS_AND_SYMBOLS[Math.floor(Math.random() * LETTERS_AND_SYMBOLS.length)];
          count++;
          setTimeout(tick, charDelayMs);
        } else {
          char.textContent = char.dataset.initial ?? "";
        }
      };
      setTimeout(tick, i * staggerMs);
    });
  }
}
