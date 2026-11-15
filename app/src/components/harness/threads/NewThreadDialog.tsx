import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { Check, MagnifyingGlass } from "@phosphor-icons/react";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { KeyHint } from "@/components/search/KeyHint";
import { SubjectOptionGlyph } from "@/components/harness/composer/SubjectPicker";
import { subjectOptions, type SubjectOption } from "@/components/harness/composer/subjectOptions";
import type { Subject } from "@/lib/db";
import type { ChatScope } from "@/lib/harness/scope";
import { cn } from "@/lib/utils";

/**
 * Where a new Chat thread works, asked before it starts: General (the whole
 * library) or one subject (`docs/harness.md`). The ⌘K palette's frame over
 * `SubjectChoices`. Escape returns focus to what opened it; a pick hands it
 * to `focusAfterPick`, since the dialog has no trigger of its own for Radix
 * to return to.
 */
export function NewThreadDialog({
  open,
  onOpenChange,
  subjects,
  value,
  onPick,
  focusAfterPick,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  subjects: readonly Subject[];
  /** The choice so far; null is unchosen. */
  value: ChatScope | null;
  onPick: (scope: ChatScope) => void;
  focusAfterPick: () => void;
}) {
  const opener = useRef<HTMLElement | null>(null);
  const picked = useRef(false);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        showCloseButton={false}
        // Hangs from the top so the field doesn't move as the list narrows.
        className="top-[14%] translate-y-0 gap-0 overflow-hidden rounded-xl border-border bg-popover p-0 shadow-lg"
        // Fires before focus moves in, so the active element is still the opener.
        onOpenAutoFocus={() => {
          opener.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
          picked.current = false;
        }}
        onCloseAutoFocus={(e) => {
          e.preventDefault();
          if (picked.current) focusAfterPick();
          else opener.current?.focus();
        }}
      >
        <DialogTitle className="sr-only">New thread</DialogTitle>
        <DialogDescription className="sr-only">
          Choose the subject this thread works in, or General for the whole library.
        </DialogDescription>
        {/* Mounted per open: an empty field, the highlight on the choice. */}
        <SubjectChoices
          subjects={subjects}
          value={value}
          onPick={(scope) => {
            picked.current = true;
            onPick(scope);
          }}
        />
      </DialogContent>
    </Dialog>
  );
}

const ROW =
  "mx-1.5 flex w-[calc(100%-0.75rem)] items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left text-[12.5px]";
const HEADING = "px-4 pb-0.5 pt-1.5 text-[11px] font-medium tracking-wide text-muted-foreground";

/**
 * The dialog's body: a search field over General and this term's subjects,
 * then past terms' under their own heading, each row a glyph, the short code
 * and the full name. Arrows move, Enter picks. It opens on the choice so far,
 * and a new search lands on its first match.
 */
export function SubjectChoices({
  subjects,
  value,
  onPick,
}: {
  subjects: readonly Subject[];
  value: ChatScope | null;
  onPick: (scope: ChatScope) => void;
}) {
  const [query, setQuery] = useState("");
  const { listed, past } = useMemo(() => subjectOptions(subjects, query), [subjects, query]);
  const rows = useMemo(() => [...listed, ...past], [listed, past]);
  const [active, setActive] = useState(() => Math.max(0, rows.findIndex((o) => o.scope === value)));
  const current = Math.min(active, Math.max(0, rows.length - 1));
  const listRef = useRef<HTMLDivElement>(null);
  const listId = useId();

  useEffect(() => {
    listRef.current?.querySelector(`[data-index="${current}"]`)?.scrollIntoView({ block: "nearest" });
  }, [current]);

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!rows.length) return;
      const step = e.key === "ArrowDown" ? 1 : -1;
      setActive((current + step + rows.length) % rows.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const o = rows[current];
      if (o) onPick(o.scope);
    }
  };

  const row = (o: SubjectOption, i: number) => (
    <button
      key={o.scope}
      id={`${listId}-${i}`}
      type="button"
      role="option"
      aria-selected={o.scope === value}
      data-index={i}
      tabIndex={-1}
      // `mousemove`, not `mouseenter`: arrowing scrolls rows under a resting
      // pointer, which would steal the highlight back.
      onMouseMove={() => setActive(i)}
      onClick={() => onPick(o.scope)}
      className={cn(ROW, i === current ? "bg-accent text-foreground" : "text-foreground/90")}
    >
      <SubjectOptionGlyph option={o} size={15} />
      <span className="min-w-0 flex-1">
        <span className="block truncate">{o.label}</span>
        <span className="mt-0.5 block truncate text-[11px] text-muted-foreground">
          {o.subject ? o.title : "The whole library"}
        </span>
      </span>
      {o.scope === value && <Check size={13} weight="bold" className="shrink-0 text-brand" />}
    </button>
  );

  return (
    <div>
      <div className="flex items-center gap-2.5 border-b border-border-subtle px-4">
        <MagnifyingGlass size={15} className="shrink-0 text-muted-foreground" />
        <input
          autoFocus
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
          onKeyDown={onKeyDown}
          placeholder="Search subjects"
          role="combobox"
          aria-label="Search subjects"
          aria-expanded
          aria-controls={listId}
          aria-activedescendant={rows[current] ? `${listId}-${current}` : undefined}
          spellCheck={false}
          autoComplete="off"
          className="h-11 min-w-0 flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground"
        />
      </div>

      <div
        ref={listRef}
        id={listId}
        role="listbox"
        aria-label="Subjects"
        className="max-h-[min(58vh,380px)] overflow-y-auto py-1.5"
      >
        {rows.length === 0 ? (
          <p className="px-4 py-6 text-center text-[12.5px] text-muted-foreground">No matching subjects.</p>
        ) : (
          <>
            {listed.length > 0 && (
              <div role="group" aria-label="New thread in" className="pb-1 last:pb-0">
                <div className={HEADING}>New thread in</div>
                {listed.map((o, i) => row(o, i))}
              </div>
            )}
            {past.length > 0 && (
              <div role="group" aria-label="Past subjects" className="pb-1 last:pb-0">
                <div className={HEADING}>Past subjects</div>
                {past.map((o, i) => row(o, listed.length + i))}
              </div>
            )}
          </>
        )}
      </div>

      <div className="flex items-center gap-3 border-t border-border-subtle px-4 py-2 text-[11px] text-muted-foreground">
        <KeyHint keys="↑↓" label="Navigate" />
        <KeyHint keys="↵" label="Choose" />
        <KeyHint keys="esc" label="Close" />
      </div>
    </div>
  );
}
