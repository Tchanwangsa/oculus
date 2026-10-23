import { memo, useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  BookOpen,
  BookmarkSimple,
  CalendarBlank,
  CaretRight,
  CheckSquare,
  CircleNotch,
  FileText,
  FilmSlate,
  FolderSimple,
  Globe,
  Kanban,
  ListChecks,
  MagnifyingGlass,
  PencilSimpleLine,
  SignIn,
  Tag,
  Terminal,
  UsersThree,
  VideoCamera,
  Warning,
  Wrench,
} from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { CodeText, CompactMd } from "@/components/markdown/MdComponents";
import { ImageLightbox } from "@/components/ui/lightbox/Lightbox";
import { isImagePath } from "@/lib/harness/attachments";
import {
  describeOculusCli,
  latestThought,
  parseToolMeta,
  providerLabel,
  summariseOculusCli,
  thoughtLabel,
  toolVerb,
  type CliIcon,
  type HarnessItem,
  type Provider,
  type ToolKind,
} from "@/lib/harness";
import { libraryPath, openLibraryPath } from "@/lib/files/openFile";
import { cn } from "@/lib/utils";

/** Timeline work rows (tool call, reasoning, error): `[icon] [title] [status]`,
 *  expanding to detail on click. */

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

export const CLI_ICON: Record<CliIcon, Icon> = {
  memory: BookmarkSimple,
  search: MagnifyingGlass,
  file: FileText,
  folder: FolderSimple,
  calendar: CalendarBlank,
  task: CheckSquare,
  project: Kanban,
  tag: Tag,
  lecture: VideoCamera,
  studio: FilmSlate,
  library: BookOpen,
};

