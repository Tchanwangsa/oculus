import { useEffect, useRef, useState } from "react";
import { ArrowCounterClockwise, CircleNotch } from "@phosphor-icons/react";

import { ChangeBox } from "@/components/studio/components/ChangeBox";
import { ExamplePills } from "@/components/studio/components/ExamplePills";
import { Inspector } from "@/components/studio/components/inspector/Inspector";
import { OwnVersionActions } from "@/components/studio/components/OwnVersionActions";
import { SourceView } from "@/components/studio/components/SourceView";
import { StageView } from "@/components/studio/components/StageView";
import { ThemeSelect } from "@/components/studio/components/ThemeSelect";
import { DURATIONS, TimeBar } from "@/components/studio/components/TimeBar";
import { StarterMark } from "@/components/studio/LevelMark";
import { InspectorColumn, stageMaxWidth, type InspectorPanel } from "@/components/studio/panel/InspectorColumn";
import { Button } from "@/components/ui/button";
import { PillTabs } from "@/components/ui/table/PillTabs";
import type { useLibraryChange } from "@/hooks/studio/useLibraryChange";
import { useComponentSource } from "@/hooks/studio/useComponentSource";
import { usePlayhead } from "@/hooks/studio/usePlayhead";
import { useStageFrame, type FrameRequest } from "@/hooks/studio/useStageFrame";
import { useStageWidth } from "@/hooks/studio/useStageWidth";
import type { ThemesState } from "@/hooks/studio/useThemes";
import { displayCode } from "@/lib/format/format";
import type { ComponentInfo } from "@/lib/studio/components";
import type { PropValues } from "@/lib/studio/props";
import { cn } from "@/lib/utils";

type Tab = "change" | "source";
const TABS: ReadonlyArray<{ value: Tab; label: string }> = [
  { value: "change", label: "Ask for a change" },
  { value: "source", label: "Source" },
];

const clone = (props: PropValues | undefined): PropValues => structuredClone(props ?? {});
const sameProps = (a: PropValues, b: PropValues) => JSON.stringify(a) === JSON.stringify(b);

/**
 * One component on the stage: the centre column (theme, examples, the frame,
 * the timeline, the change box and the source) and the props inspector
 * beside it, which folds away (`panel`) and lets the stage grow. The page
 * keys it by component, so a new one starts from its first example; a reload
 * after a change (a new `info`) starts the examples over too.
 */
