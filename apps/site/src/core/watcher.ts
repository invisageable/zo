export interface Watchable {
  readonly selector: string;
  onMatch(element: HTMLElement): void;
}

/**
 * Watches the document for elements matching a selector — both those
 * present at registration time AND those inserted later by any DOM
 * mutation. Use this when a component needs to attach behavior to
 * elements that may be cloned/injected after the script first runs
 * (e.g., charts inside the fold's cloned content).
 */
export class Watcher {
  private watchables: Watchable[] = [];
  private mo: MutationObserver | null = null;

  add(watchable: Watchable): this {
    this.watchables.push(watchable);

    // Initial sweep for elements already in the DOM.
    document.querySelectorAll<HTMLElement>(watchable.selector).forEach((el) => {
      watchable.onMatch(el);
    });

    // Start the MutationObserver lazily on first add.
    this.ensureObserving();

    return this;
  }

  private ensureObserving(): void {
    if (this.mo !== null) return;

    this.mo = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        for (const node of mutation.addedNodes) {
          if (!(node instanceof HTMLElement)) continue;

          for (const watchable of this.watchables) {
            // Node IS a match.
            if (node.matches(watchable.selector)) watchable.onMatch(node);
            // Node CONTAINS matches (querySelectorAll skips the root).
            node.querySelectorAll<HTMLElement>(watchable.selector).forEach((el) => {
              watchable.onMatch(el);
            });
          }
        }
      }
    });

    this.mo.observe(document.body, { childList: true, subtree: true });
  }

  disconnect(): void {
    this.mo?.disconnect();
    this.mo = null;
    this.watchables = [];
  }
}

export const watcher = new Watcher();
