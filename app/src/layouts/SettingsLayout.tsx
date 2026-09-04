import { NavLink, Outlet } from "react-router-dom";
import { cn } from "@/lib/utils";

const TABS = [
  { to: "canvas",  label: "Canvas" },
  { to: "ai",      label: "AI" },
  { to: "storage", label: "Storage" },
  { to: "library", label: "Library" },
] as const;

/**
 * Everything under /settings. Same shell as SubjectLayout — page title over an
 * underline tab strip, sharing one centered column with the tab content.
 */
export default function SettingsLayout() {
  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="shrink-0 border-b border-border-subtle">
        {/* Same centered column as the tab content below it. */}
        <div className="mx-auto max-w-5xl px-6">
          <div className="pt-5 pb-3">
            <h1 className="text-[22px] font-semibold tracking-tight text-foreground leading-none">
              Settings
            </h1>
          </div>

          <nav className="flex items-center gap-1">
            {TABS.map((tab) => (
              <NavLink
                key={tab.to}
                to={tab.to}
                className={({ isActive }) =>
                  cn(
                    // -1px bottom margin so the active underline sits on the
                    // header's border rather than above it.
                    "-mb-px border-b-2 px-2 pb-2 pt-1 text-[12px] font-medium transition-colors",
                    isActive
                      ? "border-primary text-foreground"
                      : "border-transparent text-muted-foreground hover:text-foreground",
                  )
                }
              >
                {tab.label}
              </NavLink>
            ))}
          </nav>
        </div>
      </header>

      <div className="flex-1 min-h-0 overflow-y-auto">
        <div className="mx-auto max-w-5xl px-6 py-6">
          <Outlet />
        </div>
      </div>
    </div>
  );
}
