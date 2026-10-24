import { useState } from "react";
import { useLocation } from "react-router-dom";
import { FolderSimple, Plus, Tray } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { SIDE_NAV_FOLDS } from "@/components/ui/layout/SideNav";
import {
  createTopic,
  type RevisionTopic,
  type RevisionTopicScope,
} from "@/lib/revision";
import { TopicNavRow } from "./TopicNavRow";

/**
 * The Revision nav's Topics group, shaped like Tasks' Tags: Unsorted and each
 * named topic as rows that filter every page (`?topic=`, a second click
 * clears it), and a `+` on the label for an inline new-topic field in the
 * scoped subject.
 */
export function TopicsGroup({
  topics,
  counts,
  unsorted,
  selected,
  subjectId,
}: {
  /** The scope's topics, Unsorted included. */
  topics: RevisionTopic[];
  /** Entries per topic id. */
  counts: Map<number, number>;
  /** Entries in any Unsorted topic of the scope. */
  unsorted: number;
  selected: RevisionTopicScope;
  /** Where a new topic goes. */
  subjectId: number;
}) {
  const { pathname, search } = useLocation();
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);

  const href = (topic: RevisionTopicScope) => {
    const params = new URLSearchParams(search);
    params.set("topic", String(selected === topic ? "all" : topic));
    return `${pathname}?${params}`;
  };

  const stop = () => {
    setAdding(false);
    setName("");
    setError(null);
  };

  const add = async () => {
    const clean = name.trim();
    if (!clean) return;
    if (topics.some((t) => t.name.toLowerCase() === clean.toLowerCase())) {
      setError(`A topic named “${clean}” already exists.`);
      return;
    }
    try {
      await createTopic(subjectId, clean);
      stop();
    } catch (e) {
      setError(String(e));
    }
  };

  const named = topics.filter((t) => !t.is_unsorted);

  return (
    <div className="mt-4">
      <div className="mb-0.5 flex h-6 items-center justify-between px-2 text-[11px] font-medium tracking-wide text-muted-foreground">
        <span className={SIDE_NAV_FOLDS}>Topics</span>
        <button
          type="button"
          aria-label="New topic"
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
            <FolderSimple
              size={16}
              className="shrink-0 text-muted-foreground"
            />
            <input
              autoFocus
              value={name}
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
              placeholder="New topic"
              aria-label="New topic name"
              aria-invalid={error != null}
              spellCheck={false}
              autoComplete="off"
              className="min-w-0 flex-1 bg-transparent text-[12.5px] text-foreground outline-none placeholder:text-muted-foreground"
            />
          </div>
          {error && (
            <p className="mt-1 px-0.5 text-[11px] text-destructive">{error}</p>
          )}
        </div>
      )}

      <div className="flex flex-col gap-0.5">
        <TopicNavRow
          label="Unsorted"
          icon={<Tray size={16} />}
          to={href("unsorted")}
          active={selected === "unsorted"}
          count={unsorted}
        />
        {named.map((topic) => (
          <TopicNavRow
            key={topic.id}
            topic={topic}
            label={topic.name}
            icon={<FolderSimple size={16} />}
            to={href(topic.id)}
            active={selected === topic.id}
            count={counts.get(topic.id) ?? 0}
          />
        ))}
      </div>

      {named.length === 0 && !adding && (
        <p
          className={cn(
            "px-2 py-1 text-[12px] text-muted-foreground",
            SIDE_NAV_FOLDS,
          )}
        >
          No topics yet.
        </p>
      )}
    </div>
  );
}
