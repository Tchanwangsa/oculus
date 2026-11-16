import { useSyncExternalStore } from "react";
import type { Icon } from "@phosphor-icons/react";
import type { HarnessThread } from "@/lib/harness";

/**
 * Thread info's Work group: what a thread is making, one row per thing,
 * filled by the features that make them. A feature registers a kind once at
 * module load; the kind's hook reads its own state for the thread and
 * returns its rows. With no rows from any kind the group is not drawn.
 */

export interface WorkRow {
  key: string;
  icon: Icon;
  title: string;
  /** "Draft ready", "Rendering 40%": the thing's state, said in words. */
  status?: string;
  tone?: "muted" | "busy" | "ok" | "bad";
  /** The route the row opens beside the chat (a new tab on ⌘-click). */
  href: string;
}

export interface WorkKind {
  id: string;
  /** Called as a hook, once per render of the card, for the open thread. */
  useRows: (thread: HarnessThread) => WorkRow[];
}

let kinds: readonly WorkKind[] = [];
const listeners = new Set<() => void>();

/** Adds a kind, or replaces the one with its id (a hot reload); returns its
 *  removal. Kinds draw in the order they were first registered. */
export function registerWorkKind(kind: WorkKind): () => void {
  const at = kinds.findIndex((k) => k.id === kind.id);
  kinds = at < 0 ? [...kinds, kind] : kinds.map((k, i) => (i === at ? kind : k));
  listeners.forEach((l) => l());
  return () => {
    if (!kinds.includes(kind)) return;
    kinds = kinds.filter((k) => k !== kind);
    listeners.forEach((l) => l());
  };
}

export function workKinds(): readonly WorkKind[] {
  return kinds;
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useWorkKinds(): readonly WorkKind[] {
  return useSyncExternalStore(subscribe, workKinds);
}
