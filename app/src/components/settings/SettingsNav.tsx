import { useMemo, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { MagnifyingGlass, X } from "@phosphor-icons/react";

import {
  SIDE_NAV_ROW,
  SIDE_NAV_ROW_IDLE,
  SideNav,
  SideNavGroupLabel,
  SideNavLink,
} from "@/components/ui/SideNav";
import {
  SETTINGS_GROUPS,
  SETTINGS_PAGES,
  searchSettings,
  settingsPage,
  type SettingsEntry,
  type SettingsJump,
} from "@/lib/settingsSearch";
import { cn } from "@/lib/utils";

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

  const open = (entry: SettingsEntry) => {
    const jump: SettingsJump = { section: entry.section ?? null };
    // Relative to the settings route; a jump within the page replaces, not stacks.
    navigate(entry.page, { state: jump, replace: pathname.endsWith(`/${entry.page}`) });
    setQuery("");
  };

  return (
    <SideNav
      header={
        <>
          <div className="px-4 pt-4 pb-3">
            <h1 className="font-display text-[16px] font-semibold leading-none tracking-tight text-foreground">
              Settings
            </h1>
          </div>

          <div className="px-2 pb-2">
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
        </>
      }
    >
      {searching ? (
        results.length > 0 ? (
          results.map((entry) => {
            const page = settingsPage(entry.page);
            return (
              <button
                key={`${entry.page}/${entry.section ?? ""}/${entry.title}`}
                type="button"
                onClick={() => open(entry)}
                className={cn(SIDE_NAV_ROW, SIDE_NAV_ROW_IDLE, "items-start text-left")}
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
              <SideNavGroupLabel>{group}</SideNavGroupLabel>
              {SETTINGS_PAGES.filter((p) => p.group === group).map((page) => (
                <SideNavLink key={page.id} to={page.id} icon={page.icon} label={page.label} />
              ))}
            </div>
          ))}
        </div>
      )}
    </SideNav>
  );
}
