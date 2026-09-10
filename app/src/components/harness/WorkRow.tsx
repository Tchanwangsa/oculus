import { useState } from "react";
import {
  Brain,
  BookOpen,
  CaretRight,
  CircleNotch,
  FileText,
  Globe,
  ListChecks,
  MagnifyingGlass,
  PencilSimpleLine,
  Terminal,
  UsersThree,
  Warning,
  Wrench,
} from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import { CodeText } from "@/components/markdown/MdComponents";
import { parseToolMeta, type HarnessItem, type ToolKind } from "@/lib/harness";
import { cn } from "@/lib/utils";

/**
 * One line of work in the timeline — a tool call, a block of reasoning, an
 * error — collapsed to `[icon] [title] [status]` with a chevron that shows on
 * hover, expanding to its detail. bb's row shape: the timeline stays a
 * readable list of what happened, and the transcript is behind a click.
 */

export const TOOL_ICON: Record<ToolKind, Icon> = {
  read: FileText,
  edit: PencilSimpleLine,
  write: PencilSimpleLine,
  bash: Terminal,
  search: MagnifyingGlass,
  oculus_cli: BookOpen,
  task: UsersThree,
  web: Globe,
  plan: ListChecks,
  other: Wrench,
};

/** "Ran", "Read", "Edited" — past tense once done, present while running. */
function verb(kind: ToolKind, done: boolean): string {
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

export function RowShell({
  icon: IconC,
  title,
  em,
  trailing,
  expandable,
  dim,
  tone = "default",
  children,
  defaultOpen = false,
}: {
  icon: Icon;
  title: string;
  /** Emphasised part after the title (the command, the path). */
  em?: string;
  trailing?: React.ReactNode;
  expandable: boolean;
  dim?: boolean;
  tone?: "default" | "error";
  children?: React.ReactNode;
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const canOpen = expandable && !!children;
  return (
    <div className={cn("min-w-0 transition-opacity", dim && !open && "opacity-40 hover:opacity-100")}>
      <button
        type="button"
        disabled={!canOpen}
        aria-expanded={open}
        onClick={() => canOpen && setOpen((o) => !o)}
        className={cn(
          "group/row flex w-full min-w-0 items-center gap-1.5 rounded-md px-2 py-0.5 text-left text-[13px] leading-5 transition-colors",
          tone === "error"
            ? "text-destructive"
            : open
              ? "text-foreground"
              : "text-muted-foreground hover:text-foreground",
          canOpen ? "cursor-pointer" : "cursor-default",
        )}
      >
        <IconC size={14} className="shrink-0" />
        <span className="shrink-0">{title}</span>
        {em && (
          <span className="min-w-0 truncate font-medium text-foreground/80" title={em}>
            {em}
          </span>
        )}
        {trailing}
        {canOpen && (
          <CaretRight
            size={11}
            className={cn(
              "ml-auto shrink-0 transition-[opacity,transform] duration-150",
              open ? "rotate-90 opacity-60" : "opacity-0 group-hover/row:opacity-60",
            )}
          />
        )}
      </button>
      {open && children && <div className="px-2 pb-1 pt-0.5">{children}</div>}
    </div>
  );
}

/** Tool args as `key: value` lines, values clipped — the arguments are the
 *  header of the detail card, not its content. */
function args(input: unknown): [string, string][] {
  if (!input || typeof input !== "object") return [];
  return Object.entries(input as Record<string, unknown>)
    .filter(([, v]) => v !== null && v !== undefined && v !== "")
    .map(([k, v]) => {
      const s = typeof v === "string" ? v : JSON.stringify(v);
      return [k, s.length > 400 ? `${s.slice(0, 400)}…` : s];
    });
}

export function ToolRow({
  item,
  liveOutput,
  dim,
}: {
  item: HarnessItem;
  /** Output still streaming, before the row's `meta.output` is written. */
  liveOutput?: string;
  dim?: boolean;
}) {
  const meta = parseToolMeta(item);
  const kind: ToolKind = meta.kind ?? "other";
  const done = meta.ok != null;
  const failed = meta.ok === false;
  const output = meta.output ?? liveOutput ?? "";
  const entries = args(meta.input);
  const isCommand = kind === "bash" || kind === "oculus_cli";
  const command = isCommand
    ? entries.find(([k]) => k === "command")?.[1]
    : undefined;
  return (
    <RowShell
      icon={TOOL_ICON[kind]}
      title={verb(kind, done)}
      em={item.content ?? undefined}
      dim={dim && done}
      expandable={entries.length > 0 || output.length > 0}
      trailing={
        !done ? (
          <CircleNotch size={12} className="shrink-0 animate-spin" />
        ) : failed ? (
          <span className="shrink-0 text-[11px] text-destructive">failed</span>
        ) : null
      }
    >
      <div className="overflow-hidden rounded-lg border border-border bg-card">
        <div className="max-h-72 overflow-auto px-3 py-2">
          {command ? (
            <CodeText className="text-muted-foreground">$ {command}</CodeText>
          ) : (
            entries.length > 0 && (
              <CodeText className="text-muted-foreground">
                {entries.map(([k, v]) => `${k}: ${v}`).join("\n")}
              </CodeText>
            )
          )}
          {output && (
            <CodeText className={cn((command || entries.length) && "mt-2 border-t border-border pt-2")}>
              {output}
            </CodeText>
          )}
        </div>
      </div>
    </RowShell>
  );
}

export function ThinkingRow({ text, live, dim }: { text: string; live?: boolean; dim?: boolean }) {
  return (
    <RowShell
      icon={Brain}
      title={live ? "Thinking…" : "Thought"}
      dim={dim}
      expandable={text.trim().length > 0}
      trailing={live ? <CircleNotch size={12} className="shrink-0 animate-spin" /> : null}
    >
      <div className="max-h-80 overflow-auto whitespace-pre-wrap break-words border-l border-border pl-3 text-[13px] leading-relaxed text-muted-foreground">
        {text}
      </div>
    </RowShell>
  );
}

export function ErrorRow({ text }: { text: string }) {
  const [first, ...rest] = text.split("\n");
  return (
    <RowShell icon={Warning} title={first} tone="error" expandable={rest.length > 0} defaultOpen={false}>
      <CodeText className="rounded-lg border border-destructive/30 bg-card px-3 py-2 text-destructive">
        {rest.join("\n")}
      </CodeText>
    </RowShell>
  );
}
