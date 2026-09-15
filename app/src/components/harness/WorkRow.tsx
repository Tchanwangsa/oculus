import { memo, useState } from "react";
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
import { parseToolMeta, toolVerb, type HarnessItem, type ToolKind } from "@/lib/harness";
import { libraryPath, openLibraryPath } from "@/lib/openFile";
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
  // A path the agent touched is a file the reader may want to see, so it opens
  // in the side panel instead of being text to retype into the ⌘K palette.
  const path = libraryPath(em);
  const tint =
    tone === "error"
      ? "text-destructive"
      : open
        ? "text-foreground"
        : "text-muted-foreground";
  return (
    <div className={cn("min-w-0 transition-opacity", dim && !open && "opacity-40 hover:opacity-100")}>
      {/* The row is a div holding two controls rather than one button wrapping
          another: a path is a link and the rest of the row is a disclosure, and
          nesting those would be both invalid and unreachable from a keyboard.
          The hover tint they share is the group's, so it still reads as one
          row. */}
      <div
        className={cn(
          "group/row flex w-full min-w-0 items-center gap-1.5 rounded-md px-2 py-0.5 text-left text-xs leading-5 transition-colors",
          tint,
          // `hover:`, not `group-hover/row:` — this *is* the group, and a
          // group-hover utility only ever matches its descendants.
          tone !== "error" && "hover:text-foreground",
        )}
      >
        <button
          type="button"
          disabled={!canOpen}
          aria-expanded={open}
          onClick={() => canOpen && setOpen((o) => !o)}
          className={cn(
            "flex min-w-0 shrink items-center gap-1.5 text-left",
            canOpen ? "cursor-pointer" : "cursor-default",
          )}
        >
          <IconC size={14} className="shrink-0" />
          <span className="shrink-0">{title}</span>
          {em && !path && (
            <span className="min-w-0 truncate font-medium text-foreground/80" title={em}>
              {em}
            </span>
          )}
        </button>
        {em && path && (
          <button
            type="button"
            onClick={() => openLibraryPath(path)}
            title={em}
            className="min-w-0 truncate rounded font-medium text-foreground/80 underline decoration-transparent underline-offset-2 transition-colors hover:text-brand hover:decoration-brand"
          >
            {em}
          </button>
        )}
        {trailing}
        {canOpen && (
          <button
            type="button"
            aria-hidden
            tabIndex={-1}
            onClick={() => setOpen((o) => !o)}
            className="ml-auto shrink-0 cursor-pointer"
          >
            <CaretRight
              size={11}
              className={cn(
                "transition-[opacity,transform] duration-150",
                open ? "rotate-90 opacity-60" : "opacity-0 group-hover/row:opacity-60",
              )}
            />
          </button>
        )}
      </div>
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

export const ToolRow = memo(function ToolRow({
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
      title={toolVerb(kind, done)}
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
});

export const ThinkingRow = memo(function ThinkingRow({
  text,
  live,
  dim,
}: {
  text: string;
  live?: boolean;
  dim?: boolean;
}) {
  return (
    <RowShell
      icon={Brain}
      title={live ? "Thinking…" : "Thought"}
      dim={dim}
      expandable={text.trim().length > 0}
      trailing={live ? <CircleNotch size={12} className="shrink-0 animate-spin" /> : null}
    >
      <div className="max-h-80 overflow-auto whitespace-pre-wrap break-words border-l border-border pl-3 text-[11.5px] leading-relaxed text-muted-foreground">
        {text}
      </div>
    </RowShell>
  );
});

export const ErrorRow = memo(function ErrorRow({ text }: { text: string }) {
  const [first, ...rest] = text.split("\n");
  return (
    <RowShell icon={Warning} title={first} tone="error" expandable={rest.length > 0} defaultOpen={false}>
      <CodeText className="rounded-lg border border-destructive/30 bg-card px-3 py-2 text-destructive">
        {rest.join("\n")}
      </CodeText>
    </RowShell>
  );
});
