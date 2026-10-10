import { useEffect, useState } from "react";
import { cn } from "@/lib/utils";
import type { DbProject } from "@/lib/planning/projects";

/** Plain text, saved as `TaskPage`'s body is (blur or ⌘↵; unchanged writes
 *  nothing, empty writes `null`). The draft re-syncs only when the brief
 *  itself changes, not on every `PROJECTS_UPDATED_EVENT`. */
export function Brief({
  project,
  onSave,
}: {
  project: DbProject;
  onSave: (brief: string | null) => void;
}) {
  const [draft, setDraft] = useState(project.brief ?? "");
  useEffect(() => setDraft(project.brief ?? ""), [project.id, project.brief]);

  const save = () => {
    const brief = draft.trim();
    if (brief === (project.brief ?? "")) return;
    onSave(brief || null);
  };

  return (
    <textarea
      value={draft}
      placeholder="What is this project?"
      onChange={(e) => setDraft(e.target.value)}
      onBlur={save}
      onKeyDown={(e) => {
        // ⌘↵ saves without blurring.
        if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
          e.preventDefault();
          save();
        }
      }}
      className={cn(
        "field-sizing-content min-h-20 w-full resize-none whitespace-pre-wrap rounded-lg border border-transparent",
        "bg-transparent px-2 py-1.5 text-[13px] leading-relaxed text-foreground outline-none transition-colors",
        "hover:border-border-subtle focus:border-brand/40 focus:bg-card",
        "placeholder:text-muted-foreground/60",
      )}
    />
  );
}
