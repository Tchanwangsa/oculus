import { useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import { Brain, CircleNotch } from "@phosphor-icons/react";
import { MD_COMPONENTS } from "@/components/markdown/MdComponents";
import { parseToolMeta, type HarnessItem, type ToolKind } from "@/lib/harness";
import type { LiveTurn } from "@/stores/harnessStore";
import { ErrorRow, RowShell, ThinkingRow, ToolRow, TOOL_ICON } from "./WorkRow";

/**
 * The thread as a list. Messages are the spine; the work between them —
 * tool calls, reasoning — is a *step*, and a finished step with more than
 * one row folds into a single summary row ("Explored 3 files, ran 2
 * commands") that opens to the rows. The step still in progress stays
 * unfolded, with its rows at full strength; finished rows are dimmed. This is
 * bb's `buildTimelineViewRows`, with one level of grouping instead of two.
 */

type ViewRow =
  | { kind: "item"; item: HarnessItem; dim: boolean }
  | { kind: "bundle"; id: string; items: HarnessItem[] };

const isWork = (i: HarnessItem) => i.kind === "tool" || i.kind === "thinking";

function buildRows(items: HarnessItem[], running: boolean): ViewRow[] {
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
  // The trailing step is live while the turn runs; otherwise it is as
  // finished as the rest.
  close(!running);
  return out;
}

function plural(n: number, one: string, many = `${one}s`) {
  return `${n} ${n === 1 ? one : many}`;
}

/** "Explored 3 files, 2 searches, ran 1 command" */
function bundleLabel(items: HarnessItem[]): { label: string; icon: ToolKind } {
  const counts: Partial<Record<ToolKind | "thinking", number>> = {};
  for (const i of items) {
    const k = i.kind === "thinking" ? "thinking" : (parseToolMeta(i).kind ?? "other");
    counts[k] = (counts[k] ?? 0) + 1;
  }
  const parts: string[] = [];
  const reads = counts.read ?? 0;
  if (reads) parts.push(`explored ${plural(reads, "file")}`);
  if (counts.search) parts.push(plural(counts.search, "search", "searches"));
  if (counts.oculus_cli) parts.push(`looked up the library ${counts.oculus_cli === 1 ? "once" : `${counts.oculus_cli} times`}`);
  if (counts.bash) parts.push(`ran ${plural(counts.bash, "command")}`);
  const edits = (counts.edit ?? 0) + (counts.write ?? 0);
  if (edits) parts.push(`edited ${plural(edits, "file")}`);
  if (counts.web) parts.push(plural(counts.web, "web lookup"));
  if (counts.task) parts.push(`ran ${plural(counts.task, "subagent")}`);
  if (counts.plan) parts.push("updated the plan");
  if (counts.other) parts.push(plural(counts.other, "tool call"));
  if (counts.thinking && parts.length === 0) parts.push("thought");
  const label = parts.join(", ");
  const dominant = (Object.entries(counts) as [ToolKind | "thinking", number][])
    .filter(([k]) => k !== "thinking")
    .sort((a, b) => b[1] - a[1])[0]?.[0] as ToolKind | undefined;
  return { label: label.charAt(0).toUpperCase() + label.slice(1), icon: dominant ?? "other" };
}

function Assistant({ text }: { text: string }) {
  return (
    <div className="min-w-0 px-2 text-sm leading-relaxed">
      <ReactMarkdown remarkPlugins={[remarkGfm, remarkMath]} rehypePlugins={[rehypeKatex]} components={MD_COMPONENTS}>
        {text}
      </ReactMarkdown>
    </div>
  );
}

function User({ text }: { text: string }) {
  return (
    <div className="flex w-full justify-end">
      <div className="max-w-[70%] whitespace-pre-wrap break-words rounded-xl border border-border bg-surface px-4 py-2.5 text-sm leading-relaxed text-foreground">
        {text}
      </div>
    </div>
  );
}

function Item({ item, dim, live }: { item: HarnessItem; dim: boolean; live: LiveTurn }) {
  switch (item.kind) {
    case "user":
      return <User text={item.content ?? ""} />;
    case "assistant":
      return <Assistant text={item.content ?? ""} />;
    case "thinking":
      return <ThinkingRow text={item.content ?? ""} dim={dim} />;
    case "tool":
      return <ToolRow item={item} dim={dim} liveOutput={item.ref_id ? live.toolOutput[item.ref_id] : undefined} />;
    case "error":
      return <ErrorRow text={item.content ?? ""} />;
  }
}

function Bundle({ items, live }: { items: HarnessItem[]; live: LiveTurn }) {
  const { label, icon } = bundleLabel(items);
  return (
    <RowShell icon={TOOL_ICON[icon]} title={label} expandable dim>
      {/* The group line: rows hang off a hairline, the way nested work does in bb. */}
      <div className="relative my-0.5 pl-3 before:absolute before:bottom-1 before:left-1.5 before:top-0 before:w-px before:bg-border before:content-['']">
        <div className="flex flex-col">
          {items.map((i) => (
            <Item key={i.id} item={i} dim={false} live={live} />
          ))}
        </div>
      </div>
    </RowShell>
  );
}

export function Timeline({ items, live }: { items: HarnessItem[]; live: LiveTurn }) {
  const rows = useMemo(() => buildRows(items, live.running), [items, live.running]);
  const quiet = live.running && !live.streaming && !live.thinking;
  return (
    <div className="flex min-w-0 flex-col gap-2">
      {rows.map((r) =>
        r.kind === "bundle" ? (
          <Bundle key={r.id} items={r.items} live={live} />
        ) : (
          <Item key={r.item.id} item={r.item} dim={r.dim} live={live} />
        ),
      )}

      {live.thinking && <ThinkingRow text={live.thinking} live />}
      {live.streaming && <Assistant text={live.streaming} />}

      {quiet && (
        <div className="mt-1 flex items-center gap-2 px-2 text-[13px] text-muted-foreground">
          {live.thinking ? <Brain size={14} /> : <CircleNotch size={14} className="animate-spin" />}
          <span className="animate-pulse">Working…</span>
        </div>
      )}
    </div>
  );
}
