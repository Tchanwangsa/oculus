import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { taskHref } from "@/components/projects/nav/taskHref";
import type { DbProjectTask } from "@/lib/planning/projects";
import { cn } from "@/lib/utils";

/** The title, editable in place. Empty or unchanged reverts rather than
 *  commits: a nameless task cannot be found on any view. */
export function TaskTitle({
  task,
  onRename,
}: {
  task: DbProjectTask;
  onRename: (title: string) => void;
}) {
  const navigate = useNavigate();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(task.title);
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing) ref.current?.select();
  }, [editing]);

  const commit = () => {
    setEditing(false);
    const title = draft.trim();
    if (!title || title === task.title) {
      setDraft(task.title);
      return;
    }
    onRename(title);
    // The tab strip titles a tab from its path (`taskHref`), so re-navigate;
    // replace keeps the back arrow where it was.
    navigate(taskHref(task.project_id, { id: task.id, title }), { replace: true });
  };

  const edit = () => {
    setDraft(task.title);
    setEditing(true);
  };

  const shared =
    "mt-2 w-full text-[22px] font-semibold leading-tight tracking-tight outline-none";

  if (editing) {
    return (
      <input
        ref={ref}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
          if (e.key === "Escape") {
            setDraft(task.title);
            setEditing(false);
          }
        }}
        className={cn(shared, "rounded-md bg-transparent text-foreground")}
      />
    );
  }

  return (
    <h1
      tabIndex={0}
      onClick={edit}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          edit();
        }
      }}
      className={cn(
        shared,
        "cursor-text rounded-md",
        task.done_at ? "text-muted-foreground line-through" : "text-foreground",
      )}
    >
      {task.title}
    </h1>
  );
}
