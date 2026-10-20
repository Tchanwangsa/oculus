import type { CanvasViewport, Point } from './model';

const midpoint = (a: Point, b: Point): Point => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });
const distance = (a: Point, b: Point) => Math.hypot(b.x - a.x, b.y - a.y);

/** After two fingers navigate, remaining fingers stay navigation-only until all lift. */
export class TouchViewportGesture {
  private contacts = new Map<number, Point>();
  private baseline: { ids: number[]; center: Point; distance: number; viewport: CanvasViewport } | null = null;
  active = false;
  get count() { return this.contacts.size; }
  has(id: number) { return this.contacts.has(id); }

  private rebase(viewport: CanvasViewport) {
    const pair = [...this.contacts.entries()].slice(0, 2);
    if (pair.length < 2) { this.baseline = null; return; }
    this.baseline = { ids: pair.map(([id]) => id), center: midpoint(pair[0][1], pair[1][1]), distance: Math.max(1, distance(pair[0][1], pair[1][1])), viewport };
  }

  down(id: number, point: Point, viewport: CanvasViewport) {
    this.contacts.set(id, point);
    if (this.contacts.size === 2) { this.active = true; this.rebase(viewport); }
    return this.active;
  }

  move(id: number, point: Point): CanvasViewport | null {
    if (!this.contacts.has(id)) return null;
    this.contacts.set(id, point);
    const base = this.baseline;
    if (!base) return null;
    const a = this.contacts.get(base.ids[0]), b = this.contacts.get(base.ids[1]);
    if (!a || !b) return null;
    const center = midpoint(a, b);
    const zoom = Math.max(0.1, Math.min(8, base.viewport.zoom * distance(a, b) / base.distance));
    return {
      ...base.viewport, zoom,
      x: center.x - (base.center.x - base.viewport.x) * zoom / base.viewport.zoom,
      y: center.y - (base.center.y - base.viewport.y) * zoom / base.viewport.zoom,
    };
  }

  up(id: number, viewport: CanvasViewport) {
    this.contacts.delete(id);
    if (this.baseline?.ids.includes(id)) this.rebase(viewport);
    if (!this.contacts.size) this.reset();
  }

  reset() { this.contacts.clear(); this.baseline = null; this.active = false; }
}
