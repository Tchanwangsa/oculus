import { useMemo, useRef, useState } from "react";
import { NavLink, useLocation, useNavigate } from "react-router-dom";
import { MagnifyingGlass, X } from "@phosphor-icons/react";

import { useScrollFade } from "@/hooks/useScrollFade";
import {
  SETTINGS_GROUPS,
  SETTINGS_PAGES,
  searchSettings,
  settingsPage,
  type SettingsEntry,
  type SettingsJump,
} from "@/lib/settingsSearch";
import { cn } from "@/lib/utils";

/** The app sidebar's `NavItem` look, so both columns read as one system. */
const ROW = "flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-[12.5px] transition-colors";
const ROW_IDLE = "text-muted-foreground font-normal hover:bg-sidebar-item-hover hover:text-foreground";

/**
 * Settings' own column: a search over `SETTINGS_ENTRIES`, and the pages in
 * groups while it is empty. A result navigates with `SettingsJump` state, which
 * `SettingsLayout` turns into a scroll to the section.
 */
export default function SettingsNav() {
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const [query, setQuery] = useState("");
  const results = useMemo(() => searchSettings(query), [query]);
  const searching = query.trim() !== "";

  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef);

  const open = (entry: SettingsEntry) => {
    const jump: SettingsJump = { section: entry.section ?? null };
    // Relative to the settings route; a jump within the page replaces, not stacks.
    navigate(entry.page, { state: jump, replace: pathname.endsWith(`/${entry.page}`) });
    setQuery("");
  };

  return (
    <nav className="flex h-full w-52 shrink-0 flex-col border-r border-border-subtle bg-background">
      <div className="shrink-0 px-4 pt-4 pb-3">
        <h1 className="font-display text-[16px] font-semibold leading-none tracking-tight text-foreground">
          Settings
        </h1>
      </div>

      <div className="shrink-0 px-2 pb-2">
        <div className="flex h-8 items-center gap-2 rounded-full border border-input bg-card px-3 transition-[color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25">
          <MagnifyingGlass size={13} className="shrink-0 text-muted-foreground" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape" && query) {
                e.preventDefault();
                setQuery("");
              } else if (e.key === "Enter" && results[0]) {
                e.preventDefault();
                open(results[0]);
              }
            }}
            placeholder="Search settings"
            aria-label="Search settings"
            spellCheck={false}
            autoComplete="off"
            className="min-w-0 flex-1 bg-transparent text-[13px] text-foreground outline-none placeholder:text-muted-foreground"
          />
          {query && (
            <button
              type="button"
              aria-label="Clear search"
              onClick={() => setQuery("")}
              className="-mr-1 flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-sidebar-item-hover hover:text-foreground"
            >
              <X size={11} weight="bold" />
            </button>
          )}
        </div>
      </div>

      {/* Scrollbar hidden like the app sidebar's; the fades carry the affordance. */}
      <div
        ref={scrollRef}
        className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden px-2 pt-1 pb-3 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
      >
        {/* One wrapper: the fade's ResizeObserver watches the first child. */}
        <div>
          {searching ? (
            results.length > 0 ? (
              results.map((entry) => {
                const page = settingsPage(entry.page);
                return (
                  <button
                    key={`${entry.page}/${entry.section ?? ""}/${entry.title}`}
                    type="button"
                    onClick={() => open(entry)}
                    className={cn(ROW, ROW_IDLE, "items-start text-left")}
                  >
                    <page.icon size={16} className="mt-px shrink-0" />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate">{entry.title}</span>
                      <span className="block truncate text-[11px] text-muted-foreground">
                        {entry.section ? page.label : page.group}
                      </span>
                    </span>
                  </button>
                );
              })
            ) : (
              <p className="px-2 py-1.5 text-[12.5px] text-muted-foreground">No matching settings</p>
            )
          ) : (
            <div className="space-y-3">
              {SETTINGS_GROUPS.map((group) => (
                <div key={group}>
                  <div className="px-2 py-1 mb-0.5 text-[11px] font-medium tracking-wide text-muted-foreground">
                    {group}
                  </div>
                  {SETTINGS_PAGES.filter((p) => p.group === group).map((page) => (
                    <NavLink
                      key={page.id}
                      to={page.id}
                      className={({ isActive }) =>
                        cn(ROW, isActive ? "bg-sidebar-item-active text-foreground font-medium" : ROW_IDLE)
                      }
                    >
                      <page.icon size={16} className="shrink-0" />
                      <span className="min-w-0 flex-1 truncate">{page.label}</span>
                    </NavLink>
                  ))}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </nav>
  );
}
