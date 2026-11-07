import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import { useAssistantSend } from "@/components/markdown/openui/assistant";
import { isDark, subscribeTheme, themeKey, useAppearance } from "@/lib/ui/themes";
import { SaveArtifact } from "@/components/revision/SaveWidget";
import { entryState, useWidgetStore } from "@/stores/chat/widgetStore";
import { WIDGET_HEIGHT_BUDGET } from "./constants";
import { fixRequest } from "./fixRequest";
import { sendState } from "./messages";
import { buildSrcdoc, newNonce } from "./srcdoc";
import { readTokens } from "./tokens";
import { useKitBase, useTranspiled } from "./useWidgetSources";
import { useFrameInView } from "./useFrameInView";
import { useWidgetFrame } from "./useWidgetFrame";
import { Loading, WidgetBar, WidgetCard, WidgetError } from "./WidgetBar";
import { WidgetExpand } from "./WidgetExpand";
import { AskLine, AskRunning } from "./ask/AskLine";
import { useWidgetAsk } from "./ask/useWidgetAsk";

/** Last reported heights by source, so a remounted widget starts at its size. */
const heights = new Map<string, number>();
/** Numbers each built document, to key its frame. */
let documents = 0;

/**
 * A committed widget: transpiled, built into a sandboxed `srcdoc`
 * (`allow-scripts` only, never same-origin) and sized to exactly what it
 * reports — no clamp below, no scrollbar; past `WIDGET_HEIGHT_BUDGET` it is
 * clipped and shows an overflow error. State is saved under `storeKey` and
 * seeds the next document (a theme change rebuilds it). Any failure is one
 * line under the title with Ask to fix. The frame is told when it leaves the
 * view or the expanded copy covers it, so a 3D scene releases its context.
 * Its `oculus/ask` requests, from either frame, go through one
 * `useWidgetAsk`, whose Allow and spinner are drawn here, outside the frame.
 * Once it runs, the title bar offers Save to Revision (not with `hideSave`).
 * A saved artifact passes `savedState` and `onPersistState`: its state is
 * the row's, and the store keeps only its grant.
 */
export function LiveWidget({
  source,
  title,
  md,
  storeKey,
  code,
  savedState,
  onPersistState,
  hideSave = false,
}: {
  source: string;
  title: string;
  md: string;
  storeKey: string;
  code: ReactNode;
  savedState?: Record<string, unknown>;
  onPersistState?: (state: Record<string, unknown>) => void;
  hideSave?: boolean;
}) {
  const compiled = useTranspiled(source);
  const kit = useKitBase();
  const theme = useSyncExternalStore(subscribeTheme, themeKey);
  const { accent } = useAppearance();
  const send = useAssistantSend();
  const frame = useRef<HTMLIFrameElement>(null);
  const [showSource, setShowSource] = useState(false);
  const [expanded, setExpanded] = useState<{ doc: string; width: number } | null>(null);
  const latest = useRef<Record<string, unknown> | null>(null);
  // Read once: a saved row comes back changed after every save it caused.
  const [seed] = useState(savedState);
  const ask = useWidgetAsk(storeKey, source, send);

  // Built from the last state at build time, never subscribed to it: a
  // state save must not reload the frame. A theme or accent change does.
  const build = useCallback(() => {
    if (!compiled?.js || !kit?.base) return null;
    const state = latest.current ?? seed ?? entryState(useWidgetStore.getState().entries[storeKey], source);
    return buildSrcdoc({ kitBase: kit.base, nonce: newNonce(), js: compiled.js, state, tokens: readTokens(), dark: isDark() });
  }, [compiled, kit, storeKey, source, seed]);
  const built = useMemo(() => {
    void theme;
    void accent;
    const doc = build();
    return doc ? { doc, id: ++documents } : null;
  }, [build, theme, accent]);
  const doc = built?.doc ?? null;

  const onState = useCallback(
    (state: Record<string, unknown>) => {
      latest.current = state;
      if (onPersistState) onPersistState(state);
      else useWidgetStore.getState().save(storeKey, source, state);
    },
    [storeKey, source, onPersistState],
  );
  const inView = useFrameInView(frame, built?.id);
  // The expanded copy is a second context, and an observer can't see the dialog covering this one.
  const live = inView && !expanded;
  const report = useWidgetFrame(frame, doc, onState, heights.get(source) ?? null, live, ask.handle);
  useEffect(() => {
    if (report.height != null) heights.set(source, report.height);
  }, [report.height, source]);

  const error = compiled?.error
    ? compiled.error
    : kit?.error
      ? `The widget kit is unavailable: ${kit.error}`
      : report.error
        ? report.error.message
        : report.height != null && report.height > WIDGET_HEIGHT_BUDGET
          ? `It is ${report.height}px tall and a widget may be ${WIDGET_HEIGHT_BUDGET}px; put the rest on Pages`
          : report.wide
            ? `It is ${report.wide.width}px wide in a ${report.wide.frameWidth}px frame`
            : null;

  const height = report.height == null ? 0 : Math.min(report.height, WIDGET_HEIGHT_BUDGET);
  const waiting = !error && (!doc || report.height == null);
  // Only a widget that runs is worth keeping.
  const runs = !error && !!doc && report.height != null;

  const expand = () => {
    const next = build();
    if (next) setExpanded({ doc: next, width: frame.current?.clientWidth ?? 0 });
  };
  const closeExpanded = () => {
    setExpanded(null);
    if (latest.current) sendState(frame.current?.contentWindow, latest.current);
  };

  return (
    <WidgetCard md={md}>
      <WidgetBar
        title={title}
        save={
          runs && !hideSave ? (
            <SaveArtifact
              source={source}
              format="widget"
              defaultTitle={title}
              // The state alone: the entry's Allow stays with this fence.
              state={() => latest.current ?? entryState(useWidgetStore.getState().entries[storeKey], source)}
              className="flex-nowrap"
            />
          ) : undefined
        }
        status={<AskRunning ask={ask} />}
        source={showSource}
        onSource={() => setShowSource((s) => !s)}
        onExpand={doc ? expand : undefined}
      />
      <AskLine ask={ask} />
      {error && <WidgetError message={error} onAsk={send ? () => send(fixRequest(title, error, source)) : undefined} />}
      {waiting && <Loading />}
      {doc && (
        <iframe
          // A new element per document, so the old one's late messages can't match.
          key={built?.id}
          ref={frame}
          // Never `allow-same-origin`: the widget would get the app's origin and the Tauri IPC.
          sandbox="allow-scripts"
          srcDoc={doc}
          title={title}
          style={{ height }}
          className="block w-full border-0"
        />
      )}
      {showSource && <div className="border-t border-border px-3 [&>pre]:my-3">{code}</div>}
      {expanded && (
        <WidgetExpand
          doc={expanded.doc}
          title={title}
          width={expanded.width}
          height={report.height ?? 0}
          onState={onState}
          ask={ask}
          onClose={closeExpanded}
        />
      )}
    </WidgetCard>
  );
}
