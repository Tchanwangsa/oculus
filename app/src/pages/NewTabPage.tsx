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
import { chatHref } from "@/lib/harness";
import { useRecentTabsStore } from "@/stores/recentTabsStore";
import { useTabStore } from "@/stores/tabStore";

/** Recent entries shown; the search field is the route to anything older. */
const SHOWN = 6;

/**
 * Where the + button, ⌘T and a fresh split land — deliberately not Home: a
 * search field (the ⌘K search inline, `lib/search.ts`), two doors and a trail.
 *
 * Both doors and a picked web result consume this tab. A browser page is a
 * native WebView in its own tab, so in the main half this tab closes behind
 * it; in a split half `openUrlInFocusedPane` puts the page in this pane.
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

  const openHere = async (url: string) => {
    await openUrlInFocusedPane(url);
    // If the page's tab has not landed yet this is the only tab, and
    // `closeTab` keeps it — so the worst case is staying put.
    if (side === "main") useTabStore.getState().closeTab(paneId);
  };

  // A bare `/chat` is the empty composer: which thread a Chat tab shows is
  // in its route, so there is nothing in the store to clear first.
  const newChat = () => navigate(chatHref(null));

  function pick(item: SearchItem, newTab: boolean) {
    openSearchItem(item, {
      newTab,
      // This pane's own router, so a split half keeps what it picks.
      navigate: (path) => navigate(path),
      addTab,
      openUrl: (url) => void openHere(url),
    });
  }

  const { selected, setIndex, onKeyDown } = useSearchSelection(sections, query, pick);

  return (
    <div className="page-scroll">
      <div className="mx-auto flex min-h-full max-w-md flex-col justify-center gap-4 px-6 py-10">
        {/* Results overlay the page so nothing under the field moves. */}
        <div className="relative" onKeyDown={onKeyDown}>
          <div className="flex items-center gap-2.5 rounded-lg border border-border bg-card px-3 focus-within:border-brand/50">
            <MagnifyingGlass size={14} className="shrink-0 text-muted-foreground" />
            <input
              autoFocus
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape" && query) {
                  e.stopPropagation();
                  setQuery("");
                }
              }}
              placeholder="Search your library, or paste a link"
              // See CLAUDE.md: no `md:` size on a field.
              className="h-9 flex-1 bg-transparent text-[13px] text-foreground outline-none placeholder:text-muted-foreground"
            />
          </div>

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

        {recents.length > 0 && (
          <Section title="Recent">
            {recents.map((entry) => {
              // Browser tabs never enter the trail, hence no browser tabs arg.
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
