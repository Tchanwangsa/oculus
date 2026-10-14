import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Check, MagnifyingGlass } from "@phosphor-icons/react";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { displayCode } from "@/lib/format/format";
import { fileTitle } from "@/lib/files/openFile";
import { fileReferenceAppearance } from "@/lib/files/referenceAppearance";
import { searchAttachableFiles, type DbTaskFile } from "@/lib/planning/projects";
import { cn } from "@/lib/utils";
import { fileFolder } from "./taskFormat";

const DEBOUNCE_MS = 150;

/**
 * A search over the whole library in a popover, `children` being the trigger.
 * An empty query lists recently opened files; typing ranks as ⌘K does. ↑↓
 * move, ↵ picks, Esc closes. A file already on the task shows checked and
 * cannot be picked again. Picking attaches and closes.
 */
export function AttachFilePicker({
  attachedIds,
  onAttach,
  children,
}: {
  attachedIds: Set<number>;
  /** Links the file; a rejection is shown in the popover. */
  onAttach: (file: DbTaskFile) => Promise<void>;
  children: ReactNode;
}) {
  const listId = useId();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<DbTaskFile[] | null>(null);
  /** The query `results` answer; Enter waits for it to match what is typed. */
  const [settled, setSettled] = useState<string | null>(null);
  const [active, setActive] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const list = useRef<HTMLDivElement>(null);
  /** Only the newest search sets results when replies land out of order. */
  const run = useRef(0);

  useEffect(() => {
    if (!open) return;
    const token = ++run.current;
    const timer = window.setTimeout(
      () => {
        searchAttachableFiles(query)
          .then((rows) => {
            if (run.current !== token) return;
            setResults(rows);
            setSettled(query);
            setActive(0);
            setError(null);
          })
          .catch((e) => {
            if (run.current === token) setError(e instanceof Error ? e.message : String(e));
          });
      },
      // The recents list opens at once; only typing waits.
      query.trim() ? DEBOUNCE_MS : 0,
    );
    return () => window.clearTimeout(timer);
  }, [open, query]);

  const current = Math.min(active, Math.max((results?.length ?? 0) - 1, 0));

  useEffect(() => {
    list.current
      ?.querySelector<HTMLElement>(`[data-index="${current}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [current, results]);

  const onOpenChange = (next: boolean) => {
    setOpen(next);
    if (!next) {
      run.current++;
      setQuery("");
      setResults(null);
      setSettled(null);
      setActive(0);
      setError(null);
    }
  };

  const pick = async (file: DbTaskFile | undefined) => {
    if (!file || busy || attachedIds.has(file.file_id)) return;
    setBusy(true);
    try {
      await onAttach(file);
      onOpenChange(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    const count = results?.length ?? 0;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!count) return;
      const step = e.key === "ArrowDown" ? 1 : -1;
      setActive((current + step + count) % count);
    } else if (e.key === "Enter" && !e.nativeEvent.isComposing) {
      e.preventDefault();
      if (settled === query) void pick(results?.[current]);
    }
  };

  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger asChild>{children}</PopoverTrigger>
      <PopoverContent align="start" className="w-80 p-2">
        <div className="mb-1 flex h-7 items-center gap-2 rounded-md border border-border-subtle px-2 focus-within:border-brand/50">
          <MagnifyingGlass size={12} className="shrink-0 text-muted-foreground" />
          <input
            autoFocus
            role="combobox"
            aria-expanded
            aria-controls={listId}
            aria-activedescendant={results?.length ? `${listId}-row-${current}` : undefined}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder="Search the library"
            spellCheck={false}
            className="min-w-0 flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
          />
        </div>
        {!query.trim() && results && results.length > 0 && (
          <p className="px-2 pt-1 pb-0.5 text-[11px] text-muted-foreground">Recently opened</p>
        )}
        <div
          ref={list}
          id={listId}
          role="listbox"
          className="-mx-1 max-h-64 overflow-y-auto px-1"
        >
          {results?.map((file, i) => {
            const { Icon, tone } = fileReferenceAppearance(file);
            const attached = attachedIds.has(file.file_id);
            const where = [
              file.subject_code ? displayCode(file.subject_code) : null,
              fileFolder(file.relative_path) || null,
            ]
              .filter(Boolean)
              .join(" · ");
            return (
              <button
                key={file.file_id}
                id={`${listId}-row-${i}`}
                data-index={i}
                type="button"
                role="option"
                tabIndex={-1}
                aria-selected={attached}
                aria-disabled={attached}
                onMouseMove={() => setActive(i)}
                onClick={() => void pick(file)}
                className={cn(
                  "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[12.5px] transition-colors",
                  i === current && "bg-accent",
                  attached ? "cursor-default text-muted-foreground" : "cursor-pointer text-foreground",
                )}
              >
                <Icon size={14} className={cn("shrink-0", attached ? "text-muted-foreground" : tone)} />
                <span className="min-w-0 flex-1">
                  <span className="block truncate">{fileTitle(file)}</span>
                  {where && (
                    <span className="block truncate text-[11px] text-muted-foreground">{where}</span>
                  )}
                </span>
                {attached && <Check size={12} weight="bold" className="shrink-0 text-brand" />}
              </button>
            );
          })}
          {results?.length === 0 && (
            <p className="px-2 py-3 text-center text-xs text-muted-foreground">
              {query.trim() ? "No files match." : "Nothing opened recently. Type to search."}
            </p>
          )}
        </div>
        {error && <p className="px-2 pt-1.5 text-[11px] text-destructive">{error}</p>}
      </PopoverContent>
    </Popover>
  );
}
