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
  const [pastOpen, setPastOpen] = useState(false);

  // A project is nearly always for something you are enrolled in now, so the
  // list is Personal and this term's subjects — and past terms, kept only so
  // an old one stays reachable, fold away behind their own heading rather than
  // padding the list you actually pick from. The chat scope picker goes
  // further and drops them entirely; here they stay reachable, because a
  // project *can* outlive the term it was set in.
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

  // A selection inside the fold can never be hidden by it — otherwise creating
  // a project for a past subject and opening the picker again shows Personal
  // unticked and no tick anywhere.
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
          {listed.map((g) => (
            <GroupRow
              key={g.id ?? "personal"}
              group={g}
              selected={g.id === subjectId}
              onPick={() => setSubjectId(g.id)}
            />
          ))}

          {/* The Sync page's picker says "Past subjects (n)" behind the same
              caret, so this reads as the same idea rather than a second one.
              It does not repeat that picker's per-term headings: there you are
              auditing several terms at once, here you are choosing one
              destination out of a short list in a narrow popover. */}
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

/** One destination: the Personal board, or a subject. Extracted only so the
 *  open list and the folded one cannot drift apart. */
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