export function Workbench({
  info,
  subject,
  themes,
  theme,
  onTheme,
  revision,
  change,
  onLibraryChanged,
  panel,
}: {
  info: ComponentInfo;
  /** The library's course code; null is the global library. */
  subject: string | null;
  themes: ThemesState;
  /** The chosen theme, null until themes are listed. */
  theme: string | null;
  onTheme: (name: string) => void;
  /** Bumped when the component's files changed under the page. */
  revision: number;
  change: ReturnType<typeof useLibraryChange>;
  /** The component was copied into or removed from the subject's library. */
  onLibraryChanged: () => void;
  /** The props inspector's width and fold, held by the page for its header. */
  panel: InspectorPanel;
}) {
  const names = Object.keys(info.examples);
  const [example, setExample] = useState<string | null>(names[0] ?? null);
  const [values, setValues] = useState<PropValues>(() => clone(info.examples[names[0]]));
  const exampleRef = useRef(example);
  exampleRef.current = example;
  useEffect(() => {
    const keys = Object.keys(info.examples);
    const keep = exampleRef.current && exampleRef.current in info.examples ? exampleRef.current : (keys[0] ?? null);
    setExample(keep);
    setValues(clone(keep ? info.examples[keep] : undefined));
  }, [info]);

  const [duration, setDuration] = useState<number>(DURATIONS[1].value);
  const head = usePlayhead(duration);
  const [stageRef, width] = useStageWidth();
  const [tab, setTab] = useState<Tab>("change");
  const source = useComponentSource(info.name, subject, revision);

  const request: FrameRequest | null =
    info.problem == null && theme && width > 0
      ? { name: info.name, subject, theme, props: values, t: head.t, width, duration, revision }
      : null;
  const frame = useStageFrame(request);
  // A frame that cannot be drawn would fail again at the next tick.
  const { pause } = head;
  useEffect(() => {
    if (frame.error) pause();
  }, [frame.error, pause]);

  const exampleProps = example ? info.examples[example] : undefined;
  const tokens = themes.themes.find((t) => t.name === theme)?.tokens ?? null;
  const pick = (name: string) => {
    setExample(name);
    setValues(clone(info.examples[name]));
  };

  return (
    <>
      <section className="page-scroll min-h-0 min-w-0 flex-1">
        <div
          className={cn(
            "mx-auto flex w-full flex-col gap-4 px-6 py-5 transition-[max-width] duration-200 ease-out",
            stageMaxWidth(panel),
          )}
        >
          <div className="flex items-center gap-3">
            <div className="min-w-0 flex-1">
              <div className="flex min-w-0 items-center gap-2">
                <h2 className="truncate text-[18px] font-semibold leading-tight text-foreground">{info.name}</h2>
                {info.starter && <StarterMark />}
              </div>
              <p className="text-[12px] text-muted-foreground">
                {info.kind === "shell" ? "Shell" : "Component"} · {levelText(info, subject)}
              </p>
            </div>
            <ThemeSelect state={themes} value={theme ?? ""} onChange={onTheme} />
          </div>
          {subject && (
            <div className="-mt-2 flex flex-wrap items-center gap-2">
              <OwnVersionActions
                kind="component"
                name={info.name}
                level={info.level}
                shadows={info.shadows}
                subject={subject}
                onDone={onLibraryChanged}
              />
            </div>
          )}
          {themes.error && (
            <p data-selectable className="text-[12px] leading-snug text-destructive">
              Themes could not be listed: {themes.error}
            </p>
          )}
          <ExamplePills names={names} value={example} onChange={pick} />
          <StageView ref={stageRef} frame={frame} problem={info.problem} alt={`${info.name} at ${head.t.toFixed(2)} s`} />
          <TimeBar head={head} duration={duration} onDuration={setDuration} />
          <div className="space-y-3 pt-1">
            <div className="flex items-center gap-3">
              <PillTabs tabs={TABS} value={tab} onChange={setTab} />
              {change.working != null && (
                <span className="flex items-center gap-1.5 text-[11.5px] text-muted-foreground">
                  <CircleNotch size={12} className="animate-spin text-brand" />
                  Changing {change.working}
                </span>
              )}
            </div>
            {tab === "change" ? (
              <ChangeBox
                name={info.name}
                working={change.working}
                applied={change.applied}
                error={change.error}
                threadId={change.threadId}
                onSend={(request) => change.send(info.name, info.level, request)}
                onUndo={change.undo}
              />
            ) : (
              <SourceView state={source} subject={subject} />
            )}
          </div>
        </div>
      </section>
      <InspectorColumn
        panel={panel}
        label="Props"
        title="Props"
        actions={
          info.problem == null ? (
            <Button
              type="button"
              variant="ghost"
              size="icon-xs"
              aria-label="Reset to example"
              title="Reset to example"
              disabled={sameProps(values, exampleProps ?? {})}
              onClick={() => setValues(clone(exampleProps))}
              className="text-muted-foreground"
            >
              <ArrowCounterClockwise size={13} />
            </Button>
          ) : null
        }
      >
        {info.problem != null ? (
          <p className="py-3 text-[12.5px] leading-snug text-muted-foreground">
            The props are known once the component loads. Ask for a change to fix it.
          </p>
        ) : (
          <Inspector props={info.props} values={values} tokens={tokens} onChange={setValues} />
        )}
      </InspectorColumn>
    </>
  );
}

function levelText(info: ComponentInfo, subject: string | null): string {
  if (info.level === "global") return subject ? "From the global library" : "Global library";
  const code = subject ? displayCode(subject) : "the subject";
  return info.shadows ? `Own version in ${code}` : `${code}'s library`;
}
