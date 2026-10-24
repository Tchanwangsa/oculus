import { lazy, Suspense, useEffect, useLayoutEffect, useRef, useState } from "react";
import { CompactMd } from "@/components/markdown/MdComponents";
import { SourceLink } from "@/components/revision/SourceLink";
import { BODY, PICKER, TITLE, TOPIC } from "@/components/revision/cheat/CheatEntry";
import { CheatTags } from "@/components/revision/cheat/CheatTags";
import { useSubjects } from "@/hooks/data/useSubjects";
import { displayCode } from "@/lib/format/format";
import type { RevisionTopic } from "@/lib/revision/types";
import { cn } from "@/lib/utils";
import { scrollParent } from "@/lib/ui/scrollAnchor";
import { Include, RowActions } from "./ProposalParts";
import { TopicNamesField } from "./RevisionFields";
import { SubjectPicker } from "./SubjectPicker";
import { included, type Row } from "./model";

const loadField = () => import("@/components/revision/MarkdownField").then((m) => ({ default: m.MarkdownField }));
const MarkdownField = lazy(loadField);

const strings = (value: unknown) => Array.isArray(value) ? value.filter((v): v is string => typeof v === "string") : [];

/**
 * One proposed cheat sheet entry, drawn as the cheat sheet draws it: title,
 * body, then a row of subject, topic, tag and source chips. Edit (or a
 * double-click) edits it in place in the same type, as the cheat sheet does,
 * so only the body's toolbar moves anything.
 */
export function CheatRow({ row, allowed, previous, topics = [], disabled, onChange }: {
  row: Row; allowed: boolean; previous?: Row; topics?: RevisionTopic[]; disabled: boolean; onChange: (row: Row) => void;
}) {
  const { subjects } = useSubjects();
  const [before, setBefore] = useState<Row | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const [readingHeight, setReadingHeight] = useState(0);
  const scrollBeforeEdit = useRef<{ element: HTMLElement; top: number } | null>(null);
  const editing = before !== null;
  const title = String(row.title ?? "Untitled");
  const body = String(row.body ?? "");
  const on = allowed && included(row);
  const subject = typeof row.subject === "string" && row.subject ? row.subject : null;
  const names = strings(row.topics), tags = strings(row.tags);
  const source = typeof row.source === "string" && row.source ? row.source : null;
  const changed = previous != null && JSON.stringify(row.title) !== JSON.stringify(previous.title);
  const start = () => {
    if (disabled || editing) return;
    const scroller = root.current && scrollParent(root.current);
    scrollBeforeEdit.current = scroller ? { element: scroller, top: scroller.scrollTop } : null;
    // The editor measures and wraps over its first frames. Keep the prose's
    // height so the chat cannot clamp its scroll offset in the meantime.
    const height = root.current?.offsetHeight ?? 0;
    // WebKit can clamp scroll synchronously while React replaces children,
    // before React applies the ancestor's new style or layout effects run.
    if (root.current) root.current.style.minHeight = `${height}px`;
    setReadingHeight(height);
    setBefore(row);
  };
  useLayoutEffect(() => {
    const held = scrollBeforeEdit.current;
    if (!editing || !held) return;
    let cancelled = false;
    const resize = new ResizeObserver(() => restore());
    const cancel = () => { cancelled = true; resize.disconnect(); };
    const restore = () => {
      if (!cancelled && Math.abs(held.element.scrollTop - held.top) > 0.5) held.element.scrollTop = held.top;
    };
    // The lazy toolbox and editor can finish measuring after the first frames.
    // Keep their resize from moving the header until the next user input.
    restore();
    if (root.current) resize.observe(root.current);
    let second = 0;
    const first = requestAnimationFrame(() => {
      restore();
      second = requestAnimationFrame(restore);
    });
    const inputs = ["wheel", "touchstart", "pointerdown", "keydown"] as const;
    for (const name of inputs) document.addEventListener(name, cancel, { passive: true, capture: true });
    return () => {
      resize.disconnect();
      cancelAnimationFrame(first);
      cancelAnimationFrame(second);
      for (const name of inputs) document.removeEventListener(name, cancel, true);
    };
  }, [editing]);
  // Fetched ahead so Edit swaps in the editor without a loading frame.
  useEffect(() => { if (!disabled) void loadField(); }, [disabled]);

  const rendered = <CompactMd text={body} className={BODY} />;
  return <div ref={root} style={editing ? { minHeight: readingHeight } : undefined} className="border-t border-border-subtle px-4 py-3">
    <div className="flex min-w-0 items-start gap-3">
      <Include title={title} checked={on} disabled={disabled || !allowed} onChange={(checked) => onChange({ ...row, included: checked })} />
      <div className="min-w-0 flex-1">
        <div className="flex items-start gap-2">
          {editing
            ? <input aria-label="Entry title" disabled={disabled} value={String(row.title ?? "")} onChange={(e) => onChange({ ...row, title: e.target.value })} onKeyDown={(e) => { if (e.key === "Enter" && !e.nativeEvent.isComposing) e.currentTarget.blur(); }} className={cn(TITLE, "h-[1lh] min-w-0 flex-1 appearance-none rounded-none border-0 bg-transparent p-0 text-foreground outline-none")} />
            : <div role="heading" aria-level={3} onDoubleClick={start} className={cn(TITLE, "min-w-0 flex-1 break-words", !on && "text-muted-foreground line-through")}>
              {title}{changed && <span className="ml-1.5 inline-block size-1.5 rounded-full bg-brand align-middle" aria-label="Updated title" />}
            </div>}
          <div className="-mt-0.5 -mr-1.5"><RowActions title={title} editing={editing} disabled={disabled} canRemove={on} onEdit={start} onSave={() => setBefore(null)} onCancel={() => { onChange(before!); setBefore(null); }} onRemove={() => onChange({ ...row, included: false })} /></div>
        </div>
        {editing
          ? <div className="mt-1"><Suspense fallback={rendered}><MarkdownField compact inline text={body} subjectId={subjects.find((s) => s.code === subject)?.id ?? null} onCommit={(next) => onChange({ ...row, body: next })} onChange={(next) => onChange({ ...row, body: next })} /></Suspense></div>
          : body && <div onDoubleClick={start} className={cn("mt-1", BODY, !on && "opacity-60")}>{rendered}</div>}
        {(editing || subject || names.length > 0 || tags.length > 0 || source) && <div className="mt-2.5 flex flex-wrap items-center gap-1.5 text-[11px] text-muted-foreground">
          {editing ? <>
            <SubjectPicker value={subject} triggerClassName={PICKER} onChange={(code) => onChange({ ...row, subject: code })} />
            <TopicNamesField value={row.topics} topics={topics} subject={subject} triggerClassName={PICKER} onChange={(next) => onChange({ ...row, topics: next })} />
          </> : <>
            {subject && <span className={TOPIC}>{displayCode(subject)}</span>}
            {names.map((name) => <span key={name} className={TOPIC}>{name}</span>)}
          </>}
          <CheatTags value={tags} onChange={editing ? (next) => onChange({ ...row, tags: next }) : undefined} disabled={disabled} />
          <SourceLink source={source} />
        </div>}
      </div>
    </div>
  </div>;
}
