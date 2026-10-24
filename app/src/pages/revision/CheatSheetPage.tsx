import { useEffect, useRef, useState } from "react";
import { useLocation } from "react-router-dom";
import { DotsSixVertical, Plus } from "@phosphor-icons/react";
import { RevisionPage } from "@/components/revision/RevisionPage";
import { CheatEntry } from "@/components/revision/cheat/CheatEntry";
import { CheatTags } from "@/components/revision/cheat/CheatTags";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { PillTabs } from "@/components/ui/table/PillTabs";
import {
  DRAG_SURFACE,
  useCardDrag,
  useSettledList,
} from "@/hooks/gestures/useCardDrag";
import { useRevisionData } from "@/layouts/RevisionLayout";
import {
  createNote,
  updateNote,
  deleteNote,
  moveNote,
  type RevisionNote,
} from "@/lib/revision";
import { cn } from "@/lib/utils";

type Grouping = "topic" | "tag";

interface Group {
  /** The group's drag container id. */
  key: string;
  title: string;
  /** The topic a drop into this group files the entry under; `null` is
   *  Unsorted, and a tag group takes no drops from other groups. */
  topicId: number | null;
  entries: RevisionNote[];
}

export default function CheatSheetPage() {
  const location = useLocation();
  const { notes, scope, topics, creationTopic, topicScope } = useRevisionData();
  const [query, setQuery] = useState("");
  const [group, setGroup] = useState<Grouping>("topic");
  // One entry edits at a time: editing another ends this one, which saves.
  const [editing, setEditing] = useState<number | null>(null);
  const [error, setError] = useState<string>();
  // A linked entry is scrolled to once, when it first appears: every save or
  // move re-reads the notes and must leave the scroll where it is.
  const scrolledTo = useRef<string | null>(null);
  useEffect(() => {
    const id = location.hash.slice(1);
    if (!id || scrolledTo.current === id) return;
    const el = document.getElementById(id);
    if (!el) return;
    scrolledTo.current = id;
    el.scrollIntoView({ block: "center" });
  }, [notes, location.hash]);
  const run = (fn: () => Promise<unknown>) => {
    setError(undefined);
    void fn().catch((e) => setError(String(e)));
  };

  // The drop resolves against refs: the groups below need the gesture's
  // state, which needs this callback.
  const groupsRef = useRef<Group[]>([]);
  const drag = useCardDrag(
    ({ id, from, containerId, before, after }) => {
      const groups = groupsRef.current;
      const note = groups.flatMap((g) => g.entries).find((n) => n.id === id);
      if (!note) return;
      if (from === containerId) {
        run(() => moveNote(id, before, after));
        return true;
      }
      const source = groups.find((g) => g.key === from);
      const target = groups.find((g) => g.key === containerId);
      if (group !== "topic" || !source || !target) return;
      const unsorted = new Set(topics.filter((t) => t.is_unsorted).map((t) => t.id));
      const topic_ids = note.topic_ids.filter(
        (t) => t !== source.topicId && !unsorted.has(t),
      );
      if (target.topicId != null) topic_ids.push(target.topicId);
      run(() => moveNote(id, before, after, { topic_ids }));
      return true;
    },
    { settleOn: notes, autoScroll: true },
  );
  const live = drag.drag;
  // Over another tag's group a drop does nothing, so nothing slides apart.
  const frozen =
    live != null && group === "tag" && live.targetContainerId !== live.containerId;

  const shown = useSettledList(notes, live).filter((n) =>
    `${n.title} ${n.body} ${n.tags.join(" ")}`
      .toLowerCase()
      .includes(query.toLowerCase()),
  );
  const byKey = new Map<string, Group>();
  for (const n of shown) {
    const topic =
      group === "topic"
        ? topics.find(
            (t) =>
              n.topic_ids.includes(t.id) &&
              !t.is_unsorted &&
              (typeof topicScope !== "number" || t.id === topicScope),
          )
        : undefined;
    const title =
      group === "topic" ? (topic?.name ?? "Unsorted") : (n.tags[0] ?? "Untagged");
    const key = group === "topic" ? `topic:${topic?.id ?? "unsorted"}` : `tag:${title}`;
    const g = byKey.get(key);
    if (g) g.entries.push(n);
    else byKey.set(key, { key, title, topicId: topic?.id ?? null, entries: [n] });
  }
  const groups = [...byKey.values()];
  groupsRef.current = groups;

  /** Arrow keys on a grip move its entry one place within its group. */
  const nudge = (entries: RevisionNote[], index: number, by: -1 | 1) => {
    const to = index + by;
    if (to < 0 || to >= entries.length) return;
    const [before, after] =
      by < 0 ? [entries[to - 1], entries[to]] : [entries[to], entries[to + 1]];
    run(() =>
      moveNote(entries[index].id, before?.id ?? null, after?.id ?? null),
    );
  };

  const tabs: { value: Grouping; label: string }[] = [
    { value: "topic", label: "Topic" },
    { value: "tag", label: "Tag" },
  ];
  return (
    <RevisionPage
      title="Cheat sheet"
      actions={
        <Button
          size="sm"
          disabled={scope === "general"}
          onClick={() =>
            run(async () =>
              setEditing(
                await createNote({
                  title: "Untitled",
                  body: "",
                  subject_id: creationTopic
                    ? creationTopic.subject_id
                    : typeof scope === "number"
                      ? scope
                      : null,
                  topic_ids: creationTopic ? [creationTopic.id] : undefined,
                }),
              ),
            )
          }
        >
          <Plus />
          New entry
        </Button>
      }
    >
      <div className="flex items-center gap-3">
        <Input
          aria-label="Search cheat sheet"
          placeholder="Search definitions, formulas and tags"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          className="min-w-0 flex-1"
        />
        <div className="flex shrink-0 items-center gap-1.5 text-[11.5px] text-muted-foreground">
          Group by
          <PillTabs tabs={tabs} value={group} onChange={setGroup} />
        </div>
      </div>
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      {!shown.length && (
        <p className="py-10 text-center text-sm text-muted-foreground">
          {notes.length
            ? "No matching entries."
            : "Keep useful definitions, formulas and examples here."}
        </p>
      )}
      {groups.map(({ key, title, entries }) => {
        const incoming =
          live != null &&
          !frozen &&
          live.targetContainerId === key &&
          live.containerId !== key;
        return (
          <section key={key} className="space-y-2.5">
            <h2
              className={cn(
                "flex items-center gap-2 text-[13px] font-semibold transition-colors",
                incoming ? "text-brand" : "text-muted-foreground",
              )}
            >
              {group === "tag" && title !== "Untagged" ? <CheatTags value={[title]} /> : title}
              <span className="text-[11.5px] font-normal tabular-nums">{entries.length}</span>
            </h2>
            <div ref={drag.containerRef(key)} className="flex flex-col gap-2.5">
              {entries.map((note, index) => {
                const grabbed = live?.id === note.id;
                const shift = grabbed || frozen ? 0 : drag.shiftFor(key, index);
                return (
                  <div
                    key={note.id}
                    ref={drag.itemRef(key, note.id)}
                    style={
                      grabbed
                        ? { transform: `translateY(${live.dy + live.scroll}px)` }
                        : shift
                          ? { transform: `translateY(${shift}px)` }
                          : undefined
                    }
                    className={cn(
                      "group/item relative rounded-xl",
                      grabbed
                        ? cn("z-10", live.settling ? "transition-transform duration-200 ease-out" : "shadow-lg")
                        : live && "transition-transform duration-200 ease-out",
                    )}
                  >
                    <button
                      type="button"
                      aria-label={`Move ${note.title}: the arrow keys move it`}
                      onPointerDown={(e) => drag.onPointerDown(e, { id: note.id, containerId: key })}
                      onKeyDown={(e) => {
                        if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
                        e.preventDefault();
                        nudge(entries, index, e.key === "ArrowUp" ? -1 : 1);
                      }}
                      className={cn(
                        "absolute top-2.5 -left-6 flex h-6 w-6 items-center justify-center rounded-md",
                        "text-muted-foreground/70 outline-none transition-opacity hover:text-foreground",
                        "focus-visible:opacity-100 focus-visible:ring-[3px] focus-visible:ring-ring/50",
                        DRAG_SURFACE,
                        grabbed
                          ? "cursor-grabbing opacity-100"
                          : cn("cursor-grab opacity-0", !live && "group-hover/item:opacity-100"),
                      )}
                    >
                      <DotsSixVertical size={14} weight="bold" />
                    </button>
                    <CheatEntry
                      note={note}
                      topics={topics}
                      editing={editing === note.id}
                      onEditingChange={(on) =>
                        setEditing((cur) => (on ? note.id : cur === note.id ? null : cur))
                      }
                      save={(patch) => run(() => updateNote(note.id, patch))}
                      remove={() => run(() => deleteNote(note.id))}
                    />
                  </div>
                );
              })}
            </div>
          </section>
        );
      })}
    </RevisionPage>
  );
}
