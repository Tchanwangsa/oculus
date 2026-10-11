import { isProvider } from "./providers";
import type { ErrorMeta, HarnessItem, ThreadUsage, HarnessThread, ToolMeta } from "./types";

const TOOL_KINDS = ["read", "edit", "write", "bash", "search", "oculus_cli", "task", "web", "plan", "other"] as const;
export type ToolKind = (typeof TOOL_KINDS)[number];

/** "Ran", "Read", "Edited" — past tense once done. Shared with the chapter
 *  panel so both speak one vocabulary. */
export function toolVerb(kind: ToolKind, done: boolean, name?: string | null): string {
  // Search and fetch are both `web`; only the tool name tells them apart.
  if (kind === "web" && name && /search/i.test(name)) return done ? "Searched" : "Searching";
  switch (kind) {
    case "read": return done ? "Read" : "Reading";
    case "edit": return done ? "Edited" : "Editing";
    case "write": return done ? "Wrote" : "Writing";
    case "bash": return done ? "Ran" : "Running";
    case "search": return done ? "Searched" : "Searching";
    case "oculus_cli": return done ? "Looked up" : "Looking up";
    case "task": return done ? "Ran subagent" : "Running subagent";
    case "web": return done ? "Fetched" : "Fetching";
    case "plan": return done ? "Updated plan" : "Updating plan";
    default: return done ? "Used" : "Using";
  }
}

export function parseUsage(t: HarnessThread | null): ThreadUsage | null {
  if (!t?.usage) return null;
  try {
    return JSON.parse(t.usage);
  } catch {
    return null;
  }
}

/** Row objects are replaced on change, so parsed metadata invalidates itself.
 *  Non-object JSON is rejected before any row reads its fields. */
const ITEM_META = new WeakMap<HarnessItem, Record<string, unknown>>();

export function parseItemMeta(item: HarnessItem): Record<string, unknown> {
  const hit = ITEM_META.get(item);
  if (hit) return hit;
  let meta: Record<string, unknown> = {};
  try {
    const parsed: unknown = JSON.parse(item.meta ?? "{}");
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      meta = parsed as Record<string, unknown>;
    }
  } catch {
    // Invalid metadata leaves the row readable, without actions to approve.
  }
  ITEM_META.set(item, meta);
  return meta;
}

const TOOL_META = new WeakMap<HarnessItem, ToolMeta>();

export function parseToolMeta(item: HarnessItem): ToolMeta {
  const hit = TOOL_META.get(item);
  if (hit) return hit;
  const meta: Record<string, unknown> = { ...parseItemMeta(item) };
  // An unknown kind has no icon; invalid outputs cannot be rendered as text.
  if (!TOOL_KINDS.includes(meta.kind as ToolKind)) delete meta.kind;
  if (typeof meta.name !== "string") delete meta.name;
  if (meta.ok != null && typeof meta.ok !== "boolean") delete meta.ok;
  if (meta.output != null && typeof meta.output !== "string") delete meta.output;
  TOOL_META.set(item, meta as ToolMeta);
  return meta as ToolMeta;
}

/** The playhead second a lecture-dock question was asked at, from the user
 *  row's `meta` (`{"at": 220}`); null elsewhere. */
export function messageAt(item: HarnessItem): number | null {
  const { at } = parseItemMeta(item);
  return typeof at === "number" ? at : null;
}

/** The agent a credentials-failure error row is about (`{"auth":"claude"}`);
 *  an unknown provider reads as an ordinary error. */
export function parseErrorMeta(item: HarnessItem): ErrorMeta {
  const { auth } = parseItemMeta(item);
  return isProvider(auth) ? { auth } : {};
}

/** A `permission` row's `meta`. A row that does not parse draws as a refusal
 *  with nothing to approve, never a malformed rule. */
export interface PermissionMeta {
  tool?: string;
  action?: string;
  target?: string | null;
  rule?: string | null;
}

export function parsePermissionMeta(item: HarnessItem): PermissionMeta {
  const m = parseItemMeta(item);
  const str = (v: unknown) => (typeof v === "string" && v ? v : null);
  return {
    tool: str(m.tool) ?? undefined,
    action: str(m.action) ?? undefined,
    target: str(m.target),
    rule: str(m.rule),
  };
}

/** An `agy` rule taken apart; null outside the shapes `is_valid_rule` in
 *  `app/src-tauri/src/harness/providers/antigravity/rules.rs` accepts. */
export function splitRule(
  rule: string,
): { action: "command" | "read_file" | "write_file" | "read_url"; value: string } | null {
  const m = /^(command|read_file|write_file|read_url)\((.+)\)$/.exec(rule);
  if (!m) return null;
  return { action: m[1] as "command" | "read_file" | "write_file" | "read_url", value: m[2] };
}
