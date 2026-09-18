import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
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
import { usePaletteStore } from "@/stores/paletteStore";
import { useTabStore } from "@/stores/tabStore";
import { useSubjects } from "@/hooks/useSubjects";

/**
 * The ⌘K palette: one field over everything you can reach — subjects, files,
 * lectures, projects, tasks, the words inside your parsed documents, the app's
 * own pages, and the web when the answer is not in the library at all.
 *
 * What it searches and in what order is `app/src/lib/search.ts`, shared with
 * the new-tab page's field so the two cannot drift; this component is the
 * dialog around it.
 *
 * Enter goes there in the current tab — the focused half of it, if the tab is
 * split — and ⌘↵ in a new one, the rule the sidebar's Recent rows already
 * follow.
 */
export default function CommandPalette() {
  const open = usePaletteStore((s) => s.open);
  const setOpen = usePaletteStore((s) => s.setOpen);
  const toggle = usePaletteStore((s) => s.toggle);

  // ⌘K is a menu item, not a key handler: macOS gives the menu bar every
  // ⌘-key before a webview sees it. That is also what makes the palette reach
  // you while a browser tab's native page holds focus — which is exactly when
  // you most want a way out of it. See `app/src-tauri/src/menu.rs`.
  useEffect(() => {
    const pending = listen("menu-search", () => toggle());
    return () => {
      pending.then((un) => un()).catch(() => {});
    };
  }, [toggle]);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent
        showCloseButton={false}
        // Palettes hang from the top of the window rather than sitting in the
        // middle of it: the list grows downwards and the field must not move
        // as it does.
        className="top-[14%] translate-y-0 gap-0 overflow-hidden rounded-xl border-border bg-popover p-0 shadow-lg sm:max-w-xl"
      >
        <DialogTitle className="sr-only">Search</DialogTitle>
        <DialogDescription className="sr-only">
          Find a subject, file, lecture, project or page.
        </DialogDescription>
        {/* Mounted with the dialog, so each open starts on an empty field and
            a fresh read of the library. */}
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
      // The shell's door: the focused pane of the tab in front, with all the
      // rules that hang off it (a browser tab gets a tab of its own, a playing
      // lecture is asked about, the peek is shut behind you).
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
