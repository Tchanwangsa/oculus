import { useState } from "react";
import { Plus } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { ProjectDestinations } from "./ProjectPicker";
import type { DbProject } from "@/lib/planning/projects";

/**
 * Add a task from the universal page, unfiled by default. The destination pick
 * only sets where it will be created; `createTask` puts it in the board's
 * first column (Planning).
 */
export function NewTaskButton({
  projects,
  onCreate,
  align = "end",
  className,
}: {
  /** Archived included (`status: "all"`). */
  projects: DbProject[];
  /** `null` is unfiled. */
  onCreate: (projectId: number | null, title: string) => void;
  /** Which edge of the button the popover lines up with. */
  align?: "start" | "center" | "end";
  /** On the button, e.g. `w-full` for a column's pill. */
  className?: string;
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
        // Keep the destination across openings, not the half-typed title.
        if (!next) setTitle("");
      }}
    >
      <PopoverTrigger asChild>
        <Button size="sm" className={cn("shrink-0", className)}>
          <Plus size={13} weight="bold" />
          New task
        </Button>
      </PopoverTrigger>

      <PopoverContent align={align} className="w-72 p-3">
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
