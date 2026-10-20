import { shortcut } from "@/lib/platform";
import { useEffect, useRef } from "react";
import { Outlet, useLocation } from "react-router-dom";

import { SideNavShell } from "@/components/ui/layout/SideNavShell";
import { SideNavCollapseToggle } from "@/components/ui/layout/SideNav";
import { PageHeader, PaneTitle } from "@/components/tabs/PaneHeader";
import { useSideNavPanel } from "@/hooks/ui/useSideNavPanel";
import SettingsNav from "@/components/settings/SettingsNav";
import { SETTINGS_PAGES, type SettingsJump } from "@/lib/search/settings";
import { settingsSectionId } from "@/components/settings/shared/section";
import { useScrollFade } from "@/hooks/ui/useScrollFade";

/** How long a jump waits for its section, and keeps it aligned while the page
 *  above it fills in. */
const JUMP_WINDOW_MS = 1000;
/** Gap left above a section a jump lands on. */
const JUMP_OFFSET = 16;

/** Everything under /settings: the settings nav column beside the page. */
export default function SettingsLayout() {
  const location = useLocation();
  const panel = useSideNavPanel({ defaultWidth: 208, minWidth: 208, maxWidth: 208, storageKey: "oculus-settings-nav-width" });
  const scrollerRef = useRef<HTMLDivElement>(null);
  const lastPath = useRef(location.pathname);
  const segment = location.pathname.split("/").filter(Boolean).pop();
  const fullBleed = SETTINGS_PAGES.some((p) => p.id === segment && p.fullBleed);
  useScrollFade(scrollerRef, "y", fullBleed);

  // A search result carries a section in `state` (not a hash: panes run memory
  // routers). Pages are lazy and load their rows async, so poll for the section
  // and re-align as content above it grows, until the window ends or the user scrolls.
  useEffect(() => {
    const scroller = scrollerRef.current;
    const section = (location.state as SettingsJump | null)?.section;
    const pageChanged = lastPath.current !== location.pathname;
    lastPath.current = location.pathname;
    if (!scroller) return;
    if (pageChanged) scroller.scrollTop = 0;
    if (!section) return;

    const id = settingsSectionId(section);
    const deadline = performance.now() + JUMP_WINDOW_MS;
    let frame = 0;
    let stopped = false;
    const stop = () => {
      stopped = true;
      cancelAnimationFrame(frame);
    };
    const align = () => {
      if (stopped) return;
      const el = scroller.querySelector<HTMLElement>(`#${id}`);
      if (el) {
        const top =
          el.getBoundingClientRect().top - scroller.getBoundingClientRect().top + scroller.scrollTop;
        scroller.scrollTop = Math.max(0, top - JUMP_OFFSET);
      }
      if (performance.now() < deadline) frame = requestAnimationFrame(align);
    };
    align();

    const inputs = ["wheel", "pointerdown", "keydown", "touchstart"] as const;
    for (const type of inputs) scroller.addEventListener(type, stop, { passive: true });
    return () => {
      stop();
      for (const type of inputs) scroller.removeEventListener(type, stop);
    };
  }, [location]);

  return (
    <SideNavShell panel={panel} nav={<SettingsNav panel={panel} />}>
      <PageHeader>
        <SideNavCollapseToggle collapsed={panel.collapsed} onToggle={panel.toggle} shortcut={shortcut("mod+alt+B")} />
        <PaneTitle>Settings</PaneTitle>
      </PageHeader>

      {fullBleed ? (
        // A table page scrolls its own rows (`GridTable`); with no scroller
        // here, a jump is the page's to handle.
        <div className="settings-content flex min-h-0 min-w-0 flex-1 flex-col">
          <Outlet />
        </div>
      ) : (
        // `scroll`, not `auto` + `scrollbar-gutter`, which reserves nothing in WebKit.
        <div ref={scrollerRef} className="settings-content min-h-0 min-w-0 flex-1 overflow-y-scroll">
          <div className="settings-gutters mx-auto max-w-3xl py-6">
            <Outlet />
          </div>
        </div>
      )}
    </SideNavShell>
  );
}
