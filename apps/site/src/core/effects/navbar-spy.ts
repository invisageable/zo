import { type Frame, type Renderable } from "../renderer";

export interface NavLink {
  link: HTMLElement;
  target: string;
}

interface Options {
  activeClass?: string;
  // Y-position (ratio of viewport height) above which a section's top must
  // pass to become "current". 0.25 = top 25vh.
  thresholdRatio?: number;
}

/**
 * Per-frame scroll-spy: finds which target section's top has most recently
 * crossed the threshold and toggles `activeClass` on the corresponding nav
 * link. Sections are looked up inside the fold-center clone so the spy
 * tracks the actual visible content (which is driven by the fold's
 * translateY animation, not native scroll).
 */
export class NavbarSpy implements Renderable {
  private items: NavLink[];
  private activeClass: string;
  private thresholdRatio: number;

  constructor(items: NavLink[], options: Options = {}) {
    this.items = items;
    this.activeClass = options.activeClass ?? "table-of-content-item--active";
    this.thresholdRatio = options.thresholdRatio ?? 0.25;
  }

  private findSection(target: string): HTMLElement | null {
    const centerScroller = document.querySelector<HTMLElement>(
      ".fold-center .fold-scroller",
    );
    return centerScroller?.querySelector<HTMLElement>(
      `[data-section="${target}"]`,
    ) ?? null;
  }

  render(_frame: Frame): void {
    const threshold = window.innerHeight * this.thresholdRatio;
    let currentIdx = -1;
    let bestDist = Infinity;

    for (let i = 0; i < this.items.length; i++) {
      const section = this.findSection(this.items[i].target);
      if (!section) continue;

      const rectTop = section.getBoundingClientRect().top;
      if (rectTop <= threshold) {
        const dist = threshold - rectTop;
        if (dist < bestDist) {
          bestDist = dist;
          currentIdx = i;
        }
      }
    }

    for (let i = 0; i < this.items.length; i++) {
      this.items[i].link.classList.toggle(this.activeClass, i === currentIdx);
    }
  }
}
