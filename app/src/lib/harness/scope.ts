import type { Subject } from "@/lib/db";

/**
 * A new Chat thread's scope as the student chose it: General (the whole
 * library) or one subject's id. Not having chosen is the absence of a value,
 * never `null` inside this type, so unchosen and General stay distinct. Rust
 * refuses to create a plain Chat thread without one (`SendOptions::chat_scope`).
 */
export type ChatScope = "general" | number;

export const GENERAL_SCOPE = "general" as const;

export function isChatScope(value: unknown): value is ChatScope {
  return value === GENERAL_SCOPE || (Number.isSafeInteger(value) && (value as number) > 0);
}

/** A route's or a stored value's scope, or null when it names neither. */
export function parseChatScope(raw: string | null | undefined): ChatScope | null {
  if (raw === GENERAL_SCOPE) return GENERAL_SCOPE;
  if (!raw || !/^\d+$/.test(raw)) return null;
  const id = Number(raw);
  return isChatScope(id) ? id : null;
}

/** The subject a scope files a thread under; null is General. */
export function scopeSubject(scope: ChatScope): number | null {
  return scope === GENERAL_SCOPE ? null : scope;
}

/** A stored scope that still names something: a deleted subject's id reads
 *  as unchosen, so the student picks again rather than sending into nothing. */
export function liveScope(
  scope: ChatScope | null | undefined,
  subjects: readonly Pick<Subject, "id">[],
): ChatScope | null {
  if (scope == null) return null;
  if (scope === GENERAL_SCOPE) return scope;
  return subjects.some((s) => s.id === scope) ? scope : null;
}
