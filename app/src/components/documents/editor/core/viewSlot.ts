/**
 * Holds the one editor view a mount point may have. A view that outlives its
 * effect (a cleanup that threw before `destroy`, a remount racing the old
 * unmount) would stay attached beside its successor as a second, live text
 * box; `mount` destroys whatever is still held before making the next, and
 * `unmount` destroys even when the caller's own cleanup failed first. A view
 * is destroyed once however many of them reach it.
 */
export class ViewSlot<V extends { destroy(): void }> {
  private held: V | null = null;
  private gone = new WeakSet<V>();

  /** Destroys any view still held, then makes and holds the next. */
  mount(make: () => V): V {
    const stale = this.held;
    this.held = null;
    try {
      if (stale) this.destroy(stale);
    } catch (e) {
      // The next view matters more than a stale one's failed teardown.
      console.error("stale editor view failed to destroy", e);
    }
    this.held = make();
    return this.held;
  }

  /** Destroys `view`, and lets go of it if it is still the held one. */
  unmount(view: V): void {
    try {
      this.destroy(view);
    } finally {
      if (this.held === view) this.held = null;
    }
  }

  private destroy(view: V) {
    if (this.gone.has(view)) return;
    this.gone.add(view);
    view.destroy();
  }
}
