import { providerLabel } from "./registry";
import type { HarnessRequest, RequestOption, ToolReview } from "./types";

/**
 * What a request row, the composer's answer bar and the sidebar show for an
 * agent's request (`HarnessRequest`, docs/harness.md "Requests"). Rust owns
 * the state; these only read it, so every choice here is testable without
 * a DOM.
 */

/** One file a change request would touch. `kind` is the CLI's: `add`,
 *  `update` or `delete`, with `moveTo` for a rename. */
export interface RequestChange {
  path: string;
  kind: string;
  moveTo: string | null;
  diff: string;
}

/** A request's details, per action, ready to draw. */
export interface RequestDetail {
  /** "Codex wants to run a command". */
  title: string;
  /** The child agent that asked, by its last path segment; null for the chat's own agent. */
  agent: string | null;
  /** A command or network request: the whole command, never shortened. */
  command: string | null;
  cwd: string | null;
  /** Only when Rust says the command runs outside the sandbox. */
  outsideSandbox: boolean;
  /** The command needs the network (Codex's network escalation). */
  network: boolean;
  /** A host the agent asks to reach with no command (Claude's
   *  `SandboxNetworkAccess`); listed in `access` too. */
  host: string | null;
  changes: RequestChange[];
  /** A permissions request: one line per access asked for. */
  access: string[];
  /** The agent's own reason, as it gave it. */
  reason: string | null;
  /** One short line naming what is asked, for the composer bar and a
   *  settled row: the command, the file, or the access. */
  subject: string;
}

const text = (v: unknown): string | null => (typeof v === "string" && v.trim() ? v : null);

/** A child agent's name from its path (`/root/child_probe` → `child_probe`). */
export function agentName(payload: Record<string, unknown>): string | null {
  const agent = payload.agent as { path?: unknown } | undefined;
  const path = text(agent?.path);
  if (!path) return null;
  return path.split("/").filter(Boolean).pop() ?? path;
}

function changesOf(payload: Record<string, unknown>): RequestChange[] {
  if (!Array.isArray(payload.changes)) return [];
  return payload.changes.flatMap((c): RequestChange[] => {
    if (!c || typeof c !== "object") return [];
    const r = c as Record<string, unknown>;
    const path = text(r.path);
    if (!path) return [];
    const kind = r.kind as Record<string, unknown> | string | undefined;
    const type = typeof kind === "string" ? kind : text(kind?.type) ?? "update";
    const moveTo = typeof kind === "object" ? text(kind?.move_path) : null;
    return [{ path, kind: type, moveTo, diff: typeof r.diff === "string" ? r.diff : "" }];
  });
}

/** The access a permissions request asks for, one line each: a write, a
 *  read, the network. Entries and the flat lists say the same thing; each
 *  path is named once at its widest access. */
export function accessLines(permissions: unknown): string[] {
  if (!permissions || typeof permissions !== "object") return [];
  const p = permissions as Record<string, unknown>;
  const fs = (p.fileSystem ?? {}) as Record<string, unknown>;
  const write = new Set<string>();
  const read = new Set<string>();
  const list = (v: unknown) => (Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : []);
  for (const path of list(fs.write)) write.add(path);
  for (const path of list(fs.read)) read.add(path);
  if (Array.isArray(fs.entries)) {
    for (const e of fs.entries) {
      const entry = e as { path?: { path?: unknown } | string; access?: unknown };
      const path = typeof entry?.path === "string" ? entry.path : text(entry?.path?.path);
      if (!path) continue;
      if (entry.access === "read") read.add(path);
      else write.add(path);
    }
  }
  const lines = [...write].map((path) => `Write to ${path}`);
  for (const path of read) if (!write.has(path)) lines.push(`Read ${path}`);
  if (p.network && typeof p.network === "object" && (p.network as { enabled?: unknown }).enabled !== false) {
    lines.push("Reach the network");
  }
  return lines;
}

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
const baseName = (path: string) => path.split("/").filter(Boolean).pop() ?? path;

