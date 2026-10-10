import type {
  FieldChange,
  FieldCommand,
  FieldEffect,
  FieldMode,
  FieldSlot,
  FieldSlotKind,
  FieldStep,
  MathField as RawField,
} from "../../../math-core/pkg/oculus_math.js";
import { callEngine, type Glue, mathsReady } from "./engine";

export type { FieldChange, FieldCommand, FieldEffect, FieldMode, FieldSlot, FieldSlotKind, FieldStep };

/** The glue's own unhooking: zeroes the object's pointer and drops it from
 *  the glue's finalizer, without a call into the instance. */
type Detachable = RawField & { __destroy_into_raw(): number };

/**
 * Frees an engine object. One of an instance that has trapped since is only
 * detached from the glue: a call into that instance traps again, and the
 * glue's own finalizer (wasm-bindgen registers every object) would make
 * that call once the object is collected.
 */
function release(raw: RawField, glue: Glue) {
  if (mathsReady()) {
    const freed = callEngine((current) => {
      if (current !== glue) return false;
      raw.free();
      return true;
    });
    if (freed) return;
  }
  (raw as Detachable).__destroy_into_raw();
}

/** Releases a collected field's object. Its held value keeps the object
 *  reachable, so the glue's finalizer cannot run before this does. */
const collected = new FinalizationRegistry<{ raw: RawField; glue: Glue }>(({ raw, glue }) => release(raw, glue));

/** What a command did (`FieldStep`), and the field after it. */
export interface Step extends FieldStep {
  field: MathField;
}

/**
 * One formula in the visual maths field: the edit model (`math-core/edit`)
 * over its LaTeX source, through the engine. Immutable: each step returns a
 * new `MathField`. Offsets are UTF-16 units (a string's indices, `data-s`/
 * `data-e`); stops and slots are ids into `stops` and `slots`.
 *
 * Its state is read out as plain data, and it keeps the engine's object
 * only to run the next step. That object dies when the engine traps (a new
 * instance takes over); the next step then opens the field again on the new
 * instance from its source, selection and pending command, forgetting only
 * the shortcut just typed (Esc no longer reverts it). The object is
 * released when this is collected, or by `free()`. Only once `mathsReady()`.
 */
export class MathField {
  readonly source: string;
  readonly display: boolean;
  readonly mode: FieldMode;
  /** The `\command` being typed, without its backslash. */
  readonly pending: string | undefined;
  /** The selection's anchor and head (the caret), as stop ids. */
  readonly anchor: number;
  readonly head: number;
  /** The selection's range of `source`: what copy takes. */
  readonly selected: readonly [number, number];
  /** Whether Space is the view's (its list of picks). */
  readonly spaceFree: boolean;
  /** Each stop's offset, by stop id: ←/→ order, never decreasing; stops of
   *  different slots can share one (`\frac ab`'s 7). */
  readonly stops: Uint32Array;
  /** Each stop's slot id, by stop id. */
  readonly stopSlots: Uint32Array;
  /** Every slot, by slot id, each before the slots inside it. */
  readonly slots: readonly FieldSlot[];
  #raw: RawField;
  /** The instance `#raw` belongs to; null once released. */
  #glue: Glue | null;

  private constructor(raw: RawField, glue: Glue) {
    this.#raw = raw;
    this.#glue = glue;
    collected.register(this, { raw, glue }, this);
    this.source = raw.source;
    this.display = raw.display;
    this.mode = raw.mode;
    this.pending = raw.pending;
    this.anchor = raw.anchor;
    this.head = raw.head;
    const [from, to] = raw.selected;
    this.selected = [from, to];
    this.spaceFree = raw.spaceFree;
    this.stops = raw.stops();
    this.stopSlots = raw.stopSlots();
    this.slots = raw.slots();
  }

  /** `source` with the caret at its end. Throws an `Error` named
   *  `ParseError` when it does not parse (edit it as TeX), and a
   *  `MathsTrap` when the engine traps on it. */
  static open(source: string, display: boolean): MathField {
    return callEngine((glue) => new MathField(glue.MathField.open(source, display), glue));
  }

  /** This field's object on `glue`, opened again if its own is gone. */
  #on(glue: Glue): RawField {
    if (this.#glue !== glue) {
      this.free();
      const opened = glue.MathField.open(this.source, this.display);
      const selected = opened.select(this.anchor, this.head);
      opened.free();
      this.#raw = selected.withPending(this.pending);
      selected.free();
      this.#glue = glue;
      collected.register(this, { raw: this.#raw, glue }, this);
    }
    return this.#raw;
  }

  #next(fn: (raw: RawField) => RawField): MathField {
    return callEngine((glue) => new MathField(fn(this.#on(glue)), glue));
  }

  /** The field with the caret at `offset` (after an undo or an outside
   *  edit): of the stops there, the first (the one going with the text
   *  before it) or, `after`, the last. Throws off a character boundary. */
  caretAt(offset: number, after = false): MathField {
    return this.#next((raw) => raw.caretAt(offset, after));
  }

  /** The field with this selection, widened so both ends share a slot. */
  select(anchor: number, head: number): MathField {
    return this.#next((raw) => raw.select(anchor, head));
  }

  /** The field with a pending `\command` (`""` right after `\`), or none. */
  withPending(name: string | undefined): MathField {
    return this.#next((raw) => raw.withPending(name));
  }

  /** Runs one key or input. Apply the step's `changes`, then its `rewrite`,
   *  each in reverse order, to `source` to get `field.source`. Throws a
   *  `TypeError` for a malformed command. */
  run(command: FieldCommand): Step {
    return callEngine((glue) => {
      const field = new MathField(this.#on(glue).run(command), glue);
      return { ...field.#raw.step!, field };
    });
  }

  /** Releases the engine's object now rather than when this is collected.
   *  The field stays usable: its next step opens it again. */
  free(): void {
    if (!this.#glue) return;
    collected.unregister(this);
    release(this.#raw, this.#glue);
    this.#glue = null;
  }
}

let shortcutTable: readonly (readonly [string, string])[] | null = null;

/** The field's shortcut table, `[keys, LaTeX]` pairs: typing the keys in
 *  maths gives the LaTeX (`#0` takes the selection, `#?` is an empty slot).
 *  Only once `mathsReady()`. */
export function fieldShortcuts(): readonly (readonly [string, string])[] {
  shortcutTable ??= callEngine((glue) => glue.shortcuts());
  return shortcutTable;
}
