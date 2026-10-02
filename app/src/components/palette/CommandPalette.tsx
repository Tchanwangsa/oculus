import { useRef, useState } from "react";
import { MagnifyingGlass, X } from "@phosphor-icons/react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  SearchGlyph,
  SearchList,
  useSearchSelection,
} from "@/components/search/SearchList";
import { navigateActive, openUrlInFocusedPane } from "@/lib/tabRouters";
import {
  filterChip,
  filterId,
  filterPlaceholder,
  filterToken,
  openSearchItem,
  resolveFilter,
  withFilter,
  type FilterDraft,
  type SearchFilter,
  type SearchItem,
} from "@/lib/search";
import { useSearch } from "@/hooks/useSearch";
import { useTauriEvent } from "@/hooks/useEvents";
import { usePaletteStore } from "@/stores/paletteStore";
import { useTabStore } from "@/stores/tabStore";
import { useSubjects } from "@/hooks/useSubjects";
import { linkInFocusedNote } from "@/components/documents/editor/commands";

/**
 * The ⌘K palette over everything reachable (library, pages, web). What it
 * searches is `app/src/lib/search.ts`, shared with the new-tab page. Enter
 * opens in the current (focused) pane, ⌘↵ in a new tab. `in:` and `type:`
 * tokens become chips in the field that narrow the search.
 */
export default function CommandPalette() {
  const open = usePaletteStore((s) => s.open);
  const setOpen = usePaletteStore((s) => s.setOpen);
  const toggle = usePaletteStore((s) => s.toggle);

  // A menu item, not a key handler: macOS gives the menu bar ⌘-keys first, which
  // also makes it work over a native browser tab. See `app/src-tauri/src/menu.rs`.
  // In a focused note, ⌘K makes a link instead.
  useTauriEvent("menu-search", () => {
    if (!linkInFocusedNote()) toggle();
  });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent
        showCloseButton={false}
        // Hangs from the top so the field doesn't move as the list grows.
        className="top-[14%] translate-y-0 gap-0 overflow-hidden rounded-xl border-border bg-popover p-0 shadow-lg sm:max-w-xl"
      >
        <DialogTitle className="sr-only">Search</DialogTitle>
        <DialogDescription className="sr-only">
          Find a subject, file, lecture, project or page.
        </DialogDescription>
        {/* Mounted per open: empty field, fresh library read. */}
        <PaletteBody onClose={() => setOpen(false)} />
      </DialogContent>
    </Dialog>
  );
}

