import { setSubjectSelected } from "@/lib/db";

export const SUBJECT_SELECTION_CHANGED_EVENT = "oculus:subject-selection-changed";

/** One write order for every mounted Sync tab. Only in-flight edits overlay
 * database reads; a committed edit must not mask a later change from elsewhere. */
export class SubjectSelectionWrites {
  private tail: Promise<void> = Promise.resolve();
  private pending = new Map<number, { selected: boolean; revision: number }>();
  private version = 0;

  constructor(
    private persist: (id: number, selected: boolean) => Promise<void>,
    private changed: () => void = () => {},
  ) {}

  get revision(): number { return this.version; }

  selected(id: number, fallback: boolean): boolean {
    return this.pending.get(id)?.selected ?? fallback;
  }

  /** Null means this read predates an edit or commit and must be repeated. */
  reconcile(rows: readonly { id: number; selected: boolean }[], revision: number): Set<number> | null {
    if (revision !== this.version) return null;
    return new Set(rows.filter((row) => this.selected(row.id, row.selected)).map((row) => row.id));
  }

  set(id: number, selected: boolean): Promise<void> {
    const revision = ++this.version;
    this.pending.set(id, { selected, revision });
    this.changed();
    const finish = () => {
      if (this.pending.get(id)?.revision === revision) this.pending.delete(id);
      this.version++;
      this.changed();
    };
    this.tail = this.tail.catch(() => {}).then(() => this.persist(id, selected)).then(
      finish,
      (reason) => { finish(); throw reason; },
    );
    return this.tail;
  }

  /** Sync must wait for checkbox writes from every tab, including later edits
   * appended while an earlier write is still committing. */
  async drain(): Promise<void> {
    for (;;) {
      const pending = this.tail;
      try {
        await pending;
      } catch (reason) {
        if (pending === this.tail) throw reason;
      }
      if (pending === this.tail) return;
    }
  }
}

export const subjectSelectionWrites = new SubjectSelectionWrites(
  setSubjectSelected,
  () => window.dispatchEvent(new Event(SUBJECT_SELECTION_CHANGED_EVENT)),
);
