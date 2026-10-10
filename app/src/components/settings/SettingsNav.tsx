import { useMemo, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";

import {
  SIDE_NAV_ROW,
  SIDE_NAV_ROW_IDLE,
  SideNav,
  SideNavGroupLabel,
  SideNavLink,
  SideNavSearch,
} from "@/components/ui/layout/SideNav";
import {
  SETTINGS_GROUPS,
  SETTINGS_PAGES,
  searchSettings,
  settingsPage,
  type SettingsEntry,
  type SettingsJump,
} from "@/lib/search/settings";
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

          <SideNavSearch
            value={query}
            onChange={setQuery}
            onEnter={() => results[0] && open(results[0])}
            placeholder="Search settings"
          />
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
