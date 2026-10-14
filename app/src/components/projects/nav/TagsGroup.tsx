import { useState } from "react";
import { Plus } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { SIDE_NAV_FOLDS } from "@/components/ui/layout/SideNav";
import {
  MAX_TAG_NAME,
  TAG_COLORS,
  createTaskTag,
  normaliseTagName,
  type DbTaskTag,
} from "@/lib/planning/projects";
import { TagDot } from "../tags/TagDot";
import { tasksHref, type TaskViewId } from "../tasks/taskViews";
import { TagNavRow } from "./TagNavRow";

/**
 * The nav column's Tags group: the registry as rows that select a tag
 * (`/tasks?tag=<id>`, a second click clears it), and a `+` on the label that
 * opens an inline field for a new one. A failed add shows its reason under
 * the field.
 */
export function TagsGroup({
  tags,
  counts,
  view,
  selected,
  onDeleted,
}: {
  tags: DbTaskTag[];
  /** Tasks per tag id. */
  counts: Map<number, number>;
  /** The view a tag link keeps. */
  view: TaskViewId;
  selected: number | null;
  /** The tag is gone; `id` lets the caller leave a page that was showing it. */
  onDeleted: (id: number) => void;
}) {
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);

  const stop = () => {
    setAdding(false);
    setName("");
    setError(null);
  };

  const add = async () => {
    let clean: string;
    try {
      clean = normaliseTagName(name);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      return;
    }
    if (tags.some((t) => t.name.toLowerCase() === clean.toLowerCase())) {
      setError(`A tag named “${clean}” already exists.`);
      return;
    }
    try {
      // A new tag takes the next palette colour, so neighbours differ.
      await createTaskTag(clean, TAG_COLORS[tags.length % TAG_COLORS.length]);
      stop();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="mt-3">
      <div className="mb-0.5 flex items-center justify-between px-2 py-0.5 text-[11px] font-medium tracking-wide text-muted-foreground">
        <span className={SIDE_NAV_FOLDS}>Tags</span>
        <button
          type="button"
          aria-label="New tag"
          onClick={() => (adding ? stop() : setAdding(true))}
          className={cn(
            "flex size-5 cursor-pointer items-center justify-center rounded text-muted-foreground outline-none",
            "hover:bg-sidebar-item-hover hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50",
          )}
        >
          <Plus size={12} weight="bold" />
        </button>
      </div>

      {adding && (
        <div className="mb-1 px-2">
          <div className="flex h-8 items-center gap-2.5 rounded-md border border-input bg-card px-2 focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25">
            <span className="flex size-4 shrink-0 items-center justify-center">
              <TagDot color={TAG_COLORS[tags.length % TAG_COLORS.length]} />
            </span>
            <input
              autoFocus
              value={name}
              maxLength={MAX_TAG_NAME}
              onChange={(e) => {
                setName(e.target.value);
                setError(null);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  void add();
                } else if (e.key === "Escape") {
                  e.preventDefault();
                  stop();
                }
              }}
              onBlur={() => {
                if (!name.trim()) stop();
              }}
              placeholder="New tag"
              aria-label="New tag name"
              aria-invalid={error != null}
              spellCheck={false}
              autoComplete="off"
              className="min-w-0 flex-1 bg-transparent text-[12.5px] text-foreground outline-none placeholder:text-muted-foreground"
            />
          </div>
          {error && <p className="mt-1 px-0.5 text-[11px] text-destructive">{error}</p>}
        </div>
      )}

      <div className="flex flex-col gap-0.5">
        {tags.map((tag) => (
          <TagNavRow
            key={tag.id}
            tag={tag}
            to={tasksHref(view, selected === tag.id ? null : tag.id)}
            active={selected === tag.id}
            count={counts.get(tag.id) ?? 0}
            onDeleted={() => onDeleted(tag.id)}
          />
        ))}
      </div>

      {tags.length === 0 && !adding && (
        <p className="px-2 py-1 text-[12px] text-muted-foreground">No tags yet.</p>
      )}
    </div>
  );
}
