import { Plus } from "@phosphor-icons/react";
import { PILL } from "@/components/projects/fields/pill";
import { TagChip } from "@/components/projects/tags/TagChip";
import { TagPicker } from "@/components/projects/tags/TagPicker";
import { useTaskTags } from "@/hooks/data/useTaskTags";
import { createTaskTag, type DbProjectTask } from "@/lib/planning/projects";
import { cn } from "@/lib/utils";

/**
 * The task's tags as removable chips, then the picker: a labelled pill while
 * there are none, a round + after them. Each chip's × is its own button,
 * outside the picker's trigger.
 */
export function TaskTags({
  task,
  onChange,
}: {
  task: DbProjectTask;
  onChange: (tagIds: number[]) => void;
}) {
  const { tags, byId } = useTaskTags();
  const chosen = task.tag_ids.flatMap((id) => byId.get(id) ?? []);

  return (
    <>
      {chosen.map((tag) => (
        <TagChip
          key={tag.id}
          tag={tag}
          className="h-7 gap-1.5 px-2.5 text-xs"
          onRemove={() => onChange(task.tag_ids.filter((id) => id !== tag.id))}
        />
      ))}
      <TagPicker
        tags={tags}
        value={task.tag_ids}
        onChange={onChange}
        onCreate={(name) => createTaskTag(name)}
      >
        {chosen.length === 0 ? (
          <button
            type="button"
            aria-label="Add tag"
            className={cn(PILL, "text-muted-foreground")}
          >
            <Plus size={12} weight="bold" className="shrink-0" />
            Tag
          </button>
        ) : (
          <button
            type="button"
            aria-label="Add tag"
            className={cn(PILL, "size-7 justify-center px-0 text-muted-foreground")}
          >
            <Plus size={12} weight="bold" />
          </button>
        )}
      </TagPicker>
    </>
  );
}
