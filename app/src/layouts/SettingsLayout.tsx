import { NavLink, Outlet } from "react-router-dom";
import { cn } from "@/lib/utils";

const TABS = [
  { to: "canvas",  label: "Canvas" },
  { to: "ai",      label: "AI" },
  { to: "storage", label: "Storage" },
  { to: "library", label: "Library" },
  { to: "browser", label: "Browser" },
  { to: "appearance", label: "Appearance" },
] as const;

/** Everything under /settings: SubjectLayout's shell, a title over an
 *  underline tab strip in the same centered column as the content. */
export default function SettingsLayout() {
  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="shrink-0 border-b border-border-subtle">
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
                    // -mb-px puts the active underline on the header's border.
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

      {/* `scroll`, not `auto` + `scrollbar-gutter`, which reserves nothing in WebKit. */}
      <div className="flex-1 min-h-0 overflow-y-scroll">
        <div className="mx-auto max-w-5xl px-6 py-6">
          <Outlet />
        </div>
      </div>
    </div>
  );
}
