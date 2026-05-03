import { type Frame, type Renderable } from "../renderer";

/**
 * Toggles a fixed top-bar's visibility based on whether an in-flow CTA
 * has scrolled past the viewport top. Also flips a corresponding shift
 * class on a content container so it recenters under the bar.
 *
 * The in-flow CTA is queried each frame because it lives inside cloned
 * fold content (added to the DOM after page load) and its position is
 * driven by the fold's translateY transform.
 */
export class StickyTopBar implements Renderable {
  private topBar: HTMLElement;
  private ctaSelector: string;
  private shiftTarget: HTMLElement | null;

  constructor(
    topBar: HTMLElement,
    ctaSelector: string,
    shiftTarget: HTMLElement | null = null,
  ) {
    this.topBar = topBar;
    this.ctaSelector = ctaSelector;
    this.shiftTarget = shiftTarget;
  }

  render(_frame: Frame): void {
    const cta = document.querySelector<HTMLElement>(this.ctaSelector);
    if (!cta) return;

    const past = cta.getBoundingClientRect().top <= 0;

    this.topBar.classList.toggle("header--active", past);
    this.shiftTarget?.classList.toggle("screen--shifted", past);
  }
}
