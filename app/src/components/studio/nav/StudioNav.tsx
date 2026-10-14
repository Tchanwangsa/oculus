import { useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Cube, FilmSlate, Palette, Plus, SquaresFour } from "@phosphor-icons/react";

import { RecentVideoRow } from "@/components/studio/nav/RecentVideoRow";
import { ResizeHandle } from "@/components/ui/layout/ResizeHandle";
import {
  SideNav,
  SideNavHeading,
  SideNavGroupLabel,
  SideNavLink,
  SideNavSearch,
} from "@/components/ui/layout/SideNav";
import { useTabActive } from "@/components/tabs/TabContext";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import type { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import { COMPONENTS_HREF, NEW_VIDEO_HREF, THEMES_HREF, VIDEOS_HREF, videoHref } from "@/lib/studio/routes";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { useRenderStore } from "@/stores/studio/renderStore";
import { useVideosStore } from "@/stores/studio/videosStore";

/** Videos listed under Recent before a search widens it. */
const RECENT = 8;

/**
 * Studio's nav column: New video, Videos, then the newest videos with a
 * spinner on one that is rendering or whose thread is mid-turn. The search
 * narrows that list to every title that matches.
 */
export function StudioNav({ panel }: { panel: ReturnType<typeof useResizablePanel> }) {
  const navigate = useNavigate();
  const videos = useVideosStore((s) => s.videos);
  const rendering = useRenderStore((s) => s.active);
  // A primitive: `live` changes on every streamed token.
  const runningKey = useHarnessStore((s) =>
    Object.keys(s.live)
      .filter((id) => s.live[Number(id)].running)
      .join(","),
  );
  const [query, setQuery] = useState("");
  const { collapsed } = panel;

  const busy = useMemo(() => {
    const threads = new Set(runningKey ? runningKey.split(",").map(Number) : []);
    const slugs = new Set(Object.values(rendering).map((r) => r.slug));
    for (const v of videos) if (v.threadId != null && threads.has(v.threadId)) slugs.add(v.slug);
    return slugs;
  }, [runningKey, rendering, videos]);

  const q = query.trim().toLowerCase();
  const listed = useMemo(
    () => (q ? videos.filter((v) => v.title.toLowerCase().includes(q)) : videos.slice(0, RECENT)),
    [videos, q],
  );
  const first = useRef(listed[0]);
  first.current = listed[0];

  // Every tab stays mounted, so only the one in front answers the chord.
  // `e.code`, not `e.key`: on macOS ⌥B arrives as `∫`.
  const active = useTabActive();
  useWindowEvent("keydown", (e) => {
    const k = e as KeyboardEvent;
    if (!active || !k.altKey || !(k.metaKey || k.ctrlKey) || k.code !== "KeyB") return;
    k.preventDefault();
    panel.toggle();
  });

  return (
    <>
      <SideNav
        aria-label="Studio"
        width={panel.width}
        collapsed={collapsed}
        animate={!panel.dragging}
        header={
          <>
            <SideNavHeading icon={FilmSlate}>Studio</SideNavHeading>
            <SideNavSearch
              value={query}
              onChange={setQuery}
              onEnter={() => {
                const v = first.current;
                if (!v) return;
                setQuery("");
                navigate(videoHref(v.slug, v.title, v.threadId != null));
              }}
              placeholder="Search videos"
            />
          </>
        }
      >
        {!q && (
          <div className="mb-2">
            <SideNavLink to={NEW_VIDEO_HREF} end icon={Plus} label="New video" />
            <SideNavLink to={VIDEOS_HREF} icon={SquaresFour} label="Videos" />
            <SideNavLink to={COMPONENTS_HREF} icon={Cube} label="Components" />
            <SideNavLink to={THEMES_HREF} icon={Palette} label="Themes" />
          </div>
        )}
        {(q || listed.length > 0) && <SideNavGroupLabel>{q ? "Results" : "Recent"}</SideNavGroupLabel>}
        <div className="flex flex-col gap-0.5">
          {listed.map((v) => (
            <RecentVideoRow key={v.slug} video={v} busy={busy.has(v.slug)} onOpened={() => setQuery("")} />
          ))}
        </div>
        {q && listed.length === 0 && (
          <p className="px-2 py-1.5 text-[12px] text-muted-foreground">No video matches.</p>
        )}
      </SideNav>
      {/* On the seam: negative margins cost no layout width. */}
      {!collapsed && (
        <ResizeHandle
          onMouseDown={panel.onMouseDown}
          dragging={panel.dragging}
          label="Resize Studio sidebar"
          className="-mx-0.5"
        />
      )}
    </>
  );
}
