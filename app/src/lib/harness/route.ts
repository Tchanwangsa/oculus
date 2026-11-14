import { parseChatScope, type ChatScope } from "./scope";

/**
 * The route for a Chat tab showing one conversation, or with no id the empty
 * composer. The thread lives only in the route: every tab has its own router
 * (`TabPane`), so two tabs hold two conversations and back walks between them.
 * `n` is the name, because `tabInfo` titles a tab from its path alone. `s` on
 * the empty composer is an entry point's scope (a group's "+"): the page
 * seeds the new chat's choice from it once and drops it from the route.
 */
export function chatHref(
  threadId?: number | null,
  title?: string | null,
  opts?: { subject?: ChatScope },
): string {
  if (threadId == null) return opts?.subject != null ? `/chat?s=${opts.subject}` : "/chat";
  const name = title?.trim();
  return name ? `/chat?t=${threadId}&n=${encodeURIComponent(name)}` : `/chat?t=${threadId}`;
}

/** The thread id a Chat route names, or null for the empty composer. */
export function chatThreadId(search: string): number | null {
  const raw = new URLSearchParams(search).get("t");
  const id = raw == null ? NaN : Number(raw);
  return Number.isInteger(id) && id > 0 ? id : null;
}

/** The scope a new-chat route asks to start in, or null when it names none. */
export function chatScope(search: string): ChatScope | null {
  return parseChatScope(new URLSearchParams(search).get("s"));
}
