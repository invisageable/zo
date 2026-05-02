import { lerp } from "../easings/interpolation/lerp";
import { type Frame, type Renderable } from "../renderer";

const SCROLL_SPEED = 0.1;
const SCROLL_LIMIT = 0.0001;

export class FoldEffect implements Renderable {
  private folds: HTMLElement[];
  private centerFold: HTMLElement;
  private scrollers: HTMLDivElement[] = [];
  private scroll = 0;
  private targetScroll = 0;

  constructor(folds: HTMLElement[]) {
    this.folds = folds;
    this.centerFold = folds[Math.floor(folds.length / 2)];
  }

  setContent(baseContent: HTMLElement, createScrollers: boolean = true): void {
    const scrollers: HTMLDivElement[] = [];

    for (let i = 0; i < this.folds.length; i++) {
      const fold = this.folds[i];
      const copy = baseContent.cloneNode(true) as HTMLElement;
      copy.id = "";
      copy.classList.remove("fold-source");

      let scroller: HTMLDivElement;

      if (createScrollers) {
        const sizeFix = document.createElement("div");
        sizeFix.classList.add("fold-size-fix");

        scroller = document.createElement("div");
        scroller.classList.add("fold-scroller");
        sizeFix.append(scroller);
        fold.append(sizeFix);
      } else {
        scroller = this.scrollers[i];
      }

      scroller.append(copy);
      scrollers[i] = scroller;
    }

    this.scrollers = scrollers;
  }

  render(_frame: Frame): void {
    if (this.scrollers.length === 0) return;

    const firstContent = this.scrollers[0].children[0] as HTMLElement;

    document.body.style.height =
      firstContent.clientHeight -
      this.centerFold.clientHeight +
      window.innerHeight +
      "px";

    this.targetScroll = -(
      document.documentElement.scrollTop || document.body.scrollTop
    );

    this.scroll += lerp(
      this.scroll,
      this.targetScroll,
      SCROLL_SPEED,
      SCROLL_LIMIT,
    );

    for (const scroller of this.scrollers) {
      const content = scroller.children[0] as HTMLElement;
      content.style.transform = `translateY(${this.scroll}px)`;
    }
  }
}
