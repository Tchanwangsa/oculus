import { memo, useEffect, useMemo, useRef, useState } from "react";
import { CaretRight, CircleNotch, Plus, SidebarSimple, Trash, VideoCamera, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { ProviderMark } from "@/components/harness/ProviderMark";
import type { HarnessThread } from "@/lib/harness";
import type { Subject } from "@/lib/db";
import { displayCode, displayName } from "@/lib/format";
import { cn } from "@/lib/utils";

const COLLAPSED_KEY = "oculus-chat-groups-collapsed";

/** Which groups are folded away, by group key. What is stored is the collapsed
 *  ones rather than the open ones, so a subject scoped for the first time
 *  arrives expanded without having to be listed anywhere first. */
function loadCollapsed(): Set<string> {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "[]");
    return new Set(Array.isArray(raw) ? raw.filter((k): k is string => typeof k === "string") : []);
  } catch {
    return new Set();
  }
}

/** Threads under the scope they were opened with, most recent group first.
 *  Threads arrive ordered by `updated_at`, so first appearance is recency for
 *  the groups too, and a subject that has since been deleted falls back into
 *  General rather than disappearing with its heading. */
function group(
  threads: HarnessThread[],
  subjects: Subject[],
): {
  key: string;
  label: string;
  title: string;
  subjectId: number | null;
  threads: HarnessThread[];
}[] {
  const out: ReturnType<typeof group> = [];
  const byKey = new Map<string, (typeof out)[number]>();
  for (const t of threads) {
    const subject = t.subject_id == null ? null : subjects.find((s) => s.id === t.subject_id) ?? null;
    const key = subject ? String(subject.id) : "general";
    let g = byKey.get(key);
    if (!g) {
      g = {
        key,
        label: subject ? displayCode(subject.code) : "General",
        title: subject ? displayName(subject.name, subject.code) : "Not scoped to a subject",
        subjectId: subject?.id ?? null,
        threads: [],
      };
      byKey.set(key, g);
      out.push(g);
    }
    g.threads.push(t);
  }
  return out;
}

/**
 * The conversations column: threads grouped by the subject they are scoped
 * to, each row its agent's mark and the thread's name.
 *
 * Its width is the page's (`useResizablePanel` in `ChatPage`); this only draws
 * it. Folded means gone rather than narrowed — the app's own sidebar pattern —
 * so the way back in is the button in the chat header, not a rail left behind
 * here. The inner box keeps the unfolded width throughout, so the fold clips
 * the list instead of reflowing every row on its way out.
 *
 * A group header doubles as a way in: its `+` opens a new thread already
 * scoped to that subject, which is the whole reason to pick a scope and the
 * one place the choice is already made.
 *
 * The name is the model's own — asked for once the first exchange is done
 * (`Harness::name_thread`) and, until it lands, the first line of the first
 * message. The model the thread runs on is not shown here: it can change per
 * send, it is already in the composer, and this list answers "which
 * conversation", not "on what".
 */
