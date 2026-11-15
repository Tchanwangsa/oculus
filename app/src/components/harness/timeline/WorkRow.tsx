import { memo, useMemo, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  CaretDown,
  BookOpen,
  BookmarkSimple,
  Brain,
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
  reviewView,
  summariseOculusCli,
  thoughtLabel,
  toolVerb,
  type CliIcon,
  type HarnessItem,
  type Provider,
  type ToolKind,
} from "@/lib/harness";
import { libraryPath, openLibraryPath } from "@/lib/files/openFile";
import { useVisibleAnimation } from "@/hooks/ui/useVisibleAnimation";
import { cn } from "@/lib/utils";
import { useOverflows } from "./useOverflows";

/** Timeline work rows (tool call, reasoning, error): `[icon] [title] [status]`,
 *  expanding to detail on click. Rows stack flat wherever they sit — a run's
 *  fold, a bundle, a bare step — with no guide line or extra indent; detail
 *  opens beneath at the row's own edge. */

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
  code,
  em,
  note,
  live,
  trailing,
  expandable,
  dim,
  tone = "default",
  children,
  defaultOpen = false,
  flush,
}: {
  icon?: Icon;
  title: string;
  /** A command that is the whole label, drawn as code in place of `title`;
   *  it wraps in full while the row is open. */
  code?: string;
  /** Emphasised part after the title (the command, the path). */
  em?: string;
  /** Muted text after the title, truncating (a live thought's latest line). */
  note?: string;
  /** Still at work: the title shimmers while it can be seen. */
  live?: boolean;
  trailing?: React.ReactNode;
  expandable: boolean;
  dim?: boolean;
  tone?: "default" | "error";
  children?: React.ReactNode;
  defaultOpen?: boolean;
  /** Children are rows themselves (a bundle's steps): no inset, so they line
   *  up with the header. */
  flush?: boolean;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const shimmer = useVisibleAnimation<HTMLSpanElement>(!!live);
  const canOpen = expandable && (!!children || !!code);
  const wrap = !!code && open;
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
          "group/row flex w-full min-w-0 gap-1.5 rounded-md px-2 py-0.5 text-left text-xs leading-5 transition-colors",
          wrap ? "items-start" : "items-center",
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
            "flex min-w-0 shrink gap-1.5 text-left",
            wrap ? "items-start" : "items-center",
            canOpen ? "cursor-pointer" : "cursor-default",
          )}
        >
          {IconC && <IconC size={14} className={cn("shrink-0", wrap && "mt-[3px]")} />}
          {/* With an `em` or a `note`, the verb holds and that truncates;
              without one the title is the long part and must truncate, or a
              narrow dock's scroller gains a horizontal axis. */}
          {code ? (
            <span
              ref={shimmer}
              className={cn(
                "min-w-0 font-mono text-[11.5px] text-foreground/80",
                wrap ? "whitespace-pre-wrap break-all" : "truncate",
                live && "text-shimmer",
              )}
              title={wrap ? undefined : code}
            >
              {code}
            </span>
          ) : (
            <span
              ref={shimmer}
              className={cn("min-w-0", em || note ? "shrink-0" : "truncate", live && "text-shimmer")}
              title={em || note ? undefined : title}
            >
              {title}
            </span>
          )}
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
            className={cn("ml-auto shrink-0 cursor-pointer", wrap && "mt-[5px]")}
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
      {open && children && <div className={flush ? undefined : "px-2 pb-1 pt-0.5"}>{children}</div>}
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

/** Taller detail than this folds behind Show more: the timeline is the only
 *  scroller on the page. */
const DETAIL_MAX_H = 288;

/** A tool's detail on a quiet surface: an optional muted lead (the call's
 *  command or arguments) and its output, sized to content up to
 *  `DETAIL_MAX_H`. */