export function RowShell({
  icon: IconC,
  title,
  em,
  note,
  live,
  trailing,
  expandable,
  dim,
  tone = "default",
  children,
  defaultOpen = false,
}: {
  /** None for a reasoning row, which is words only. */
  icon?: Icon;
  title: string;
  /** Emphasised part after the title (the command, the path). */
  em?: string;
  /** Muted text after the title, truncating (a live thought's latest line). */
  note?: string;
  /** Still at work: the title shimmers. */
  live?: boolean;
  trailing?: React.ReactNode;
  expandable: boolean;
  dim?: boolean;
  tone?: "default" | "error";
  children?: React.ReactNode;
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const canOpen = expandable && !!children;
  // A library path in `em` becomes a link that opens the file.
  const path = libraryPath(em);
  const tint =
    tone === "error"
      ? "text-destructive"
      : open
        ? "text-foreground"
        : "text-muted-foreground";
  return (
    <div className={cn("min-w-0 transition-opacity will-change-[opacity]", dim && !open && "opacity-40 hover:opacity-100")}>
      {/* Sibling controls, not nested buttons: the path link and the disclosure. */}
      <div
        className={cn(
          "group/row flex w-full min-w-0 items-center gap-1.5 rounded-md px-2 py-0.5 text-left text-xs leading-5 transition-colors",
          tint,
          // `hover:`, not `group-hover/row:` — this *is* the group.
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
          {IconC && <IconC size={14} className="shrink-0" />}
          {/* With an `em` or a `note`, the verb holds and that truncates;
              without one the title is the long part and must truncate, or a
              narrow dock's scroller gains a horizontal axis. */}
          <span
            className={cn("min-w-0", em || note ? "shrink-0" : "truncate", live && "text-shimmer")}
            title={em || note ? undefined : title}
          >
            {title}
          </span>
          {note && <span className="min-w-0 truncate">{note}</span>}
          {em && !path && (
            <span className="min-w-0 truncate font-medium text-foreground/80" title={em}>
              {em}
            </span>
          )}
        </button>
        {em && path && (
          <button
            type="button"
            onClick={(e) => openLibraryPath(path, e.metaKey || e.ctrlKey)}
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
                "transition-[opacity,transform] will-change-[opacity,transform] duration-150",
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

/** Tool args as `key: value` lines, values clipped. */
function args(input: unknown): [string, string][] {
  if (!input || typeof input !== "object") return [];
  return Object.entries(input as Record<string, unknown>)
    .filter(([, v]) => v !== null && v !== undefined && v !== "")
    .map(([k, v]) => {
      const s = typeof v === "string" ? v : JSON.stringify(v);
      return [k, s.length > 400 ? `${s.slice(0, 400)}…` : s];
    });
}

/** A Read of a picture shows the picture. Only paths under the asset scope
 *  (`tauri.conf.json`) load; anything else fails and falls back to the args. */
function ReadPicture({ path, onFail }: { path: string; onFail: () => void }) {
  const [open, setOpen] = useState(false);
  const src = convertFileSrc(path);
  return (
    <>
      <ImageLightbox src={src} alt={path} open={open} onOpenChange={setOpen} />
      <button
        type="button"
        aria-label="Open the picture"
        onClick={() => setOpen(true)}
        className="block max-w-full cursor-zoom-in overflow-hidden rounded-lg border border-border bg-card transition-colors hover:border-ring"
      >
        <img
          src={src}
          alt={path}
          onError={onFail}
          className="block max-h-72 w-auto max-w-full"
        />
      </button>
    </>
  );
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
  const [pictureFailed, setPictureFailed] = useState(false);
  const readPath =
    kind === "read" ? entries.find(([k]) => k === "file_path" || k === "path")?.[1] : undefined;
  const picture = readPath && !pictureFailed && isImagePath(readPath) ? readPath : null;
  // An `oculus …` call says what it did ("Saved a memory"); its command
  // stays in the detail below.
  const cliActions = useMemo(
    () => (kind === "oculus_cli" ? describeOculusCli(item.content ?? "") : []),
    [kind, item.content],
  );
  const cli = summariseOculusCli(cliActions, done);
  return (
    <RowShell
      icon={cli ? CLI_ICON[cli.icon] : TOOL_ICON[kind]}
      title={cli ? cli.title : toolVerb(kind, done, meta.name)}
      em={cli ? cli.em : (item.content ?? undefined)}
      live={!!cli && !done}
      dim={dim && done}
      expandable={entries.length > 0 || output.length > 0}
      trailing={
        cli && !done ? null : !done ? (
          <CircleNotch size={12} className="shrink-0 animate-spin" />
        ) : failed ? (
          <span className="shrink-0 text-[11px] text-destructive">failed</span>
        ) : null
      }
    >
      {picture ? (
        <ReadPicture path={picture} onFail={() => setPictureFailed(true)} />
      ) : (
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
      )}
    </RowShell>
  );
});

/** The thought itself, opened: rendered markdown beside a hairline. */
function Thought({ text }: { text: string }) {
  return (
    <div className="ml-1.5 border-l border-border pl-3">
      <CompactMd text={text} className="md-thought leading-relaxed" />
    </div>
  );
}

/** A finished reasoning block: "Thought for 12s", words only. */
export const ThinkingRow = memo(function ThinkingRow({
  text,
  ms,
  dim,
}: {
  text: string;
  ms: number | null;
  dim?: boolean;
}) {
  return (
    <RowShell title={thoughtLabel(ms)} dim={dim} expandable={text.trim().length > 0}>
      <Thought text={text} />
    </RowShell>
  );
});

/** The block still streaming: a shimmering "Thinking" and its latest sentence.
 *  Only `LiveTail` draws it, so it re-renders per fold, never per token. */
export function LiveThinkingRow({ text }: { text: string }) {
  return (
    <RowShell title="Thinking" live note={latestThought(text)} expandable={text.trim().length > 0}>
      <Thought text={text} />
    </RowShell>
  );
}

/** A turn that failed for lack of credentials: a fixable state, so `brand` and
 *  a Sign in button rather than a destructive error row. */
const SignedOutCard = memo(function SignedOutCard({
  provider,
  message,
  onSignIn,
}: {
  provider: Provider;
  message: string;
  onSignIn?: (provider: Provider) => void;
}) {
  return (
    <div className="flex items-start gap-2.5 rounded-xl border border-border bg-card px-3 py-2.5">
      <SignIn size={14} className="mt-0.5 shrink-0 text-brand" />
      <div className="min-w-0 flex-1">
        <div className="text-xs text-foreground">{providerLabel(provider)} is signed out</div>
        <p className="mt-0.5 break-words text-[11.5px] leading-relaxed text-muted-foreground">
          {message}
        </p>
      </div>
      {onSignIn && (
        <Button size="xs" className="shrink-0" onClick={() => onSignIn(provider)}>
          Sign in
        </Button>
      )}
    </div>
  );
});

export const ErrorRow = memo(function ErrorRow({
  text,
  auth,
  onSignIn,
}: {
  text: string;
  /** The provider reporting missing credentials (`parseErrorMeta`, or the event's `auth`). */
  auth?: Provider;
  onSignIn?: (provider: Provider) => void;
}) {
  const [first, ...rest] = text.split("\n");
  if (auth) return <SignedOutCard provider={auth} message={text} onSignIn={onSignIn} />;
  return (
    <RowShell icon={Warning} title={first} tone="error" expandable={rest.length > 0} defaultOpen={false}>
      <CodeText className="rounded-lg border border-destructive/30 bg-card px-3 py-2 text-destructive">
        {rest.join("\n")}
      </CodeText>
    </RowShell>
  );
});
