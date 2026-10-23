import {
  describeOculusCli,
  parseToolMeta,
  thoughtLabel,
  thoughtMs,
  type CliAction,
  type CliIcon,
  type HarnessItem,
  type ToolKind,
} from "@/lib/harness";

export type ViewRow =
  | { kind: "item"; item: HarnessItem; dim: boolean }
  | { kind: "bundle"; id: string; items: HarnessItem[] };

const isWork = (i: HarnessItem) => i.kind === "tool" || i.kind === "thinking";

export function buildRows(items: HarnessItem[], running: boolean): ViewRow[] {
  const out: ViewRow[] = [];
  let step: HarnessItem[] = [];
  const close = (dim: boolean) => {
    if (step.length >= 2 && dim) out.push({ kind: "bundle", id: `b-${step[0].id}`, items: step });
    else for (const item of step) out.push({ kind: "item", item, dim });
    step = [];
  };
  for (const item of items) {
    if (isWork(item)) step.push(item);
    else {
      close(true);
      out.push({ kind: "item", item, dim: false });
    }
  }
  // The trailing step is live while the turn runs.
  close(!running);
  return out;
}

function plural(n: number, one: string, many = `${one}s`) {
  return `${n} ${n === 1 ? one : many}`;
}

/** A bundle's glyph: a tool kind's, an oculus command's, or none for thoughts alone. */
export type BundleIcon = { tool: ToolKind } | { cli: CliIcon } | null;

/** "Explored 3 files, saved 2 memories, ran 1 command" */
export function bundleLabel(items: HarnessItem[]): { label: string; icon: BundleIcon } {
  const counts: Partial<Record<ToolKind | "thinking", number>> = {};
  // Oculus commands tally by what they did, in the order first seen.
  const cli = new Map<string, { action: CliAction; n: number }>();
  for (const i of items) {
    const k = i.kind === "thinking" ? "thinking" : (parseToolMeta(i).kind ?? "other");
    counts[k] = (counts[k] ?? 0) + 1;
    if (k !== "oculus_cli") continue;
    for (const action of describeOculusCli(i.content ?? "")) {
      const hit = cli.get(action.key);
      if (hit) hit.n += 1;
      else cli.set(action.key, { action, n: 1 });
    }
  }
  if (counts.thinking === items.length) {
    const ms = items.map(thoughtMs);
    const total = ms.every((m) => m != null) ? ms.reduce<number>((a, m) => a + (m ?? 0), 0) : null;
    return { label: thoughtLabel(total), icon: null };
  }
  const parts: string[] = [];
  const reads = counts.read ?? 0;
  if (reads) parts.push(`explored ${plural(reads, "file")}`);
  if (counts.search) parts.push(plural(counts.search, "search", "searches"));
  // A tally merges clauses that read the same ("listed memories" twice).
  for (const clause of new Set([...cli.values()].map(({ action, n }) => action.tally(n)))) parts.push(clause);
  if (counts.bash) parts.push(`ran ${plural(counts.bash, "command")}`);
  const edits = (counts.edit ?? 0) + (counts.write ?? 0);
  if (edits) parts.push(`edited ${plural(edits, "file")}`);
  if (counts.web) parts.push(plural(counts.web, "web lookup"));
  if (counts.task) parts.push(`ran ${plural(counts.task, "subagent")}`);
  if (counts.plan) parts.push("updated the plan");
  if (counts.other) parts.push(plural(counts.other, "tool call"));
  const label = parts.join(", ");
  const dominant = (Object.entries(counts) as [ToolKind | "thinking", number][])
    .filter(([k]) => k !== "thinking")
    .sort((a, b) => b[1] - a[1])[0]?.[0] as ToolKind | undefined;
  const topCli = [...cli.values()].sort((a, b) => b.n - a.n)[0]?.action.icon;
  const icon: BundleIcon = dominant === "oculus_cli" && topCli ? { cli: topCli } : { tool: dominant ?? "other" };
  return { label: label.charAt(0).toUpperCase() + label.slice(1), icon };
}
