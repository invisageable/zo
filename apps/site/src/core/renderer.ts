export interface Frame {
  time: number;
}

export interface Renderable {
  render(frame: Frame): void;
}

export class Renderer {
  private renderables: Renderable[] = [];
  private rafId: number | null = null;

  add(renderable: Renderable): this {
    this.renderables.push(renderable);

    return this;
  }

  remove(renderable: Renderable): void {
    const idx = this.renderables.indexOf(renderable);

    if (idx !== -1) this.renderables.splice(idx, 1);
  }

  render(): void {
    if (this.rafId !== null) return;

    const tick = (time: number) => {
      const frame: Frame = { time };
      
      for (const renderable of this.renderables) renderable.render(frame);
      
      this.rafId = window.requestAnimationFrame(tick);
    };

    this.rafId = window.requestAnimationFrame(tick);
  }

  stop(): void {
    if (this.rafId === null) return;

    window.cancelAnimationFrame(this.rafId);

    this.rafId = null;
  }

  /**
   * Drop every renderable and stop the rAF loop. Called by the global
   * view-transitions lifecycle hook before each page swap so renderables
   * pointing at soon-to-be-replaced DOM nodes don't keep ticking.
   */
  reset(): void {
    this.stop();
    this.renderables = [];
  }
}

export const renderer = new Renderer();
