import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { Composer } from "@/components/harness/composer/Composer";
import { DropOverlay } from "@/components/ui/layout/DropOverlay";
import { StyleChip } from "@/components/studio/style/StyleChip";
import { readStyle, writeStyle, type StudioStyle } from "@/components/studio/style/style";
import { useNewVideo } from "@/hooks/studio/useNewVideo";
import { useThemes } from "@/hooks/studio/useThemes";
import { useStoredState } from "@/hooks/ui/useStoredState";
import { draftKey } from "@/stores/chat/draftStore";
import { useHarnessStore } from "@/stores/chat/harnessStore";

/**
 * The New video box: the hero question, a subject chip and the style chip over
 * a composer. Sending scaffolds the video and opens its thread
 * (`useNewVideo`). The subject is this page's own choice, not Chat's.
 */
export function NewVideo() {
  const store = useHarnessStore;
  const subjects = useHarnessStore((s) => s.subjects);
  const provider = useHarnessStore((s) => s.provider);
  const model = useHarnessStore((s) => s.model);
  const reasoning = useHarnessStore((s) => s.reasoning);
  const [subjectId, setSubjectId] = useState<number | null>(null);
  const [stored, setStored] = useStoredState("oculus-studio-style", readStyle, writeStyle);
  // The theme is per video: picked by hand it holds, else the subject's own.
  const [theme, setTheme] = useState<string | null>(null);
  const style: StudioStyle = useMemo(() => ({ ...stored, theme }), [stored, theme]);
  const [restore, setRestore] = useState<{ text: string; n: number } | null>(null);
  const columnRef = useRef<HTMLDivElement>(null);
  const [dropping, setDropping] = useState(false);

  useEffect(() => {
    void store.getState().loadSubjects();
  }, [store]);

  const code = useMemo(() => {
    const s = subjects.find((x) => x.id === subjectId);
    return s ? s.code : null;
  }, [subjects, subjectId]);
  const themes = useThemes(code);
  const { start, error } = useNewVideo(subjectId, code, style);

  const onStyle = useCallback(
    (next: StudioStyle) => {
      setTheme(next.theme);
      setStored(next);
    },
    [setStored],
  );
  const onSend = useCallback(
    (text: string) => {
      void start(text).then((ok) => {
        // The box cleared on send; a failure gives the words back.
        if (!ok) setRestore((r) => ({ text, n: (r?.n ?? 0) + 1 }));
      });
    },
    [start],
  );

  return (
    <div ref={columnRef} className="relative min-h-0 flex-1">
      <div className="h-full overflow-y-auto px-6 py-6">
        {/* Auto margins, not `justify-center`, which would clip an overflow at the top. */}
        <div className="mx-auto my-auto flex min-h-full w-full max-w-[720px] flex-col">
          <div className="my-auto flex flex-col gap-5 py-6">
            <h2 className="text-center font-display text-[26px] font-semibold tracking-tight text-foreground">
              What should the video explain?
            </h2>
            <Composer
              draftKey={draftKey(null, "studio")}
              provider={provider}
              model={model}
              reasoning={reasoning}
              providerLocked={false}
              subjects={subjects}
              subjectId={subjectId}
              onSubject={setSubjectId}
              subjectLocked={false}
              running={false}
              usage={null}
              rateLimits={[]}
              restore={restore}
              onRestored={() => setRestore(null)}
              onProvider={(p) => store.getState().setProvider(p)}
              onModel={(m) => store.getState().setModel(m)}
              onReasoning={(r) => store.getState().setReasoning(r)}
              onSend={onSend}
              onStop={() => {}}
              autoFocus
              dropRef={columnRef}
              onDropping={setDropping}
              placeholder="Describe it, and type @ to add a lecture, assignment or file"
              leading={<StyleChip style={style} onChange={onStyle} themes={themes} />}
            />
            {error && <p className="px-0.5 text-[12px] text-destructive">{error}</p>}
          </div>
        </div>
      </div>
      <DropOverlay show={dropping} label="Drop to attach a picture" />
    </div>
  );
}
