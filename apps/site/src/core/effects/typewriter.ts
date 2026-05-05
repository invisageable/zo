interface PlayOptions {
  charDelay?: number;
  charsPerTick?: number;
  // Target total animation duration in ms. When set, overrides
  // charsPerTick to evenly spread the reveal over this window.
  totalMs?: number;
}

/**
 * Reveals an element's text content char-by-char. Walks every text node in
 * document order, blanks them, then types each one out — markup (spans for
 * syntax highlighting, links, etc.) stays intact, only the text inside
 * animates.
 *
 * One Typewriter instance owns one cancellation token: calling `play()`
 * again — even on a different element — cancels any in-flight animation.
 */
export class Typewriter {
  // Per-element token so concurrent plays on different elements don't
  // cancel each other. Calling `play()` again on the SAME element bumps
  // its token and the previous in-flight tick exits.
  private tokens = new WeakMap<HTMLElement, number>();

  play(element: HTMLElement, options: PlayOptions = {}): void {
    const charDelay = options.charDelay ?? 8;
    const myToken = (this.tokens.get(element) ?? 0) + 1;
    this.tokens.set(element, myToken);

    const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
    const nodes: Text[] = [];
    let n: Node | null;
    while ((n = walker.nextNode())) nodes.push(n as Text);

    const originals = nodes.map((node) => node.textContent ?? "");
    nodes.forEach((node) => { node.textContent = ""; });

    // If `totalMs` is set, divide the total chars by the number of ticks
    // (totalMs / charDelay) to get how many to reveal per tick. Caller's
    // explicit `charsPerTick` only used when totalMs is absent.
    const totalChars = originals.reduce((sum, t) => sum + t.length, 0);
    const charsPerTick = options.totalMs
      ? Math.max(1, Math.ceil(totalChars / Math.max(1, options.totalMs / charDelay)))
      : Math.max(1, options.charsPerTick ?? 1);

    let nodeIdx = 0;
    let charIdx = 0;
    const tick = () => {
      if (myToken !== this.tokens.get(element)) return;
      // Advance up to `charsPerTick` characters per tick, possibly across
      // multiple text nodes if the current one runs out.
      let budget = charsPerTick;
      while (budget > 0 && nodeIdx < nodes.length) {
        const text = originals[nodeIdx];
        const remaining = text.length - charIdx;
        if (remaining <= 0) {
          nodes[nodeIdx].textContent = text;
          nodeIdx++;
          charIdx = 0;
          continue;
        }
        const take = Math.min(remaining, budget);
        charIdx += take;
        nodes[nodeIdx].textContent = text.slice(0, charIdx);
        budget -= take;
      }
      if (nodeIdx < nodes.length) setTimeout(tick, charDelay);
    };
    tick();
  }

  cancel(element: HTMLElement): void {
    this.tokens.set(element, (this.tokens.get(element) ?? 0) + 1);
  }
}
