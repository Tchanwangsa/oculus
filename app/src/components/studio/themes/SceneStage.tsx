import { useEffect, useMemo, useState } from "react";

import { StageView } from "@/components/studio/components/StageView";
import { DURATIONS, TimeBar } from "@/components/studio/components/TimeBar";
import { PillTabs } from "@/components/ui/table/PillTabs";
import type { ComponentsState } from "@/hooks/studio/useComponents";
import { usePlayhead } from "@/hooks/studio/usePlayhead";
import { useStageFrame } from "@/hooks/studio/useStageFrame";
import { useStageWidth } from "@/hooks/studio/useStageWidth";

/** The sample scenes a theme is previewed on: starter components, each in
 *  its first example. Only the ones the library has (and that load) show. */
const SCENES: ReadonlyArray<{ value: string; label: string }> = [
  { value: "TitleCard", label: "Title" },
  { value: "Axes", label: "Graph" },
  { value: "Graph", label: "Diagram" },
  { value: "Code", label: "Code" },
  { value: "Bars", label: "Chart" },
  { value: "Table", label: "Table" },
  { value: "Grid", label: "Grid" },
];

/**
 * The theme on a sample scene: pills choose the scene, the frame is
 * `studio_component_frame` at the playhead in the theme (its unsaved draft
 * when there is one), and the timeline plays or scrubs the scene's motion.
 */
export function SceneStage({
  theme,
  draftText,
  subject,
  components,
  revision,
}: {
  theme: string;
  /** The draft's text, drawn in place of the saved theme; null draws the
   *  saved one. */
  draftText: string | null;
  subject: string | null;
  components: ComponentsState;
  /** Bumped when the libraries changed under the page. */
  revision: number;
}) {
  const scenes = useMemo(
    () => SCENES.filter((s) => components.components.some((c) => c.name === s.value && c.problem == null)),
    [components.components],
  );
  const [scene, setScene] = useState<string>(SCENES[0].value);
  const shown = scenes.find((s) => s.value === scene) ?? scenes[0] ?? null;
  const info = shown ? components.components.find((c) => c.name === shown.value) : undefined;
  const props = info ? (Object.values(info.examples)[0] ?? {}) : {};

  const [duration, setDuration] = useState<number>(DURATIONS[1].value);
  const head = usePlayhead(duration);
  const [stageRef, width] = useStageWidth();
  const frame = useStageFrame(
    info && width > 0
      ? { name: info.name, subject, theme, themeText: draftText, props, t: head.t, width, duration, revision }
      : null,
  );
  // A frame that cannot be drawn would fail again at the next tick.
  const { pause } = head;
  useEffect(() => {
    if (frame.error) pause();
  }, [frame.error, pause]);

  return (
    <div className="space-y-3">
      {scenes.length > 0 && shown && (
        <div className="flex items-center gap-2">
          <span className="text-[11px] font-medium tracking-wide text-muted-foreground">Scene</span>
          <PillTabs tabs={scenes} value={shown.value} onChange={setScene} className="flex-wrap gap-1" />
        </div>
      )}
      {components.loaded && scenes.length === 0 ? (
        <div className="flex aspect-video w-full items-center justify-center rounded-xl bg-secondary px-6 text-center text-[13px] text-muted-foreground ring-1 ring-border">
          {components.error ?? "None of the starter components is in this library to preview the theme on."}
        </div>
      ) : (
        <StageView ref={stageRef} frame={frame} problem={null} alt={`${theme} on ${shown?.label ?? "a scene"} at ${head.t.toFixed(2)} s`} />
      )}
      <TimeBar head={head} duration={duration} onDuration={setDuration} />
    </div>
  );
}
