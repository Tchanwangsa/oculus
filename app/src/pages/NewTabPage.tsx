import { useState } from "react";
import { Chat, Globe, MagnifyingGlass } from "@phosphor-icons/react";
import { useNavigate } from "react-router-dom";
import { ROW, Section } from "@/components/home/Section";
import { SearchList, useSearchSelection } from "@/components/search/SearchList";
import { tabInfo } from "@/components/tabs/tabInfo";
import { usePaneTab, useTabId } from "@/components/tabs/TabContext";
import { useSearch } from "@/hooks/useSearch";
import { useSubjects } from "@/hooks/useSubjects";
import { searchEngine, searchHome } from "@/lib/browser";
import { openSearchItem, type SearchItem } from "@/lib/search";
import { openUrlInFocusedPane } from "@/lib/tabRouters";
import { useHarnessStore } from "@/stores/harnessStore";
import { useRecentTabsStore } from "@/stores/recentTabsStore";
import { useTabStore } from "@/stores/tabStore";

/** As much of the trail as a near-empty page can show without becoming a
 *  history page — the sidebar's Recent group shows five of the same list, and
 *  the field above is the faster route to anything older. */
const SHOWN = 6;

/**
 * Where the + button, ⌘T and a fresh split land: a blank page that asks where
 * you are going.
 *
 * It is deliberately *not* Home. Home is the launcher — the composer over
 * today's agenda, what you were last in, your projects — and a tab you opened
 * to put something beside what you are already reading does not want a second
 * dashboard. This page is a field, two doors and a trail.
 *
 * **The field is the whole page once you type in it.** It is the ⌘K search,
 * drawn inline rather than over the window (`app/src/lib/search.ts` is the one
 * behind both), and it answers with everything: subjects, files, lectures,
 * projects, tasks, the prose inside your parsed documents — and, when what you
 * typed is not in the library at all, the address you pasted or a web search
 * for it. A field that can only fail to find something is a field you learn
 * not to type in.
 *
 * The results are an **overlay** hung off the field, not a page that replaces
 * this one: the doors and the trail stay exactly where they are underneath,
 * and the field does not move as answers land. A search you abandon costs you
 * nothing — there is no page to come back from, only a list to dismiss.
 *
 * Both doors consume this tab rather than opening another, and so does a
 * picked web result. Chat is a plain navigation, so the tab becomes the
 * conversation. A page cannot be — it is a native WebView Rust owns — so in
 * the main half this tab closes itself behind the tab that page arrives in,
 * the way a browser's new-tab page becomes the page you asked for. In a
 * **split** half there is no tab to arrive in: `openUrlInFocusedPane` puts the
 * page in this pane instead, which is the entire point of having split.
 */
export default function NewTabPage() {
  const navigate = useNavigate();
  const paneId = useTabId();
  const { side } = usePaneTab();
  const recents = useRecentTabsStore((s) => s.recents).slice(0, SHOWN);
  const addTab = useTabStore((s) => s.addTab);
  const { subjects, current } = useSubjects();

  const [query, setQuery] = useState("");
  const typing = query.trim() !== "";
  const sections = useSearch(typing ? query : "", { subjects, current });

  /** A web page, opened where this pane is. The main half hands it to a tab
   *  and steps aside; a split half keeps it. */
  const openHere = async (url: string) => {
    await openUrlInFocusedPane(url);
    // The tab in front by now is the page's, so this is an ordinary close of a
    // background tab. If the snapshot has not landed yet this is the only tab
    // in the strip, and `closeTab` keeps it and sends it home — which is this
    // page, so the worst case is the tab staying where it already is.
    if (side === "main") useTabStore.getState().closeTab(paneId);
  };

  const newChat = () => {
    void useHarnessStore.getState().open(null);
    navigate("/chat");
  };

  function pick(item: SearchItem, newTab: boolean) {
    openSearchItem(item, {
      newTab,
      // This pane's own router, not the shell's: a result picked here lands in
      // the half the field is drawn in, which on a split tab is the half you
      // opened to put something in.
      navigate: (path) => navigate(path),
      addTab,
      openUrl: (url) => void openHere(url),
    });
  }

  const { selected, setIndex, onKeyDown } = useSearchSelection(sections, query, pick);

  return (
    <div className="page-scroll">
      <div className="mx-auto flex min-h-full max-w-md flex-col justify-center gap-4 px-6 py-10">
        {/* The field and its results are one stack: the list hangs off the
            field as an overlay rather than taking a slot in the column, so the
            page under it — the two doors, the Recent trail — stays exactly
            where it was, and the field never moves as results land. */}
        <div className="relative" onKeyDown={onKeyDown}>
          <div className="flex items-center gap-2.5 rounded-lg border border-border bg-card px-3 focus-within:border-brand/50">
            <MagnifyingGlass size={14} className="shrink-0 text-muted-foreground" />
            <input
              autoFocus
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              // Escape empties the field rather than leaving the page: there
              // is nothing to go back to from a new tab.
              onKeyDown={(e) => {
                if (e.key === "Escape" && query) {
                  e.stopPropagation();
                  setQuery("");
                }
              }}
              placeholder="Search your library, or paste a link"
              // One unconditional size: `text-base md:text-sm` is shadcn's iOS
              // zoom fix and this viewport is always past `md`. See CLAUDE.md.
              className="h-9 flex-1 bg-transparent text-[13px] text-foreground outline-none placeholder:text-muted-foreground"
            />
          </div>

          {/* Raised over the page rather than folded into it — the same
              popover surface the ⌘K palette sits on. */}
          {typing && (
            <div className="absolute inset-x-0 top-full z-20 mt-2 overflow-hidden rounded-lg border border-border bg-popover shadow-lg">
              <SearchList
                sections={sections}
                selected={selected}
                onHover={setIndex}
                onPick={pick}
                className="max-h-[min(60vh,420px)]"
                empty="Nothing in your library matches."
              />
            </div>
          )}
        </div>

        <Section>
          <button
            type="button"
            className={ROW}
            onClick={() => void openHere(searchHome())}
          >
            <Globe size={13} className="shrink-0 text-muted-foreground" />
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[12px] text-foreground">
                New browser tab
              </span>
              <span className="block truncate text-[11px] text-muted-foreground">
                Search {searchEngine().label}, or paste a link
              </span>
            </span>
          </button>
          <button type="button" className={ROW} onClick={newChat}>
            <Chat size={13} className="shrink-0 text-muted-foreground" />
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[12px] text-foreground">
                New chat
              </span>
              <span className="block truncate text-[11px] text-muted-foreground">
                Ask the agent about your library
              </span>
            </span>
          </button>
        </Section>

        {/* Nothing visited yet is nothing to say — the trail arrives with use,
            rather than an empty box apologising for being empty. */}
        {recents.length > 0 && (
          <Section title="Recent">
            {recents.map((entry) => {
              // Named exactly as its tab and its sidebar row are. Browser tabs
              // never enter the trail, so there is none to pass.
              const { title, icon } = tabInfo(entry.path, subjects, [], 13);
              return (
                <button
                  key={entry.key}
                  type="button"
                  title={title}
                  className={ROW}
                  data-tab-href={entry.path}
                  onClick={() => navigate(entry.path)}
                >
                  <span className="shrink-0 text-muted-foreground">{icon}</span>
                  <span className="min-w-0 flex-1 truncate text-[12px] text-foreground">
                    {title}
                  </span>
                </button>
              );
            })}
          </Section>
        )}
      </div>
    </div>
  );
}
