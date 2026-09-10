import { useState } from "react";
import {
  ArrowsClockwise,
  BookOpen,
  CaretRight,
  CaretUpDown,
  CircleNotch,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
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
import { SubjectRow } from "@/components/subjects/SubjectRow";
import type { Subject } from "@/lib/db";

interface SubjectPickerProps {
  subjects: Subject[];
  selectedIds: Set<number>;
  onToggle: (id: number) => void;
  onRefetch: () => void;
  refetching: boolean;
  canRefetch: boolean;
}

/**
 * The Sync page's subject selector: a compact trigger in the header row that
 * opens the current / past subject checklist in a popover. What's checked
 * here is what "Sync now" pulls.
 */
export function SubjectPicker({
  subjects,
  selectedIds,
  onToggle,
  onRefetch,
  refetching,
  canRefetch,
}: SubjectPickerProps) {
  const [pastExpanded, setPastExpanded] = useState(false);

  const current = subjects.filter((s) => s.is_current);
  const past = subjects.filter((s) => !s.is_current);

  const pastBySemester = past.reduce<Record<string, Subject[]>>((acc, s) => {
    const key = s.term_name ?? "Unknown term";
    (acc[key] ??= []).push(s);
    return acc;
  }, {});

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          className="h-7 gap-1.5 px-3 text-xs font-normal text-foreground"
        >
          <BookOpen size={13} className="text-muted-foreground" />
          {selectedIds.size > 0
            ? `${selectedIds.size} subject${selectedIds.size === 1 ? "" : "s"}`
            : "No subjects"}
          <CaretUpDown size={11} className="text-muted-foreground/60" />
        </Button>
      </PopoverTrigger>

      <PopoverContent side="bottom" align="start" className="w-80 p-0">
        <div className="flex items-center justify-between border-b border-border-subtle px-3 py-2">
          <p className="font-display text-[13px] font-semibold text-foreground">
            Subjects to sync
          </p>
          <Button
            variant="ghost"
            size="sm"
            onClick={onRefetch}
            disabled={refetching || !canRefetch}
            className="h-6 -mr-1.5 text-[11px] text-muted-foreground hover:text-foreground"
          >
            {refetching ? (
              <>
                <CircleNotch size={11} className="animate-spin" /> Fetching…
              </>
            ) : (
              <>
                <ArrowsClockwise size={11} /> Refetch
              </>
            )}
          </Button>
        </div>

        <div className="max-h-96 overflow-y-auto p-2">
          {subjects.length === 0 ? (
            <div className="py-6 text-center">
              <BookOpen size={24} className="text-muted-foreground/40 mx-auto mb-2" />
              <p className="text-xs text-foreground font-medium mb-0.5">
                No subjects loaded
              </p>
              <p className="text-[11px] text-muted-foreground">
                Fetch your Canvas subjects to get started.
              </p>
            </div>
          ) : (
            <div className="space-y-3">
              {current.length > 0 && (
                <div>
                  <p className="text-[11px] text-muted-foreground mb-1 px-1">
                    {current[0]?.term_name}
                  </p>
                  <div className="space-y-1">
                    {current.map((s) => (
                      <SubjectRow
                        key={s.id}
                        subject={s}
                        checked={selectedIds.has(s.id)}
                        onToggle={() => onToggle(s.id)}
                      />
                    ))}
                  </div>
                </div>
              )}

              {past.length > 0 && (
                <Collapsible open={pastExpanded} onOpenChange={setPastExpanded}>
                  <CollapsibleTrigger className="flex items-center gap-1.5 pl-1 py-1 text-[11px] font-medium text-muted-foreground hover:text-foreground transition-colors cursor-pointer">
                    <CaretRight
                      size={9}
                      className={cn(
                        "shrink-0 transition-transform",
                        pastExpanded && "rotate-90",
                      )}
                    />
                    Past subjects ({past.length})
                  </CollapsibleTrigger>
                  <CollapsibleContent className="space-y-2 pt-1">
                    {Object.entries(pastBySemester)
                      .sort(([a], [b]) => b.localeCompare(a))
                      .map(([term, courses]) => (
                        <div key={term}>
                          <p className="text-[11px] text-muted-foreground mb-1 px-1">
                            {term}
                          </p>
                          <div className="space-y-1">
                            {courses.map((s) => (
                              <SubjectRow
                                key={s.id}
                                subject={s}
                                checked={selectedIds.has(s.id)}
                                onToggle={() => onToggle(s.id)}
                                dimmed
                              />
                            ))}
                          </div>
                        </div>
                      ))}
                  </CollapsibleContent>
                </Collapsible>
              )}
            </div>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}