export const ThreadList = memo(function ThreadList({
  threads,
  subjects,
  activeId,
  runningIds,
  width,
  restWidth,
  collapsed,
  animate,
  onToggle,
  onOpen,
  onNew,
  onDelete,
}: {
  threads: HarnessThread[];
  subjects: Subject[];
  activeId: number | null;
  /** Drawn width: 0 while folded. */
  width: number;
  /** The width it unfolds back to — what the contents are laid out at. */
  restWidth: number;
  collapsed: boolean;
  /** Off mid-drag: a width that eased toward every mouse position lagged the
   *  handle by a frame and felt like dragging elastic. */
  animate: boolean;
  onToggle: () => void;
  /** Which threads have a turn in flight. A set of ids rather than the
   *  store's live map: that map changes with every streamed token, and this
   *  column only ever asks it one yes/no question per row. */
  runningIds: Set<number>;
  onOpen: (id: number) => void;
  /** No argument is the composer's current scope; a subject id (or null for
   *  General) starts the new thread in that group instead. */
  onNew: (subjectId?: number | null) => void;
  onDelete: (id: number) => void;
}) {
  const [confirming, setConfirming] = useState<number | null>(null);
  // Which *groups* are folded — not to be confused with the panel's own
  // `collapsed` prop above.
  const [folded, setFolded] = useState<Set<string>>(loadCollapsed);

  useEffect(() => {
    localStorage.setItem(COLLAPSED_KEY, JSON.stringify([...folded]));
  }, [folded]);

  const groups = useMemo(() => group(threads, subjects), [threads, subjects]);

  const setGroupOpen = (key: string, open: boolean) =>
    setFolded((prev) => {
      if (open === !prev.has(key)) return prev;
      const next = new Set(prev);
      if (open) next.delete(key);
      else next.add(key);
      return next;
    });

  return (
    <aside
      /* width/min/max move together so flexbox cannot clamp the box to its
         min-content size mid-fold. */
      style={{ width, minWidth: width, maxWidth: width }}
      className={cn(
        "flex shrink-0 grow-0 flex-col overflow-hidden",
        // The divider is the panel's right edge, so it goes when the panel
        // does — a 1px rule left standing on a zero-width box reads as a
        // second border beside the card's own.
        !collapsed && "border-r border-border-subtle",
        animate && "transition-[width,min-width,max-width] duration-200 ease-out",
      )}
    >
      <div className="flex h-full flex-col" style={{ width: restWidth, minWidth: restWidth }}>
        <div className="flex items-center gap-1 p-2">
          <Button variant="ghost" size="xs" className="min-w-0 flex-1 justify-start" onClick={() => onNew()}>
            <Plus size={13} /> New thread
          </Button>
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="Hide conversations"
            title="Hide conversations (⌘⌥B)"
            className="shrink-0 text-muted-foreground"
            onClick={onToggle}
          >
            <SidebarSimple size={14} />
          </Button>
        </div>
        {/* `overflow-y: scroll`, not `auto`: the gutter has to be reserved whether
            or not the list overflows, because a bar that appears only once it does
            takes its width out of every row and jogs the column sideways as
            threads come and go. `scrollbar-gutter: stable` is the modern spelling
            and is a no-op in this WebKit — measured: it reports support and
            computes to `stable`, and reserves nothing. Always-on overflow does. */}
        <div className="flex flex-1 flex-col overflow-y-scroll px-2 pb-2">
          {groups.map((g) => {
            const open = !folded.has(g.key);
            const busy = g.threads.some((t) => runningIds.has(t.id));
            return (
              <div key={g.key} className="mb-1.5">
                {/* The label and its caret are one control on the left — the
                    caret says folded or not, which belongs beside the name and
                    not beside the `+`, where it read as a second button doing
                    something to the group. The right-hand slot holds the count
                    until hover swaps it for the new-thread `+`. */}
                <div className="group/head flex items-center gap-1 pl-2.5 pr-1 py-1">
                  <button
                    type="button"
                    title={g.title}
                    aria-expanded={open}
                    onClick={() => setGroupOpen(g.key, !open)}
                    className="flex min-w-0 flex-1 items-center gap-0.5 text-left text-[11px] font-medium tracking-wide text-muted-foreground transition-colors hover:text-foreground"
                  >
                    <span className="truncate">{g.label}</span>
                    <CaretRight
                      size={11}
                      className={cn(
                        "shrink-0 transition-[transform,opacity]",
                        // Folded, the caret is the state and stays; open, it is
                        // only an affordance and waits for the pointer.
                        open ? "rotate-90 opacity-0 group-hover/head:opacity-100" : "opacity-60",
                      )}
                    />
                  </button>
                  <div className="relative flex size-4 shrink-0 items-center justify-center">
                    <button
                      type="button"
                      aria-label={`New thread in ${g.label}`}
                      title={`New thread in ${g.label}`}
                      /* A thread started from a folded group would be started out
                         of sight, so the group opens with it. */
                      onClick={() => {
                        setGroupOpen(g.key, true);
                        onNew(g.subjectId);
                      }}
                      className="absolute inset-0 hidden items-center justify-center rounded text-muted-foreground transition-colors hover:text-foreground group-hover/head:flex"
                    >
                      <Plus size={11} weight="bold" />
                    </button>
                    {/* Folded, the group is the only place left to say that
                        something inside it is running — the rows carrying their
                        own spinners are folded away with it. */}
                    {busy && !open ? (
                      <CircleNotch size={11} className="animate-spin text-muted-foreground group-hover/head:hidden" />
                    ) : (
                      <span className="text-[10px] tabular-nums text-muted-foreground opacity-60 group-hover/head:hidden">
                        {g.threads.length}
                      </span>
                    )}
                  </div>
                </div>
                {open && (
                  <div className="flex flex-col gap-0.5">
                    {g.threads.map((t) => {
                      const running = runningIds.has(t.id);
                      const active = t.id === activeId;
                      return (
                        <div
                          key={t.id}
                          className={cn(
                            "group/thread flex rounded-lg text-xs transition-colors",
                            active
                              ? "bg-accent text-foreground"
                              : "text-muted-foreground hover:bg-accent hover:text-foreground",
                          )}
                        >
                          {/* The padding and the provider mark live *inside* the
                              button, and the row is left to stretch it — so the
                              whole row opens the thread, the way the sidebar's
                              own rows do. With the padding on this div instead,
                              the hit target was the title's 16px line box inside
                              a 28px row while the hover fill covered the dead 6px
                              strips above and below it: a click that looked
                              aimed, on a row that had lit up under the pointer,
                              landed on the div and did nothing. It read as the
                              app dropping clicks, and it showed up most when
                              switching threads fast, which is when you stop
                              settling on the text. */}
                          <button
                            type="button"
                            onClick={() => onOpen(t.id)}
                            className="flex min-w-0 flex-1 items-center gap-2 rounded-lg py-1.5 pl-2.5 text-left"
                          >
                            {/* Monochrome and `currentColor`, so it dims with the row
                                rather than sitting on it as a second colour. */}
                            <ProviderMark provider={t.provider} className="size-3.5 shrink-0 opacity-70" />
                            <span className="min-w-0 flex-1 truncate">{t.title || "Untitled"}</span>
                            {/* A thread opened in the lecture player's dock is
                                grouped under its subject like any other — it is
                                the same conversation and the same list — so the
                                one thing the row has to add is which kind of
                                scope it was given. The app's lecture icon,
                                because that is what it means everywhere else. */}
                            {t.lecture_id && (
                              <VideoCamera
                                size={11}
                                className="shrink-0 opacity-60"
                                aria-label="Lecture thread"
                              />
                            )}
                          </button>
                          {/* The one strip of the row that is not the thread: its
                              own controls, and a spinner in their place while a
                              turn runs. */}
                          <div className="flex shrink-0 items-center pl-1 pr-1.5">
                            {running ? (
                              <CircleNotch size={12} className="animate-spin text-muted-foreground" />
                            ) : confirming === t.id ? (
                              <ConfirmDelete
                                label={"Yes"}
                                onConfirm={() => {
                                  setConfirming(null);
                                  onDelete(t.id);
                                }}
                                onCancel={() => setConfirming(null)}
                              />
                            ) : (
                              <button
                                type="button"
                                aria-label="Delete thread"
                                onClick={() => setConfirming(t.id)}
                                className="rounded p-0.5 opacity-0 transition-opacity hover:text-foreground group-hover/thread:opacity-100"
                              >
                                <Trash size={12} />
                              </button>
                            )}
                          </div>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </aside>
  );
});

/**
 * The armed state of a row's delete: the confirm itself, and a way out.
 *
 * It takes focus on mount so that clicking anywhere else backs out — WebKit
 * does not focus a button when you click it, so the blur this leans on never
 * fired, and the row simply stuck on the confirm with no way back. Escape
 * cancels too, and the ✕ is the visible version of the same, for when the way
 * out should not have to be guessed at.
 */
function ConfirmDelete({
  label,
  onConfirm,
  onCancel,
}: {
  label: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLButtonElement>(null);
  // Mount only: a re-render mid-turn must not yank focus back out of whatever
  // the user has since started typing in.
  useEffect(() => ref.current?.focus(), []);
  return (
    <div className="flex shrink-0 items-center gap-0.5">
      <button
        ref={ref}
        type="button"
        onClick={onConfirm}
        onBlur={onCancel}
        onKeyDown={(e) => {
          if (e.key === "Escape") onCancel();
        }}
        className="rounded px-1 text-[10.5px] text-destructive hover:bg-destructive/10"
      >
        {label}
      </button>
      <button
        type="button"
        aria-label="Keep thread"
        title="Keep"
        /* mousedown, not click: the confirm button's blur lands first and
           would unmount this one before a click could land. */
        onMouseDown={(e) => {
          e.preventDefault();
          onCancel();
        }}
        className="rounded p-0.5 text-muted-foreground hover:text-foreground"
      >
        <X size={11} weight="bold" />
      </button>
    </div>
  );
}
