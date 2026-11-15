import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { Books, CaretDown, Check, MagnifyingGlass } from "@phosphor-icons/react";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { FilterFold } from "@/components/ui/filter/FilterFold";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { subjectOption, subjectOptions, type SubjectOption } from "@/components/harness/composer/subjectOptions";
import type { Subject } from "@/lib/db";
import type { ChatScope } from "@/lib/harness/scope";
import { cn } from "@/lib/utils";

/**
 * A thread's scope: one subject, or General (the whole library). Not a sandbox
 * — it narrows the `@` menu and names the course folder in the agent's
 * instructions (`docs/harness.md`). The small pill above a composer (Studio's
 * New video) over a searchable popover: General, this term's subjects, then
 * past ones under a fold; arrows move, Enter picks, Escape closes back to the
 * trigger. A new Chat thread chooses in `NewThreadDialog` instead.
 */
export function SubjectPicker({
  subjects,
  value,
  onChange,
  className,
}: {
  subjects: readonly Subject[];
  value: ChatScope;
  onChange: (scope: ChatScope) => void;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [pastOpen, setPastOpen] = useState(false);
  const [active, setActive] = useState(0);
  const searchRef = useRef<HTMLInputElement>(null);
  const rows = useRef(new Map<number, HTMLButtonElement>());
  const listId = useId();

  const { listed, past } = useMemo(() => subjectOptions(subjects, query), [subjects, query]);
  const chosen = useMemo(() => subjectOption(subjects, value), [subjects, value]);
  // The fold never hides the choice, and a search shows every match.
  const pastChosen = past.some((o) => o.scope === value);
  const foldOpen = pastOpen || pastChosen || query.trim() !== "";
  const visible = useMemo(() => (foldOpen ? [...listed, ...past] : listed), [foldOpen, listed, past]);
  // Closing the fold can leave the highlight past the last visible row.
  const current = Math.min(active, Math.max(0, visible.length - 1));

  // A stale search would hide rows the next time it opens.
  useEffect(() => {
    if (!open) setQuery("");
  }, [open]);
  // Opening lands on the choice; a new search on its first match.
  useEffect(() => {
    if (!open) return;
    const at = query.trim() ? 0 : visible.findIndex((o) => o.scope === value);
    setActive(Math.max(0, at));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, query]);
  useEffect(() => {
    rows.current.get(current)?.scrollIntoView({ block: "nearest" });
  }, [current]);

  const pick = (o: SubjectOption) => {
    onChange(o.scope);
    setOpen(false);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!visible.length) return;
      const step = e.key === "ArrowDown" ? 1 : -1;
      setActive((current + step + visible.length) % visible.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const o = visible[current];
      if (o) pick(o);
    }
  };

  const row = (o: SubjectOption, i: number) => (
    <OptionRow
      key={o.scope}
      id={`${listId}-${i}`}
      option={o}
      selected={o.scope === value}
      active={i === current}
      onHover={() => setActive(i)}
      onPick={() => pick(o)}
      rowRef={(el) => {
        if (el) rows.current.set(i, el);
        else rows.current.delete(i);
      }}
    />
  );

  const title = chosen ? chosen.title : "General — the whole library";

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          title={title}
          aria-label={`Subject: ${title}`}
          className={cn(
            "inline-flex h-6 max-w-[200px] min-w-0 cursor-pointer items-center gap-1.5 rounded-full border border-border/70 bg-card px-2 text-[11px] text-muted-foreground transition-[color,background-color,border-color] outline-none hover:bg-accent hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/25 data-[state=open]:bg-accent",
            className,
          )}
        >
          {chosen && <SubjectOptionGlyph option={chosen} size={12} />}
          <span className="min-w-0 truncate">{chosen ? chosen.label : "General"}</span>
          <CaretDown size={12} className="shrink-0 text-muted-foreground" />
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className="flex max-h-[min(var(--radix-popover-content-available-height),24rem)] w-64 flex-col overflow-hidden p-0"
        onOpenAutoFocus={(e) => {
          e.preventDefault();
          searchRef.current?.focus();
        }}
      >
        <div className="shrink-0 p-2">
          <div className="flex h-8 items-center gap-2 rounded-full border border-input bg-card px-3 transition-[color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25">
            <MagnifyingGlass size={13} className="shrink-0 text-muted-foreground" />
            <input
              ref={searchRef}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={onKeyDown}
              placeholder="Search subjects"
              role="combobox"
              aria-label="Search subjects"
              aria-expanded
              aria-controls={listId}
              aria-activedescendant={visible[current] ? `${listId}-${current}` : undefined}
              spellCheck={false}
              autoComplete="off"
              className="min-w-0 flex-1 bg-transparent text-[13px] text-foreground outline-none placeholder:text-muted-foreground"
            />
          </div>
        </div>
        <div
          id={listId}
          role="listbox"
          aria-label="Subjects"
          className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-1 pb-1"
        >
          {listed.length + past.length === 0 ? (
            <div className="px-2 py-1.5 text-xs text-muted-foreground">No matching subjects.</div>
          ) : (
            <>
              {listed.map((o, i) => row(o, i))}
              {past.length > 0 && (
                <FilterFold label="Past subjects" count={past.length} open={foldOpen} onOpenChange={setPastOpen}>
                  {past.map((o, i) => row(o, listed.length + i))}
                </FilterFold>
              )}
            </>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}

/** A choice's glyph: the subject's own icon, or the library's for General. */
export function SubjectOptionGlyph({ option, size }: { option: SubjectOption; size: number }) {
  return option.subject ? (
    <SubjectIcon code={option.subject.code} size={size} className="shrink-0" />
  ) : (
    <Books size={size} className="shrink-0 text-muted-foreground" />
  );
}

function OptionRow({
  id,
  option,
  selected,
  active,
  onHover,
  onPick,
  rowRef,
}: {
  id: string;
  option: SubjectOption;
  selected: boolean;
  active: boolean;
  onHover: () => void;
  onPick: () => void;
  rowRef: (el: HTMLButtonElement | null) => void;
}) {
  return (
    <button
      ref={rowRef}
      id={id}
      type="button"
      role="option"
      aria-selected={selected}
      aria-label={option.title}
      title={option.title}
      tabIndex={-1}
      onMouseMove={onHover}
      onClick={onPick}
      className={cn(
        "flex w-full cursor-default select-none items-center gap-2 rounded-md px-2 py-1.5 text-left text-[12.5px] text-foreground outline-none transition-colors",
        active && "bg-accent",
      )}
    >
      <SubjectOptionGlyph option={option} size={13} />
      <span className="min-w-0 flex-1 truncate">{option.label}</span>
      {selected && <Check size={12} weight="bold" className="shrink-0 text-brand" />}
    </button>
  );
}
