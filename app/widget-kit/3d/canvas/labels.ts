/**
 * The labels a canvas shows over its scene. `Label3D` sits in the scene's
 * tree but draws nothing there: it registers here, the canvas's label layer
 * draws each one as DOM, and every frame `place` moves them to where their
 * points project. While the canvas is released the list is frozen, so the
 * labels stay over the still at their last positions.
 */
import { createContext } from "react";
import { Vector3, type Camera } from "three";
import type { Colour } from "../../colors";
import type { Vec3 } from "../geometry/vec";

export type Attach = "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "nw";

export interface LabelSpec {
  at: Vec3;
  source: string;
  attach?: Attach;
  color?: Colour;
}

export class LabelRegistry {
  private live: LabelSpec[] = [];
  private shown: LabelSpec[] = [];
  private frozen = false;
  private elements: (HTMLElement | null)[] = [];
  private listeners = new Set<() => void>();
  private point = new Vector3();

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => void this.listeners.delete(listener);
  };

  /** What the layer draws, in order: live labels, or the frozen ones. */
  snapshot = () => this.shown;

  add(spec: LabelSpec) {
    this.live.push(spec);
    this.publish();
  }

  remove(spec: LabelSpec) {
    this.live = this.live.filter((s) => s !== spec);
    this.publish();
  }

  /** A label's text, side or colour changed (its point is read each frame). */
  changed() {
    this.publish();
  }

  /** Keeps the labels shown while the scene that places them is gone. */
  freeze() {
    this.frozen = true;
  }

  /** The scene is drawing again: show what it has registered. */
  thaw() {
    if (!this.frozen) return;
    this.frozen = false;
    this.publish();
  }

  bind(index: number, element: HTMLElement | null) {
    this.elements[index] = element;
  }

  /** Moves each label to where its point is seen; one behind the camera hides. */
  place(camera: Camera, width: number, height: number) {
    camera.updateMatrixWorld();
    this.shown.forEach((spec, i) => {
      const el = this.elements[i];
      if (!el) return;
      const p = this.point.set(spec.at[0], spec.at[1], spec.at[2]).project(camera);
      const seen = Number.isFinite(p.x) && Number.isFinite(p.y) && p.z >= -1 && p.z <= 1;
      el.style.visibility = seen ? "visible" : "hidden";
      if (seen) el.style.transform = `translate(${((p.x + 1) / 2) * width}px, ${((1 - p.y) / 2) * height}px)`;
    });
  }

  private publish() {
    if (this.frozen) return;
    this.shown = [...this.live];
    for (const listener of [...this.listeners]) listener();
  }
}

/** The registry of the canvas a part is drawn in, null outside one. */
export const LabelContext = createContext<LabelRegistry | null>(null);
