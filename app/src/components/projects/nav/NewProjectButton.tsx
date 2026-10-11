import { useMemo, useState } from "react";
import { CaretRight, Check, Kanban, Plus } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode, displayName } from "@/lib/format/format";
import type { Subject } from "@/lib/db";

/**
 * Start a project in any group — the only way to a subject's first project,
 * since the index draws a group only once it has one. A plain list, not a
 * `Select` (a portal inside the popover).
 */
export function NewProjectButton({
  subjects,
  onCreate,
}: {
  subjects: Subject[];
  /** `null` is the Personal group. */
  onCreate: (subjectId: number | null, name: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [subjectId, setSubjectId] = useState<number | null>(null);
  const [pastOpen, setPastOpen] = useState(false);

  // Personal and this term's subjects; past terms fold away but stay reachable.
  const { listed, past } = useMemo(() => {
    const row = (s: Subject) => ({
      id: s.id as number | null,
      label: displayCode(s.code),
      title: displayName(s.name, s.code),
      subject: s,
    });
    return {
      listed: [
        {
          id: null as number | null,
          label: "Personal",
          title: "Not scoped to a subject",
          subject: null as Subject | null,
        },
        ...subjects.filter((s) => s.is_current).map(row),
      ],
      past: subjects.filter((s) => !s.is_current).map(row),
    };
  }, [subjects]);

  // The fold never hides the current selection.
  const pastSelected = past.some((g) => g.id === subjectId);

  const commit = () => {
    const title = name.trim();
    if (!title) return;
    onCreate(subjectId, title);
    setName("");
    setOpen(false);
  };

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        // Keep the group across openings, not the half-typed name.
        if (!next) setName("");
      }}
    >
      <PopoverTrigger asChild>
        <Button size="sm" className="shrink-0">
          <Plus size={13} weight="bold" />
          New project
        </Button>
      </PopoverTrigger>

      <PopoverContent align="end" className="w-72 p-3">
        <Input
          autoFocus
          value={name}
          placeholder="Project name"
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
          }}
          className="h-8 rounded-lg text-[13px]"
        />

        <p className="mt-3 mb-1 px-1 text-[11px] font-medium tracking-wide text-muted-foreground">
          In
        </p>
        <div className="-mx-1 max-h-52 overflow-y-auto px-1">
          {listed.map((g) => (
            <GroupRow
              key={g.id ?? "personal"}
              group={g}
              selected={g.id === subjectId}
              onPick={() => setSubjectId(g.id)}
            />
          ))}

          {past.length > 0 && (
            <Collapsible
              open={pastOpen || pastSelected}
              onOpenChange={setPastOpen}
              className="mt-1"
            >
              <CollapsibleTrigger className="flex w-full cursor-pointer items-center gap-1.5 rounded-md px-2 py-1.5 text-left text-[11px] font-medium text-muted-foreground transition-colors hover:text-foreground">
                <CaretRight
                  size={9}
                  className={cn(
                    "shrink-0 transition-transform",
                    (pastOpen || pastSelected) && "rotate-90",
                  )}
                />
                Past subjects ({past.length})
              </CollapsibleTrigger>
              <CollapsibleContent>
                {past.map((g) => (
                  <GroupRow
                    key={g.id ?? "personal"}
                    group={g}
                    selected={g.id === subjectId}
                    onPick={() => setSubjectId(g.id)}
                  />
                ))}
              </CollapsibleContent>
            </Collapsible>
          )}
        </div>

        <Button
          size="sm"
          disabled={!name.trim()}
          onClick={commit}
          className="mt-3 w-full"
        >
          Create
        </Button>
      </PopoverContent>
    </Popover>
  );
}

function GroupRow({
  group,
  selected,
  onPick,
}: {
  group: { label: string; title: string; subject: Subject | null };
  selected: boolean;
  onPick: () => void;
}) {
  return (
    <button
      type="button"
      title={group.title}
      onClick={onPick}
      className={cn(
        "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[12.5px] transition-colors",
        selected
          ? "bg-accent text-foreground"
          : "text-muted-foreground hover:bg-accent hover:text-foreground",
      )}
    >
      {group.subject ? (
        <SubjectIcon code={group.subject.code} size={13} />
      ) : (
        <Kanban size={13} className="shrink-0" />
      )}
      <span className="min-w-0 flex-1 truncate">{group.label}</span>
      {selected && <Check size={12} weight="bold" className="shrink-0 text-brand" />}
    </button>
  );
}
