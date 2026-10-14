import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Check, MagnifyingGlass, Plus } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import type { DbTaskTag } from "@/lib/planning/projects";
import { TagDot } from "./TagDot";

type Row = { kind: "tag"; tag: DbTaskTag } | { kind: "create"; name: string };

/**
 * A multi-select over the tag registry in a popover, `children` being the
 * trigger. Picking toggles and never closes; typing filters, and a query that
 * names no tag offers `Create "<query>"`, which makes the tag then selects it.
 * ↑↓ move, ↵ picks, Esc closes.
 */
export function TagPicker({
  tags,
  value,
  onChange,
  onCreate,
  align = "start",
  children,
}: {
  tags: DbTaskTag[];
  /** The selected tag ids. */
  value: number[];
  onChange: (ids: number[]) => void;
  /** Makes the tag (or returns the one already named so); the picker selects it. */
  onCreate: (name: string) => Promise<DbTaskTag>;
  align?: "start" | "center" | "end";
  children: ReactNode;
}) {
  const listId = useId();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);

  const selected = useMemo(() => new Set(value), [value]);

  const rows = useMemo<Row[]>(() => {
    const name = query.trim().replace(/\s+/g, " ");
    const key = name.toLowerCase();
    const matches = tags.filter((t) => t.name.toLowerCase().includes(key));
    const out: Row[] = matches.map((tag) => ({ kind: "tag", tag }));
    if (name && !tags.some((t) => t.name.toLowerCase() === key)) {
      out.push({ kind: "create", name });
    }
    return out;
  }, [tags, query]);

  const current = Math.min(active, Math.max(rows.length - 1, 0));

  useEffect(() => {
    list.current
      ?.querySelector<HTMLElement>(`[data-index="${current}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [current, rows.length]);

  const onOpenChange = (next: boolean) => {
    setOpen(next);
    if (!next) {
      setQuery("");
      setActive(0);
      setError(null);
    }
  };

  const toggle = (id: number) => {
    onChange(selected.has(id) ? value.filter((v) => v !== id) : [...value, id]);
  };

  const create = async (name: string) => {
    if (creating) return;
    setCreating(true);
    setError(null);
    try {
      const tag = await onCreate(name);
      if (!selected.has(tag.id)) onChange([...value, tag.id]);
      setQuery("");
      setActive(0);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setCreating(false);
    }
  };

  const pick = (row: Row | undefined) => {
    if (!row) return;
    if (row.kind === "tag") toggle(row.tag.id);
    else void create(row.name);
    input.current?.focus();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!rows.length) return;
      const step = e.key === "ArrowDown" ? 1 : -1;
      setActive((current + step + rows.length) % rows.length);
    } else if (e.key === "Enter" && !e.nativeEvent.isComposing) {
      e.preventDefault();
      pick(rows[current]);
    }
  };

  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger asChild>{children}</PopoverTrigger>
      <PopoverContent align={align} className="w-64 p-2">
        <div className="mb-1 flex h-7 items-center gap-2 rounded-md border border-border-subtle px-2 focus-within:border-brand/50">
          <MagnifyingGlass size={12} className="shrink-0 text-muted-foreground" />
          <input
            ref={input}
            autoFocus
            role="combobox"
            aria-expanded
            aria-controls={listId}
            aria-activedescendant={rows.length ? `${listId}-row-${current}` : undefined}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setActive(0);
              setError(null);
            }}
            onKeyDown={onKeyDown}
            placeholder="Search or create a tag"
            spellCheck={false}
            className="min-w-0 flex-1 bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
          />
        </div>
        <div
          ref={list}
          id={listId}
          role="listbox"
          aria-multiselectable
          className="-mx-1 max-h-52 overflow-y-auto px-1"
        >
          {rows.map((row, i) => {
            const isActive = i === current;
            const common = {
              id: `${listId}-row-${i}`,
              "data-index": i,
              type: "button" as const,
              role: "option" as const,
              tabIndex: -1,
              onMouseMove: () => setActive(i),
              onClick: () => pick(row),
            };
            const rowClass = cn(
              "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[12.5px] transition-colors",
              isActive ? "bg-accent text-foreground" : "text-muted-foreground",
            );
            if (row.kind === "create") {
              return (
                <button
                  key="create"
                  {...common}
                  aria-selected={false}
                  disabled={creating}
                  className={cn(rowClass, "disabled:opacity-60")}
                >
                  <Plus size={12} weight="bold" className="shrink-0" />
                  <span className="min-w-0 flex-1 truncate">Create &ldquo;{row.name}&rdquo;</span>
                </button>
              );
            }
            const checked = selected.has(row.tag.id);
            return (
              <button
                key={row.tag.id}
                {...common}
                aria-selected={checked}
                className={rowClass}
              >
                <span className="flex size-3 shrink-0 items-center justify-center">
                  {checked && <Check size={12} weight="bold" className="text-brand" />}
                </span>
                <TagDot color={row.tag.color} />
                <span className="min-w-0 flex-1 truncate">{row.tag.name}</span>
              </button>
            );
          })}
          {rows.length === 0 && (
            <p className="px-2 py-3 text-center text-xs text-muted-foreground">
              No tags yet. Type a name to create one.
            </p>
          )}
        </div>
        {error && <p className="px-2 pt-1.5 text-[11px] text-destructive">{error}</p>}
      </PopoverContent>
    </Popover>
  );
}
