import { useEffect, useState } from "react";
import { ArrowSquareOut, CalendarBlank, CaretDown, CaretUp, FileText, Note } from "@phosphor-icons/react";
import { TagDot } from "@/components/projects/tags/TagDot";
import { isTagColor } from "@/lib/planning/projects/tags";
import { Button } from "@/components/ui/button";
import { proposalInspect, type Proposal } from "@/lib/harness/proposals";
import { navigateActive } from "@/lib/shell/tabRouters";
import { openLibraryPath } from "@/lib/files/openFile";
import { taskHref } from "@/components/projects/nav/taskHref";
import { projectHref } from "@/components/projects/nav/projectHref";
import { cn } from "@/lib/utils";
import { StatusPill } from "@/components/projects/board/StatusPill";
import { ProjectIcon } from "@/components/projects/icons/ProjectIcon";
import { moveTask, type ProjectColumn } from "@/lib/planning/projects";
import { object, type Row } from "./model";

/** Rows shown before Show more; a project's results run to dozens of tasks. */
const SHOWN = 6;

export function Results({ proposal }: { proposal: Proposal }) {
  const [live, setLive] = useState<Record<string, Row | null>>({});
  const [error, setError] = useState<string>();
  const [all, setAll] = useState(false);
  const key = JSON.stringify(proposal.made);
  useEffect(() => {
    let active = true;
    const made = JSON.parse(key) as Proposal["made"];
    const groups = new Map<string, string[]>();
    for (const item of made) if (["task", "project", "event", "tag", "cheat", "question", "quiz", "artifact", "topic"].includes(item.entity)) groups.set(item.entity, [...(groups.get(item.entity) ?? []), item.id]);
    let run = 0;
    const load = () => {
      const current = ++run;
      void Promise.all([...groups].map(async ([entity, ids]) => {
        const entries = await proposalInspect(entity, ids);
        return Object.fromEntries(Object.entries(entries).map(([id, value]) => [`${entity}:${id}`, value]));
      })).then((entries) => { if (active && current === run) { setLive(Object.assign({}, ...entries)); setError(undefined); } }).catch((e) => { if (active && current === run) setError(String(e)); });
    };
    load();
    // Edits and deletes elsewhere redraw the rows' titles and struck-through state.
    const events = ["oculus:revision-updated", "oculus:projects-updated", "oculus:calendar-updated"];
    for (const name of events) window.addEventListener(name, load);
    return () => { active = false; for (const name of events) window.removeEventListener(name, load); };
  }, [key]);
  /** The made row's route; ⌘-click opens it in a new tab (`data-tab-href`). */
  const href = (item: Proposal["made"][number]): string | null => {
    const row = object(live[`${item.entity}:${item.id}`]);
    // Revision shows one subject, so a link opens the row's own.
    const revision = (path: string, topic = "all") => `/revision${path}?subject=${typeof row.subject_id === "number" ? row.subject_id : ""}&topic=${topic}`;
    if (item.entity === "cheat") return `${revision("")}#cheat-${item.id}`;
    if (item.entity === "question" || item.entity === "quiz") return revision("/quizzes");
    if (item.entity === "artifact") return revision("/artifacts");
    if (item.entity === "topic") return revision("", item.id);
    if (item.entity === "task") return taskHref(typeof row.project === "number" ? row.project : null, { id: Number(item.id), title: String(row.title ?? item.title ?? "Task") });
    if (item.entity === "project") return projectHref({ id: Number(item.id), name: String(row.name ?? item.title ?? "Project") });
    if (item.entity === "event") return "/calendar";
    if (item.entity === "tag") return `/tasks?tag=${encodeURIComponent(item.id)}`;
    return null;
  };
  // A note is a library file, not a route, so its ⌘-click is answered here.
  const open = (item: Proposal["made"][number], newTab: boolean) => {
    if (item.entity === "note") openLibraryPath(item.id, newTab);
    else { const to = href(item); if (to) navigateActive(to); }
  };
  return <div className="px-4 py-2">
    {(all ? proposal.made : proposal.made.slice(0, SHOWN)).map((item, i) => {
      const key = `${item.entity}:${item.id}`;
      // A made row read back as missing was deleted since (or by Undo); it keeps its title, struck through.
      const gone = item.op !== "delete" && key in live && live[key] === null;
      const current = object(live[key]);
      const title = String(current.title ?? current.name ?? item.title ?? item.id);
      const present = !gone && current.id != null;
      const deleted = item.op === "delete" || proposal.kind.startsWith("Delete");
      // A deletion is struck while it stands; Undo brings the item back.
      const struck = gone || (deleted && proposal.status === "created");
      const opens = (proposal.status === "created" || present) && !deleted && !gone && !(proposal.kind === "TagChanges" && Array.isArray(proposal.state.changes) && proposal.state.changes.some((change) => String(object(change).tag) === item.id));
      return <div key={`${key}:${i}`} className="-mx-2 flex min-h-8 min-w-0 items-center gap-2.5 rounded-md px-2 py-1 text-[13px] leading-5">
        {/* Each row leads with its kind: a task's live status (editable as in the tasks table), a project's icon, a tag's colour. */}
        <span className="flex size-5 shrink-0 items-center justify-center text-muted-foreground">
          {item.entity === "task" && present && proposal.status === "created" && typeof current.column === "string" && <StatusPill glyphOnly project={{ columns: current.columns as ProjectColumn[] }} columnId={current.column} onPick={(column) => moveTask(Number(item.id), column, null, null).catch((e) => setError(String(e)))} />}
          {item.entity === "project" && <ProjectIcon project={{ id: Number(item.id) }} size={15} />}
          {item.entity === "event" && <CalendarBlank size={15} />}
          {item.entity === "note" && <FileText size={15} />}
          {item.entity === "cheat" && <Note size={15} />}
          {item.entity === "tag" && isTagColor(current.color) && <TagDot color={current.color} />}
        </span>
        <span className={cn("min-w-0 flex-1 break-words", struck && "text-muted-foreground line-through decoration-muted-foreground/50")}>{title}</span>
        {struck && proposal.status === "created" && <span className="shrink-0 text-xs text-muted-foreground">Deleted</span>}
        {/* An undone change leaves its item in place, so it still opens. */}
        {opens && <button type="button" data-tab-href={href(item) ?? undefined} onClick={(e) => open(item, e.metaKey || e.ctrlKey)} onAuxClick={(e) => { if (e.button === 1) open(item, true); }} className="-mr-1 inline-flex h-6 shrink-0 cursor-pointer items-center gap-1 rounded-full px-2 text-xs text-muted-foreground transition-colors hover:bg-brand-muted hover:text-brand" aria-label={`Open ${title}`}>Open<ArrowSquareOut size={12} /></button>}
      </div>;
    })}
    {proposal.made.length > SHOWN && <Button variant="ghost" size="xs" className="-ml-2 text-muted-foreground" onClick={() => setAll(!all)}>{all ? <><CaretUp />Show less</> : <><CaretDown />Show {proposal.made.length - SHOWN} more</>}</Button>}
    {error && <div className="text-xs text-destructive">Couldn’t load current links: {error}</div>}
  </div>;
}
