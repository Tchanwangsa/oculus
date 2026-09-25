import { useState } from "react";
import { MagnifyingGlass } from "@phosphor-icons/react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { SearchList, useSearchSelection } from "@/components/search/SearchList";
import { navigateActive, openUrlInFocusedPane } from "@/lib/tabRouters";
import { openSearchItem, type SearchItem } from "@/lib/search";
import { useSearch } from "@/hooks/useSearch";
import { useTauriEvent } from "@/hooks/useEvents";
import { usePaletteStore } from "@/stores/paletteStore";
import { useTabStore } from "@/stores/tabStore";
import { useSubjects } from "@/hooks/useSubjects";

/**
 * The ⌘K palette over everything reachable (library, pages, web). What it
 * searches is `app/src/lib/search.ts`, shared with the new-tab page. Enter
 * opens in the current (focused) pane, ⌘↵ in a new tab.
 */
export default function CommandPalette() {
  const open = usePaletteStore((s) => s.open);
  const setOpen = usePaletteStore((s) => s.setOpen);
  const toggle = usePaletteStore((s) => s.toggle);

  // A menu item, not a key handler: macOS gives the menu bar ⌘-keys first, which
  // also makes it work over a native browser tab. See `app/src-tauri/src/menu.rs`.
  useTauriEvent("menu-search", () => toggle());

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

  const [query, setQuery] = useState("");
  const sections = useSearch(query, { subjects, current });

  function pick(item: SearchItem, newTab: boolean) {
    onClose();
    openSearchItem(item, {
      newTab,
      // The shell's navigation, with its departure rules.
      navigate: navigateActive,
      addTab,
      openUrl: (url) => void openUrlInFocusedPane(url),
    });
  }

  const { selected, setIndex, onKeyDown } = useSearchSelection(sections, query, pick);

  return (
    <div onKeyDown={onKeyDown}>
      <div className="flex items-center gap-2.5 border-b border-border-subtle px-4">
        <MagnifyingGlass size={15} className="shrink-0 text-muted-foreground" />
        <input
          autoFocus
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search subjects, files, lectures…"
          className="h-11 flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground"
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
        <Hint keys="↵" label="Open" />
        <Hint keys="⌘ ↵" label="Open in new tab" />
        <Hint keys="esc" label="Close" />
      </div>
    </div>
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
