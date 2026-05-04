export interface Observable {
  readonly element: HTMLElement;
  onIntersect(isIntersecting: boolean, entry: IntersectionObserverEntry): void;
}

export class Observer {
  private observables = new Map<HTMLElement, Observable>();
  private io: IntersectionObserver;

  constructor(options?: IntersectionObserverInit) {
    this.io = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        const obs = this.observables.get(entry.target as HTMLElement);
        if (!obs) continue;
        obs.onIntersect(entry.isIntersecting, entry);
      }
    }, options);
  }

  add(observable: Observable): this {
    this.observables.set(observable.element, observable);
    this.io.observe(observable.element);
    return this;
  }

  remove(observable: Observable): void {
    this.observables.delete(observable.element);
    this.io.unobserve(observable.element);
  }

  disconnect(): void {
    this.observables.clear();
    this.io.disconnect();
  }
}

export const observer = new Observer({ threshold: 0.3 });
