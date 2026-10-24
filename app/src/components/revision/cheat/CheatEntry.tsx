import { useEffect, useRef, useState } from "react";
import { Check, PencilSimple, Trash } from "@phosphor-icons/react";
import { CompactMd } from "@/components/markdown/MdComponents";
import { MarkdownField } from "@/components/revision/MarkdownField";
import { SourceLink } from "@/components/revision/SourceLink";
import { TopicPicker } from "@/components/revision/TopicPicker";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import type { RevisionNote, RevisionTopic } from "@/lib/revision";
import { cn } from "@/lib/utils";
import { CheatTags } from "./CheatTags";

/** The title's type, shared by the heading and its field so editing never
 *  changes the entry's size, and by the chat's proposed entries. The tracking
 *  is the headings' own (base.css), which a field does not get. */
export const TITLE = "font-display text-[15px] font-semibold leading-snug tracking-[-0.02em]";
/** The body's type: the compact note field's 13px and 1.6 line height. */
export const BODY = "text-[13px] leading-[1.6]";
/** The topic chip's size, the same as a tag chip's. */
export const TOPIC = "inline-flex h-5 items-center rounded-full bg-surface px-2 text-[11px] leading-none";
/** A picker's trigger drawn as the chip it edits. */
export const PICKER = cn(TOPIC, "gap-1 border-transparent font-normal text-muted-foreground shadow-none has-[>svg]:px-2");

/**
 * One cheat sheet entry. Its title and body select like any content. Edit (or
 * a double-click) edits it in place: the title becomes a field in the same
 * type, the body the inline compact note field under its toolbar, and the
 * topic and tag chips gain pickers. Each part saves as it changes or blurs, or
 * when editing ends; Done or a press outside the entry leaves editing. Whether
 * it is editing is the page's, so only one entry edits at a time.
 */
export function CheatEntry({
  note,
  topics,
  editing,
  onEditingChange,
  save,
  remove,
}: {
  note: RevisionNote;
  /** Every topic, to name the entry's own. */
  topics: RevisionTopic[];
  editing: boolean;
  onEditingChange: (editing: boolean) => void;
  save: (patch: Partial<RevisionNote>) => void;
  remove: () => void;
}) {
  const [title, setTitle] = useState(note.title);
  const [confirming, setConfirming] = useState(false);

  // Another entry taking over unmounts the title field without a blur, so the
  // draft is saved when editing ends; `saved` keeps the blur from saving twice.
  const draft = useRef(title);
  draft.current = title;
  const saved = useRef(note.title);
  saved.current = note.title;
  const saveRef = useRef(save);
  saveRef.current = save;
  const commitTitle = () => {
    const next = draft.current.trim();
    if (next && next !== saved.current) {
      saved.current = next;
      saveRef.current({ title: next });
    } else setTitle(saved.current);
  };
  const commitRef = useRef(commitTitle);
  commitRef.current = commitTitle;
  useEffect(() => {
    if (!editing) return;
    setTitle(saved.current);
    return () => commitRef.current();
  }, [editing]);

  // A press outside ends editing. React events cross portals, so the pickers
  // and dialogs opened from the entry count as inside; the document resets the
  // mark on capture and reads it once the press has bubbled.
  const inside = useRef(false);
  const leaveRef = useRef(onEditingChange);
  leaveRef.current = onEditingChange;
  useEffect(() => {
    if (!editing) return;
    const reset = () => {
      inside.current = false;
    };
    const onDown = (e: PointerEvent) => {
      if (inside.current || e.button !== 0 || !(e.target instanceof Element)) return;
      // The note field's completion list and maths toolbox sit on <body>.
      if (e.target.closest(".cm-tooltip")) return;
      // A press on a scrollbar scrolls the page rather than leaving.
      const el = e.target;
      if (el instanceof HTMLElement && (e.offsetX > el.clientWidth || e.offsetY > el.clientHeight)) return;
      leaveRef.current(false);
    };
    document.addEventListener("pointerdown", reset, true);
    document.addEventListener("pointerdown", onDown);
    return () => {
      document.removeEventListener("pointerdown", reset, true);
      document.removeEventListener("pointerdown", onDown);
    };
  }, [editing]);

  const start = () => {
    // The double-click's word selection would outlive the text it was in.
    window.getSelection()?.removeAllRanges();
    onEditingChange(true);
  };

  const named = topics.filter((t) => !t.is_unsorted && note.topic_ids.includes(t.id));

  return (
    <article
      id={`cheat-${note.id}`}
      onPointerDownCapture={() => {
        inside.current = true;
      }}
      className={cn(
        "group/entry min-w-0 rounded-xl border bg-card px-4 py-3 transition-colors",
        editing ? "border-brand/40" : "border-border",
      )}
    >
      <div className="flex items-start gap-2">
        {editing ? (
          <input
            autoFocus={!note.body}
            aria-label="Entry title"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            onBlur={commitTitle}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.nativeEvent.isComposing) e.currentTarget.blur();
              if (e.key === "Escape") setTitle(note.title);
            }}
            className={cn(TITLE, "min-w-0 flex-1 bg-transparent p-0 text-foreground outline-none")}
          />
        ) : (
          <h3 data-selectable className={cn(TITLE, "min-w-0 flex-1")} onDoubleClick={start}>
            {note.title}
          </h3>
        )}
        <div
          className={cn(
            "-mt-0.5 -mr-1.5 flex transition-opacity",
            !editing && "opacity-0 group-hover/entry:opacity-100 focus-within:opacity-100",
          )}
        >
          {editing ? (
            <Button variant="ghost" size="icon-xs" aria-label="Done editing" onClick={() => onEditingChange(false)}>
              <Check />
            </Button>
          ) : (
            <Button variant="ghost" size="icon-xs" aria-label={`Edit ${note.title}`} onClick={start}>
              <PencilSimple />
            </Button>
          )}
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label={`Delete ${note.title}`}
            onClick={() => setConfirming(true)}
          >
            <Trash />
          </Button>
        </div>
      </div>
      {editing ? (
        // Inline: the text sits where the rendered body's did, and only the
        // toolbar above it moves anything.
        <div className="mt-1">
          <MarkdownField
            compact
            inline
            text={note.body}
            subjectId={note.subject_id}
            onCommit={(next) => {
              if (next !== note.body) save({ body: next });
            }}
          />
        </div>
      ) : (
        <div data-selectable className={cn("mt-1", BODY)} onDoubleClick={start}>
          <CompactMd text={note.body} className={BODY} />
        </div>
      )}
      {(editing || named.length > 0 || note.tags.length > 0 || note.source_ref) && (
        <div className="mt-2.5 flex flex-wrap items-center gap-1.5 text-[11px] text-muted-foreground">
          {editing ? (
            <TopicPicker
              subjectId={note.subject_id}
              value={note.topic_ids}
              onChange={(topic_ids) => save({ topic_ids })}
              triggerClassName={PICKER}
            />
          ) : (
            named.map((t) => (
              <span key={t.id} className={TOPIC}>
                {t.name}
              </span>
            ))
          )}
          <CheatTags value={note.tags} onChange={editing ? (tags) => save({ tags }) : undefined} />
          <SourceLink source={note.source_ref} />
        </div>
      )}
      <ConfirmDialog
        open={confirming}
        onCancel={() => setConfirming(false)}
        onConfirm={() => {
          setConfirming(false);
          remove();
        }}
        title={`Delete “${note.title}”?`}
        description="The entry is removed from your cheat sheet."
        confirmLabel="Delete entry"
      />
    </article>
  );
}
