import { useState } from "react";
import { Plus } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ProjectDestinations } from "./ProjectPicker";
import type { DbProject } from "@/lib/projects";

/**
 * Add a task from the universal page — **unfiled by default**, with a picker
 * to file it in a project instead.
 *
 * That default is the point of the page: a task you think of while you are
 * looking at everything you have on is a task you have not decided where to
 * put yet, and making you decide before you can write it down is what an
 * inbox exists to avoid. It belongs to no project at all (migration 37), not
 * to a project called Inbox, so nothing has to be cleaned up if it never gets
 * filed.
 *
 * `NewProjectButton` is the same control one level up — a name field over a
 * destination list — and this follows it deliberately. The list itself is
 * `ProjectDestinations` (`./ProjectPicker.tsx`), shared with the refile picker
 * on a task's page and in the universal table, since "where shall this go" and
 * "where does this belong" have the same answer and had the same forty lines
 * twice. What stays here is the one difference: the pick does not write, it
 * only sets the destination the title is about to be created in.
 *
 * It does not pick a *column*. `createTask` files a new task in the first
 * column of the destination's board, which is Backlog on every default one —
 * where a card's life starts. `ProjectTable`'s inline add reaches for the
 * first column you actually work in instead, and the difference is the point:
 * that one is typed inside a project you are already working, this one is
 * something you have just remembered.
 */
export function NewTaskButton({
  projects,
  onCreate,
}: {
  /** Every project, archived included — `status: "all"`, as the page reads
   *  them for their boards. */
  projects: DbProject[];
  /** `null` is unfiled. */
  onCreate: (projectId: number | null, title: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("");
  const [projectId, setProjectId] = useState<number | null>(null);

  const commit = () => {
    const text = title.trim();
    if (!text) return;
    onCreate(projectId, text);
    setTitle("");
    setOpen(false);
  };

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        // A half-typed title is not worth keeping; the destination is, so a
        // second task for the same project is one field away.
        if (!next) setTitle("");
      }}
    >
      <PopoverTrigger asChild>
        <Button size="sm" className="shrink-0">
          <Plus size={13} weight="bold" />
          New task
        </Button>
      </PopoverTrigger>

      <PopoverContent align="end" className="w-72 p-3">
        <Input
          autoFocus
          value={title}
          placeholder="Task title"
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
          }}
          className="h-8 rounded-lg text-[13px]"
        />

        <p className="mt-3 mb-1 px-1 text-[11px] font-medium tracking-wide text-muted-foreground">
          In
        </p>
        <ProjectDestinations projects={projects} value={projectId} onPick={setProjectId} />

        <Button
          size="sm"
          disabled={!title.trim()}
          onClick={commit}
          className="mt-3 w-full"
        >
          Create
        </Button>
      </PopoverContent>
    </Popover>
  );
}