/** What the row and the bar say about a request, from its payload. */
export function requestDetail(request: HarnessRequest): RequestDetail {
  const p = request.payload ?? {};
  const who = providerLabel(request.provider);
  const agent = agentName(p);
  const changes = request.action === "file_change" ? changesOf(p) : [];
  const command = request.action === "command" || request.action === "network" ? text(p.command) : null;
  const host = request.action === "network" ? text(p.host) : null;
  const access = request.action === "permissions" ? accessLines(p.permissions) : host ? [`Reach ${host}`] : [];
  const network = request.action === "network" && !host;
  const asker = agent ? `${who}'s subagent ${agent}` : who;
  let title: string;
  let subject: string;
  switch (request.action) {
    case "command":
      title = `${asker} wants to run a command`;
      subject = command ?? "A command";
      break;
    case "network":
      title = `${asker} wants to reach the network`;
      subject = command ?? host ?? "Network access";
      break;
    case "file_change":
      title = `${asker} wants to change ${changes.length === 1 ? "a file" : plural(changes.length, "file")}`;
      subject = changes.length === 1 ? baseName(changes[0].path) : changes.map((c) => baseName(c.path)).join(", ") || "Files";
      break;
    case "permissions":
      title = `${asker} wants more access`;
      subject = access.join(", ") || "More access";
      break;
    default:
      title = `${asker} asks for approval`;
      subject = "A request";
  }
  return {
    title,
    agent,
    command,
    cwd: text(p.cwd),
    outsideSandbox: p.outsideSandbox === true,
    network,
    host,
    changes,
    access,
    reason: text(p.reason),
    subject,
  };
}

/** A file change's kind in words. */
export function changeKindLabel(change: RequestChange): string {
  if (change.moveTo) return `Move to ${change.moveTo}`;
  switch (change.kind) {
    case "add":
      return "New file";
    case "delete":
      return "Delete";
    default:
      return "Edit";
  }
}

/** One line of a unified diff and how it is coloured. */
export interface DiffLine {
  text: string;
  tone: "add" | "remove" | "hunk" | "context";
}

export function diffLines(diff: string): DiffLine[] {
  const lines = diff.replace(/\n$/, "").split("\n");
  return lines.map((line) => ({
    text: line,
    tone: line.startsWith("@@")
      ? "hunk"
      : line.startsWith("+") && !line.startsWith("+++")
        ? "add"
        : line.startsWith("-") && !line.startsWith("---")
          ? "remove"
          : "context",
  }));
}

/** A request the student can still answer: pending, on a live route. */
export const isAnswerable = (r: HarnessRequest) => r.status === "pending" && r.responder === "live";

/** How a request stands, as its row says it. */
export interface RequestState {
  /** `waiting` draws the full card; the rest a quiet settled row. */
  kind: "waiting" | "approved" | "declined" | "expired" | "cancelled";
  /** "Waiting for approval", "Approved", "Always allowed", "Declined", "Expired", "Cancelled". */
  label: string;
  /** Said under it: why it ended, an option's scope, what delivery knows. */
  notes: string[];
  /** Delivery is in doubt, or failed and needs answering again. */
  warn: boolean;
}

/** The option a request was answered with, if Rust recorded one it offers. */
export function chosenOption(request: HarnessRequest): RequestOption | null {
  return request.options.find((o) => o.id === request.response) ?? null;
}

/** How a request stands: waiting (with a failed delivery said), or how it
 *  was settled with the reason and what is known of the answer's delivery. */
