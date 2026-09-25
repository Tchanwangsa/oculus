import { useEffect } from "react";
import { ContinueSection } from "@/components/home/ContinueSection";
import { HomeComposer } from "@/components/home/HomeComposer";
import { ProjectsSection } from "@/components/home/ProjectsSection";
import { SyncLine } from "@/components/home/SyncLine";
import { TodaySection } from "@/components/home/TodaySection";
import { useNow } from "@/hooks/useNow";
import { useHarnessStore } from "@/stores/harnessStore";

/**
 * Home is the launcher: the composer is the page, with Today, Continue and
 * Projects beneath it. Each section hides itself when it has nothing to say.
 */
export default function HomePage() {
  // `useNow`, so a window left open overnight updates its heading.
  const now = useNow();

  // Loaded here because the composer and Continue both read it; owning it in
  // either would make the other silently depend on mount order.
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
        {/* Each section owns its read and returns null when empty. */}
        <TodaySection now={now} />
        <ContinueSection />
        <ProjectsSection />
      </div>
    </div>
  );
}