function ToolDetail({ lead, output }: { lead?: string; output: string }) {
  const [open, setOpen] = useState(false);
  const [body, long] = useOverflows(`${lead ?? ""}\n${output}`, true, DETAIL_MAX_H);
  const clipped = long && !open;
  return (
    <div className="rounded-md bg-surface">
      <div
        ref={body}
        className={cn(
          "px-3 py-2",
          clipped && "overflow-hidden [mask-image:linear-gradient(to_bottom,#000_calc(100%-2.25rem),transparent)]",
        )}
        style={clipped ? { maxHeight: DETAIL_MAX_H } : undefined}
      >
        {lead && <CodeText className="text-muted-foreground">{lead}</CodeText>}
        {output && <CodeText className={cn("text-foreground/85", lead && "mt-1.5")}>{output}</CodeText>}
      </div>
      {long && (
        <button
          type="button"
          data-copy-skip
          aria-expanded={open}
          onClick={() => setOpen((o) => !o)}
          className="flex cursor-pointer items-center gap-1 px-3 pb-1.5 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
        >
          {open ? "Show less" : "Show more"}
          <CaretDown size={11} className={cn("transition-transform will-change-transform", open && "rotate-180")} />
        </button>
      )}
    </div>
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
  // A plain command is its own label; the detail is then its output alone.
  const label = !cli && command ? command : undefined;
  const lead = label ? undefined : command ?? (entries.length > 0 ? entries.map(([k, v]) => `${k}: ${v}`).join("\n") : undefined);
  // In Auto the CLI's reviewer decided this call; said quietly, never asked.
  const review = reviewView(meta.review);
  return (
    <RowShell
      icon={cli ? CLI_ICON[cli.icon] : TOOL_ICON[kind]}
      title={cli ? cli.title : toolVerb(kind, done, meta.name)}
      code={label}
      em={label ? undefined : cli ? cli.em : (item.content ?? undefined)}
      live={!!cli && !done}
      dim={dim && done}
      expandable={entries.length > 0 || output.length > 0 || !!review}
      trailing={
        <>
          {review && (
            <span className={cn("shrink-0 text-[11px]", review.tone === "danger" ? "text-destructive" : "text-muted-foreground")}>
              {review.label}
            </span>
          )}
          {cli && !done ? null : !done ? (
            <CircleNotch size={12} className="shrink-0 animate-spin" />
          ) : failed && !review ? (
            <span className="shrink-0 text-[11px] text-destructive">failed</span>
          ) : null}
        </>
      }
    >
      {review && <ReviewNote review={review} />}
      {picture ? (
        <ReadPicture path={picture} onFail={() => setPictureFailed(true)} />
      ) : lead || output ? (
        <ToolDetail lead={lead} output={output} />
      ) : null}
    </RowShell>
  );
});

/** The reviewer's decision, opened: who decided, the risk it saw, why. */
function ReviewNote({ review }: { review: NonNullable<ReturnType<typeof reviewView>> }) {
  return (
    <div className="mb-1.5 text-[11px] leading-relaxed text-muted-foreground" data-selectable>
      <p className={cn(review.tone === "danger" && "text-destructive")}>
        {[review.label, review.risk].filter(Boolean).join(" · ")}
        {" — you were not asked (Auto)"}
      </p>
      {review.rationale && <p className="break-words">{review.rationale}</p>}
    </div>
  );
}

/** The thought itself, opened: rendered markdown under the row's label
 *  (past the 14px icon and its gap), with no rule beside it. */
function Thought({ text }: { text: string }) {
  return (
    <div className="pl-5">
      <CompactMd text={text} className="md-thought leading-relaxed" />
    </div>
  );
}

/** A finished reasoning block: "Thought for 12s". */
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
    <RowShell icon={Brain} title={thoughtLabel(ms)} dim={dim} expandable={text.trim().length > 0}>
      <Thought text={text} />
    </RowShell>
  );
});

/** The block still streaming: a shimmering "Thinking" and its latest sentence.
 *  Only `LiveTail` draws it, so it re-renders per fold, never per token. */
export function LiveThinkingRow({ text }: { text: string }) {
  return (
    <RowShell icon={Brain} title="Thinking" live note={latestThought(text)} expandable={text.trim().length > 0}>
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
