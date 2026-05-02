import { type Observable } from "../observer";

/**
 * Toggles a class on an element based on its viewport visibility.
 * Used to drive replay-on-scroll animations: removing the class when
 * the element leaves resets CSS animations; re-adding it when the
 * element re-enters runs them fresh from the start.
 */
export class PlayingToggle implements Observable {
  readonly element: HTMLElement;
  private className: string;

  constructor(element: HTMLElement, className: string = "playing") {
    this.element = element;
    this.className = className;
  }

  onIntersect(isIntersecting: boolean): void {
    this.element.classList.toggle(this.className, isIntersecting);
  }
}
