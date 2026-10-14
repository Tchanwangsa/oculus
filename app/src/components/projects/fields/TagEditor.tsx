import { useRef } from "react";
import { Plus } from "@phosphor-icons/react";
import { PILL } from "./pill";
import { cn } from "@/lib/utils";
import { createTaskTag, type DbTaskTag } from "@/lib/planning/projects";
import { useTaskTags } from "@/hooks/data/useTaskTags";
import { TagPicker } from "../tags/TagPicker";
import { TagChip } from "../tags/TagChip";

/** Project names resolve through the same registry and picker as task tags. */
export function TagEditor({ tags, onChange }: {
  tags: string[];
  onChange: (tags: string[]) => void;
}) {
  const { tags: registry } = useTaskTags();
  // TagPicker selects a new id before its registry refresh has rendered.
  const pending = useRef<DbTaskTag | null>(null);
  const names = new Set(tags.map(t => t.toLowerCase()));
  const chosen = registry.filter(t => names.has(t.name.toLowerCase()));
  return <div className="flex flex-wrap items-center gap-1.5">
    {chosen.map(tag => <TagChip key={tag.id} tag={tag} className="h-7 px-2.5 text-xs"
      onRemove={() => onChange(tags.filter(t => t.toLowerCase() !== tag.name.toLowerCase()))} />)}
    <TagPicker tags={registry} value={chosen.map(t => t.id)}
      onChange={ids => onChange(ids.flatMap(id => {
        const tag = registry.find(t => t.id === id) ?? (pending.current?.id === id ? pending.current : null);
        return tag ? [tag.name] : [];
      }))}
      onCreate={async name => {
        const tag = await createTaskTag(name);
        pending.current = tag;
        return tag;
      }}>
      <button type="button" aria-label="Add a tag" className={cn(PILL, "text-muted-foreground")}>
        <Plus size={12} weight="bold" />{tags.length === 0 && "Add a tag"}
      </button>
    </TagPicker>
  </div>;
}
