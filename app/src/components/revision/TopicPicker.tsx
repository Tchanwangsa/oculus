import { useRevisionStore } from "@/stores/revision/revisionStore";
import { FilterPopover } from "@/components/ui/filter/FilterPopover";
import { FilterChecklist } from "@/components/ui/filter/FilterChecklist";

/** Unsorted is the writer's default when no named topics are selected. */
export function TopicPicker({
  subjectId,
  value,
  onChange,
  triggerClassName,
}: {
  subjectId: number | null;
  value: number[];
  onChange: (ids: number[]) => void;
  /** Over the filter pill's size, e.g. to sit among chips. */
  triggerClassName?: string;
}) {
  const topics = useRevisionStore((state) => state.topics).filter(
    (topic) => topic.subject_id === subjectId && !topic.is_unsorted,
  );
  const selected = new Set(
    value.filter((id) => topics.some((topic) => topic.id === id)).map(String),
  );
  const names = topics
    .filter((topic) => selected.has(String(topic.id)))
    .map((topic) => topic.name);
  return (
    <FilterPopover
      label={names.length ? names.join(", ") : "Unsorted"}
      active={names.length > 0}
      triggerClassName={triggerClassName}
    >
      {() => (
        <>
          <p className="px-2 py-1 text-xs text-muted-foreground">Topics</p>
          <FilterChecklist
            noun="topics"
            options={topics.map((topic) => ({
              value: String(topic.id),
              label: topic.name,
            }))}
            selected={selected}
            onChange={(next) => onChange([...next].map(Number))}
          />
          {!topics.length && (
            <p className="px-2 py-2 text-xs text-muted-foreground">
              Create a topic in the Revision sidebar.
            </p>
          )}
        </>
      )}
    </FilterPopover>
  );
}