function PaletteBody({ onClose }: { onClose: () => void }) {
  const addTab = useTabStore((s) => s.addTab);
  const { subjects, current } = useSubjects();

  const inputRef = useRef<HTMLInputElement>(null);
  const draftRef = useRef<HTMLInputElement>(null);

  const [query, setQuery] = useState("");
  // State, so these are stable for `useSearch`'s deps.
  const [filters, setFilters] = useState<SearchFilter[]>([]);
  // The chip being typed: `in:` / `type:` turns into one the moment its colon
  // lands, and the list is its values until it is picked.
  const [draft, setDraft] = useState<FilterDraft | null>(null);
  const sections = useSearch(query, { subjects, current, filters, offerFilters: true, draft });

  function addFilter(filter: SearchFilter) {
    setFilters((fs) => withFilter(fs, filter));
    setDraft(null);
    inputRef.current?.focus();
  }

  function removeFilter(filter: SearchFilter) {
    setFilters((fs) => fs.filter((f) => f !== filter));
    inputRef.current?.focus();
  }

  function onChange(text: string) {
    const token = filterToken(text);
    if (token) {
      setQuery(text.slice(0, token.start));
      setDraft({ key: token.key, value: token.value });
    } else setQuery(text);
  }

  function onDraftChange(value: string) {
    // A space after a value that names one thing commits it; otherwise the
    // space stays, since a subject's name has several words.
    if (draft && /\s$/.test(value) && value.length > draft.value.length) {
      const filter = resolveFilter({ key: draft.key, value: value.trim() }, subjects);
      if (filter) return addFilter(filter);
    }
    setDraft((d) => d && { ...d, value });
  }

  function pick(item: SearchItem, newTab: boolean) {
    const { target } = item;
    // Filter rows edit the field and keep the palette open; ⌘ changes nothing.
    if (target.kind === "filter-key") {
      setDraft({ key: target.key, value: "" });
      return;
    }
    if (target.kind === "filter") {
      addFilter(target.filter);
      return;
    }
    // Under a draft only its values are live; anything else is a list the
    // debounce has not replaced yet.
    if (draft) return;
    onClose();
    openSearchItem(item, {
      newTab,
      // The shell's navigation, with its departure rules.
      navigate: navigateActive,
      addTab,
      openUrl: (url) => void openUrlInFocusedPane(url),
    });
  }

  const { selected, setIndex, onKeyDown, item } = useSearchSelection(
    sections,
    `${filters.map(filterId).join(" ")}|${draft ? `${draft.key}:${draft.value}` : ""}|${query}`,
    pick,
  );

  const atStart = (e: React.KeyboardEvent<HTMLInputElement>) =>
    e.currentTarget.selectionStart === 0 && e.currentTarget.selectionEnd === 0;

  function onInputKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key !== "Backspace" || !atStart(e)) return;
    // Back into the chip being typed, else take the last chip off.
    if (draft) {
      e.preventDefault();
      draftRef.current?.focus();
    } else if (filters.length > 0) {
      e.preventDefault();
      setFilters((fs) => fs.slice(0, -1));
    }
  }

  function onDraftKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Backspace" && atStart(e)) {
      // Backspacing into `in:` drops it, as Discord does.
      e.preventDefault();
      setDraft(null);
      inputRef.current?.focus();
    } else if (e.key === "Tab") {
      e.preventDefault();
      if (item?.target.kind === "filter") pick(item, false);
    }
  }

  return (
    <div onKeyDown={onKeyDown}>
      <div className="flex items-center gap-2.5 border-b border-border-subtle px-4">
        <MagnifyingGlass size={15} className="shrink-0 text-muted-foreground" />
        {(filters.length > 0 || draft) && (
          <span className="flex shrink-0 items-center gap-1.5">
            {filters.map((f) => (
              <FilterChip key={filterId(f)} filter={f} onRemove={() => removeFilter(f)} />
            ))}
            {draft && (
              <DraftChip
                key={draft.key}
                draft={draft}
                inputRef={draftRef}
                onChange={onDraftChange}
                onKeyDown={onDraftKeyDown}
              />
            )}
          </span>
        )}
        <input
          ref={inputRef}
          autoFocus
          value={query}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={onInputKeyDown}
          placeholder={
            draft
              ? ""
              : filters.length > 0
                ? filterPlaceholder(filters)
                : "Search subjects, files, lectures…"
          }
          className="h-11 min-w-0 flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground"
        />
      </div>

      <SearchList
        sections={sections}
        selected={selected}
        onHover={setIndex}
        onPick={pick}
        className="max-h-[min(58vh,380px)]"
      />

      <div className="flex items-center gap-3 border-t border-border-subtle px-4 py-2 text-[11px] text-muted-foreground">
        {draft ? (
          <>
            <Hint keys="↵" label="Add filter" />
            <Hint keys="tab" label="Complete" />
          </>
        ) : (
          <>
            <Hint keys="↵" label="Open" />
            <Hint keys="⌘ ↵" label="Open in new tab" />
          </>
        )}
        <Hint keys="esc" label="Close" />
      </div>
    </div>
  );
}

/** `in: COMP30022`; Backspace at the field's start removes the last one, so the
 *  × stays out of the tab order. */
function FilterChip({ filter, onRemove }: { filter: SearchFilter; onRemove: () => void }) {
  const { value, icon } = filterChip(filter);
  return (
    <span className="flex h-6 items-center gap-1 rounded-full border border-border bg-surface pl-2 pr-0.5 text-[12px]">
      <SearchGlyph spec={icon} size={12} />
      <span className="text-muted-foreground">{filter.key}:</span>
      <span className="text-foreground">{value}</span>
      <button
        type="button"
        tabIndex={-1}
        aria-label={`Remove ${filter.key}: ${value}`}
        onClick={onRemove}
        className="flex size-5 items-center justify-center rounded-full text-muted-foreground hover:bg-accent hover:text-foreground"
      >
        <X size={10} />
      </button>
    </span>
  );
}

/** The chip being typed: the caret moves into it, so `in:` reads as a chip
 *  from its colon on. An invisible twin of the text sizes the input. */
function DraftChip({
  draft,
  inputRef,
  onChange,
  onKeyDown,
}: {
  draft: FilterDraft;
  inputRef: React.RefObject<HTMLInputElement | null>;
  onChange: (value: string) => void;
  onKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void;
}) {
  const hint = draft.key === "in" ? "subject" : "type";
  return (
    <span className="flex h-6 items-center gap-1 rounded-full border border-brand/40 bg-surface px-2 text-[12px]">
      <span className="text-muted-foreground">{draft.key}:</span>
      <span className="inline-grid">
        <span aria-hidden className="invisible col-start-1 row-start-1 whitespace-pre">
          {draft.value || hint}
        </span>
        <input
          ref={inputRef}
          autoFocus
          size={1}
          value={draft.value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={onKeyDown}
          placeholder={hint}
          aria-label={`${draft.key}: filter`}
          className="col-start-1 row-start-1 w-full min-w-0 bg-transparent text-foreground outline-none placeholder:text-muted-foreground/60"
        />
      </span>
    </span>
  );
}

function Hint({ keys, label }: { keys: string; label: string }) {
  return (
    <span className="flex items-center gap-1.5">
      <kbd className="rounded border border-border bg-surface px-1 py-0.5 text-[10px] leading-none text-muted-foreground">
        {keys}
      </kbd>
      {label}
    </span>
  );
}
