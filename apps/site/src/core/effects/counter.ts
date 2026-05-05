import { type Frame, type Renderable } from "../renderer";
import { type Observable } from "../observer";

/**
 * Animates an element's textContent from 0 up to a target value when the
 * element enters the viewport. Leaving the viewport resets the value so
 * re-entry replays the count from the start — same restart contract as
 * PlayingToggle, but for JS-driven number tweens instead of CSS keyframes.
 */
export class Counter implements Renderable, Observable {
  readonly element: HTMLElement;
  private target: number;
  private startValue: number = 0;
  private duration: number;
  private startTime: number | null = null;
  private playing = false;

  constructor(
    element: HTMLElement,
    target: number,
    duration: number = 1500,
  ) {
    this.element = element;
    this.target = target;
    this.duration = duration;
    this.element.textContent = "0";
  }

  onIntersect(isIntersecting: boolean): void {
    if (isIntersecting) {
      this.startValue = 0;
      this.playing = true;
      this.startTime = null;
    } else {
      this.playing = false;
      this.element.textContent = "0";
    }
  }

  // Animate from the currently-displayed value to a new target. Used by
  // pages that drive the counter from a non-scroll signal (e.g. a filter
  // change) instead of viewport intersection.
  setTarget(target: number): void {
    this.startValue = Number(this.element.textContent ?? 0) || 0;
    this.target = target;
    this.startTime = null;
    this.playing = true;
  }

  render(frame: Frame): void {
    if (!this.playing) return;
    if (this.startTime === null) this.startTime = frame.time;

    const elapsed = frame.time - this.startTime;
    const progress = Math.min(elapsed / this.duration, 1);
    // easeOutCubic — fast first, settles at the end.
    const eased = 1 - Math.pow(1 - progress, 3);

    const value = this.startValue + eased * (this.target - this.startValue);
    this.element.textContent = String(Math.floor(value));

    if (progress >= 1) this.playing = false;
  }
}
