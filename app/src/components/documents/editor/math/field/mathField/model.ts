import type { MathfieldElement } from "mathlive";

/** The slice of MathLive's internal model `caretAt`, `wholeStructures`,
 *  `dropEmptyScript`, `atomsOf` and the matrix keys (`mathMatrixField.ts`)
 *  read. */
export interface MlAtom {
  type: string;
  command: string;
  value: string | undefined;
  mode: string;
  parent: MlAtom | undefined;
  parentBranch: unknown;
  captureSelection: boolean;
  leftSibling: MlAtom | undefined;
  rightSibling: MlAtom | undefined;
  environmentName?: string;
  /** A `leftright` atom's delimiters; `?` is a right one not yet typed. */
  leftDelim?: string;
  rightDelim?: string;
  /** An array's cells, row by row, each its atoms from a `first`. */
  rows?: (MlAtom[] | undefined)[][];
  body?: MlAtom[];
  /** Set, drops the cached LaTeX of the atom and its ancestors. */
  isDirty: boolean;
  hasChildren: boolean;
  /** Alone in its branch: for a `first` atom, the branch is empty. */
  hasNoSiblings: boolean;
  hasEmptyBranch(branch: string): boolean;
  branch(name: string): MlAtom[] | undefined;
}

export interface MlModel {
  at(offset: number): MlAtom | undefined;
  offsetOf(atom: MlAtom): number;
}

export function modelOf(mf: MathfieldElement): MlModel | null {
  const model = (mf as unknown as { _mathfield?: { model?: MlModel } })._mathfield?.model;
  return model && typeof model.at === "function" ? model : null;
}

/** The field's atoms in offset order, each as a comparable key. */
export function atomsOf(mf: MathfieldElement): string[] {
  const model = modelOf(mf);
  const keys: string[] = [];
  for (let i = 0; model && i <= mf.lastOffset; i++) {
    const a = model.at(i);
    keys.push(a ? `${a.type}\0${a.command}\0${a.value ?? ""}` : "");
  }
  return keys;
}
