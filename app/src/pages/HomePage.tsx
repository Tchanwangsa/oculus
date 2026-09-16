import { useEffect } from "react";
import { ContinueSection } from "@/components/home/ContinueSection";
import { HomeComposer } from "@/components/home/HomeComposer";
import { ProjectsSection } from "@/components/home/ProjectsSection";
import { SyncLine } from "@/components/home/SyncLine";
import { TodaySection } from "@/components/home/TodaySection";
import { useNow } from "@/hooks/useNow";
import { useHarnessStore } from "@/stores/harnessStore";

/**
 * Home is the launcher. Asking is the common landing intent, so the composer
 * *is* the page — Today, Continue and Projects are a short trail beneath it
 * rather than a dashboard the composer has to share space with. `/` used to
 * redirect straight to `/chat`; this replaces that redirect, and keeps the one
 * thing the redirect was really for at the top of the first screen.
 *
 * Every section below hides itself entirely when it has nothing to say — the
 * no-placeholder-UI rule — so a quiet day is a shorter page, not a page of
 * empty states apologising for being empty.
 */
export default function HomePage() {
  // `useNow` and not a render-time `new Date()`: a window left open overnight
  // would otherwise keep yesterday's heading until something else re-rendered.
  const now = useNow();

  // The page loads the subject list once because two sections read it: the
  // composer's scope picker offers it, and Continue's thread rows resolve a
  // thread's subject code out of it. Left in either one, the other would
  // quietly depend on that sibling being mounted first — reorder or drop the
  // composer and every thread row reads "Library" with nothing erroring. The
  // setter is idempotent, so owning it here costs a single query either way.
  useEffect(() => {
    void useHarnessStore.getState().loadSubjects();
  }, []);

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-2xl px-6 py-6 space-y-6">
        <div>
          <h1 className="text-[22px] font-semibold leading-none tracking-tight text-foreground">
            {now.toLocaleDateString("en-AU", {
              weekday: "long",
              day: "numeric",
              month: "long",
            })}
          </h1>
          <SyncLine />
        </div>

        <HomeComposer />
        {/* Each section owns its own read through `useHomeSection` — mount, the
            tab's front edge, and its own events — and each returns null when it
            has nothing, so the order below is the whole layout and `space-y-6`
            never spaces a gap. The page owns only what more than one of them
            needs: the clock, which Today shares, and the subject list. */}
        <TodaySection now={now} />
        <ContinueSection />
        <ProjectsSection />
      </div>
    </div>
  );
}
