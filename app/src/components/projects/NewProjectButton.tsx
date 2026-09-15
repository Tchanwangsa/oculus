import { useMemo, useState } from "react";
import { Check, Kanban, Plus } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode, displayName } from "@/lib/format";
import type { Subject } from "@/lib/db";

/**
 * Start a project in any group, from one control.
 *
 * The index only draws a group once it has something in it — that is what
 * keeps a term's worth of empty subject headings off the page — so the inline
 * composer on a group can only ever add to a subject that already has
 * projects. This is the other door: the group is part of what you are filling
 * in rather than something you have to find a heading for first, which is what
 * makes a subject's *first* project reachable at all.
 *
 * The picker is a plain list rather than a `Select`: a select inside a popover
 * is a portal inside a portal, and the rows here want a subject's icon and its
 * full name beside the code anyway.
 */
export function NewProjectButton({
  subjects,
  onCreate,
}: {
  subjects: Subject[];
  /** `null` is the Personal group — the same contract `ProjectGroups` has. */
  onCreate: (subjectId: number | null, name: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [subjectId, setSubjectId] = useState<number | null>(null);

  // Current subjects first: a project is nearly always for something you are
  // enrolled in now, and past terms are kept only so an old one stays
  // reachable.
  const groups = useMemo(() => {
    const ordered = [
      ...subjects.filter((s) => s.is_current),
      ...subjects.filter((s) => !s.is_current),
    ];
    return [
      { id: null as number | null, label: "Personal", title: "Not scoped to a subject", subject: null as Subject | null },
      ...ordered.map((s) => ({
        id: s.id as number | null,
        label: displayCode(s.code),
        title: displayName(s.name, s.code),
        subject: s,
      })),
    ];
  }, [subjects]);

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
        // A half-typed name is not worth keeping; the group is, so a second
        // project for the same subject is one field away.
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
          {groups.map((g) => {
            const selected = g.id === subjectId;
            return (
              <button
                key={g.id ?? "personal"}
                type="button"
                title={g.title}
                onClick={() => setSubjectId(g.id)}
                className={cn(
                  "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[12.5px] transition-colors",
                  selected
                    ? "bg-accent text-foreground"
                    : "text-muted-foreground hover:bg-accent hover:text-foreground",
                )}
              >
                {g.subject ? (
                  <SubjectIcon code={g.subject.code} size={13} />
                ) : (
                  <Kanban size={13} className="shrink-0" />
                )}
                <span className="min-w-0 flex-1 truncate">{g.label}</span>
                {selected && <Check size={12} weight="bold" className="shrink-0 text-brand" />}
              </button>
            );
          })}
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