export function requestState(request: HarnessRequest): RequestState {
  const who = providerLabel(request.provider);
  if (request.status === "pending") {
    const failed = request.delivery === "failed";
    return {
      kind: "waiting",
      label: "Waiting for approval",
      notes: failed ? [`Your answer did not reach ${who}. Answer again.`] : [],
      warn: failed,
    };
  }
  if (request.status === "expired" || request.status === "cancelled") {
    const uncertain = request.delivery === "uncertain";
    return {
      kind: request.status,
      label: request.status === "expired" ? "Expired" : "Cancelled",
      notes: [request.reason ?? (request.status === "expired" ? "It can no longer be answered." : "It was cancelled."),
        ...(uncertain ? [uncertainNote(who)] : [])],
      warn: uncertain,
    };
  }
  const option = chosenOption(request);
  const decision = option?.decision ?? "accept";
  const notes: string[] = [];
  if (decision === "accept_for_session" && option?.scope) notes.push(option.scope);
  if (request.delivery === "sending") notes.push(`Sending your answer to ${who}…`);
  if (request.delivery === "uncertain") notes.push(uncertainNote(who));
  return {
    kind: decision === "decline" ? "declined" : "approved",
    label: decision === "decline" ? "Declined" : decision === "accept_for_session" ? "Always allowed" : "Approved",
    notes,
    warn: request.delivery === "uncertain",
  };
}

const uncertainNote = (who: string) =>
  `Delivery uncertain: the connection to ${who} broke while your answer was sent, so it may not have arrived. It was not sent again.`;

/** The answers the composer offers for a request: Decline and Approve
 *  inline, any answer that reaches past this call (a session grant) under
 *  More with its scope. None for a request that cannot be answered. */
export interface RequestActions {
  decline: RequestOption | null;
  approve: RequestOption | null;
  more: RequestOption[];
}

export function requestActions(request: HarnessRequest): RequestActions {
  if (!isAnswerable(request)) return { decline: null, approve: null, more: [] };
  return {
    decline: request.options.find((o) => o.decision === "decline") ?? null,
    approve: request.options.find((o) => o.decision === "accept") ?? null,
    more: request.options.filter((o) => o.decision === "accept_for_session"),
  };
}

/** A thread's requests the student can answer, oldest first: the composer
 *  answers the first, and says how many wait. */
export function waitingRequests(requests: Record<number, HarnessRequest> | undefined): HarnessRequest[] {
  if (!requests) return [];
  return Object.values(requests)
    .filter(isAnswerable)
    .sort((a, b) => a.id - b.id);
}

/** Whether a request waits on the student, in the thread or in one run. */
export function awaitsApproval(requests: Record<number, HarnessRequest> | undefined, runId?: number): boolean {
  if (!requests) return false;
  for (const r of Object.values(requests)) {
    if (isAnswerable(r) && (runId === undefined || r.run_id === runId)) return true;
  }
  return false;
}

/** A decision the CLI made on a tool row without asking: Auto's reviewer
 *  (Codex's, or Claude's classifier, `source: "classifier"`), or a deny
 *  rule that holds in every mode (`source: "rule"`). */
export interface ReviewView {
  label: string;
  tone: "muted" | "danger";
  /** "Low risk"; null when the reviewer gave none. */
  risk: string | null;
  rationale: string | null;
  /** Why nobody was asked, said after the label when the row is opened. */
  note: string;
}

const REVIEW_WORDS: Record<string, Record<string, [string, ReviewView["tone"]]>> = {
  classifier: {
    approved: ["Approved by Claude's classifier", "muted"],
    denied: ["Refused by Claude's classifier", "danger"],
  },
  rule: {
    denied: ["Refused by a rule", "danger"],
  },
  reviewer: {
    approved: ["Reviewer approved", "muted"],
    denied: ["Reviewer denied", "danger"],
    in_progress: ["Reviewing", "muted"],
    timed_out: ["Review timed out", "danger"],
    aborted: ["Review stopped", "muted"],
  },
};

export function reviewView(review: ToolReview | undefined | null): ReviewView | null {
  if (!review || typeof review !== "object") return null;
  const risk = text(review.risk);
  const source = review.source === "classifier" || review.source === "rule" ? review.source : "reviewer";
  const [label, tone] = REVIEW_WORDS[source][review.status] ?? ["Reviewed", "muted"];
  return {
    label,
    tone,
    risk: risk ? `${risk.charAt(0).toUpperCase()}${risk.slice(1)} risk` : null,
    rationale: text(review.rationale),
    note: source === "rule" ? "Oculus keeps agents out of this in every mode" : "you were not asked (Auto)",
  };
}
