import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import { useAssistantSend } from "@/components/markdown/openui/assistant";
import { isDark, subscribeTheme, themeKey, useAppearance } from "@/lib/ui/themes";
import { SaveArtifact } from "@/components/revision/SaveWidget";
import { entryState, useWidgetStore } from "@/stores/chat/widgetStore";
import { WIDGET_HEIGHT_BUDGET } from "./constants";
import { fixRequest } from "./fixRequest";
import { sendModule, sendState } from "./messages";
import { buildSrcdoc, newNonce } from "./srcdoc";
import { readTokens } from "./tokens";
import { useKitBase, useTranspiled } from "./useWidgetSources";
import { useFrameInView } from "./useFrameInView";
import { useWidgetFrame } from "./useWidgetFrame";
import { Loading, Sketch, WidgetBar, WidgetCard, WidgetError } from "./WidgetBar";
import { WidgetExpand } from "./WidgetExpand";
import { AskLine, AskRunning } from "./ask/AskLine";
import { useWidgetAsk } from "./ask/useWidgetAsk";
import { streamGate } from "./stream/gate";
import { moduleCode } from "./stream/preview";
import { usePreview } from "./stream/usePreview";
import { cn } from "@/lib/utils";

/** Last reported heights by source, so a remounted widget starts at its size. */
const heights = new Map<string, number>();
/** Numbers each built document, to key its frame. */
let documents = 0;

/**
 * A widget: transpiled, built into a sandboxed `srcdoc` (`allow-scripts`
 * only, never same-origin) and sized to exactly what it reports — no clamp
 * below, no scrollbar; past `WIDGET_HEIGHT_BUDGET` it is clipped and shows
 * an overflow error. State is saved under `storeKey` and seeds the next
 * document (a theme change rebuilds it). Any failure is one line under the
 * title with Ask to fix. The frame is told when it leaves the view or the
 * expanded copy covers it, so a 3D scene releases its context. Its
 * `oculus/ask` requests, from either frame, go through one `useWidgetAsk`,
 * whose Allow and spinner are drawn here, outside the frame. Once it runs,
 * the title bar offers Save to Revision (not with `hideSave`). A saved
 * artifact passes `savedState` and `onPersistState`: its state is the row's,
 * and the store keeps only its grant.
 *
 * `streaming`: the reply is still arriving and `source` is a prefix. The
 * frame boots once, with no program, and each repaired prefix that
 * transpiles is posted into it (`usePreview`, `moduleCode`); the card is
 * dimmed and inert, says "Building…", and refuses requests and saves. When
 * the reply commits, this same component (the row keeps it) posts the final
 * program into the same frame: no second boot. A later rebuild (a theme
 * change) boots the committed program inline, as a widget that mounts
 * committed always does (docs/widgets.md#a-widget-draws-while-it-streams).
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
  streaming = false,
}: {
  source: string;
  title: string;
  md: string;
  storeKey: string;
  code: ReactNode;
  savedState?: Record<string, unknown>;
  onPersistState?: (state: Record<string, unknown>) => void;
  hideSave?: boolean;
  streaming?: boolean;
}) {
  // Decided once: a frame booted to stream takes every program as a message.
  const [streamed] = useState(streaming);
  const compiled = useTranspiled(streaming ? null : source);
  const preview = usePreview(source, streaming);
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
  const readState = useRef(() => ({}) as Record<string, unknown>);
  readState.current = () => latest.current ?? seed ?? entryState(useWidgetStore.getState().entries[storeKey], source);

  // Built from the last state at build time, never subscribed to it: a
  // state save must not reload the frame. A theme or accent change does.
  // A streamed frame first boots without a program, so neither a new preview
  // nor the commit rebuilds it; a rebuild after its final program was sent
  // (a theme change) boots that program inline, like any committed widget.
  const finalJs = compiled?.js ?? null;
  const [finalSent, setFinalSent] = useState(false);
  const docFor = useCallback(
    (js: string | null) =>
      kit?.base
        ? buildSrcdoc({ kitBase: kit.base, nonce: newNonce(), js, state: readState.current(), tokens: readTokens(), dark: isDark() })
        : null,
    [kit],
  );
  const inlineJs = streamed ? null : finalJs;
  const inlineKey = streamed ? null : storeKey;
  const built = useMemo(() => {
    void theme;
    void accent;
    void inlineKey;
    const js = streamed ? (finalSent ? finalJs : null) : inlineJs;
    if (!streamed && !js) return null;
    const doc = docFor(js);
    return doc ? { doc, id: ++documents, streams: js == null } : null;
    // `finalJs` and `finalSent` are read, not dependencies: a streamed frame takes its program by message.
  }, [docFor, theme, accent, streamed, inlineJs, inlineKey]);
  const doc = built?.doc ?? null;

  const save = useCallback(
    (state: Record<string, unknown>) => {
      latest.current = state;
      if (onPersistState) onPersistState(state);
      else useWidgetStore.getState().save(storeKey, source, state);
    },
    [storeKey, source, onPersistState],
  );
  const gate = streamGate(streaming, ask.handle, save);
  const inView = useFrameInView(frame, built?.id);
  // The expanded copy is a second context, and an observer can't see the dialog covering this one.
  const live = inView && !expanded;
  const report = useWidgetFrame(frame, doc, gate.onState, heights.get(source) ?? null, live, gate.onRequest);
  useEffect(() => {
    if (!streaming && report.height != null) heights.set(source, report.height);
  }, [report.height, source, streaming]);

  // A streaming document's programs: each preview while the reply streams,
  // then the committed one; each once per document, nothing after the final.
  const program = streaming
    ? preview && { js: preview.js, pending: preview.pending, final: false }
    : finalJs != null
      ? { js: finalJs, pending: [], final: true }
      : null;
  const modules = useRef(0);
  const sent = useRef<{ doc: string; js: string; final: boolean } | null>(null);
  useEffect(() => {
    if (!built?.streams || !report.spoken || !program || !kit?.base) return;
    const last = sent.current?.doc === built.doc ? sent.current : null;
    if (last && (last.final || (last.js === program.js && !program.final))) return;
    const id = ++modules.current;
    const code = moduleCode({ ...program, id, runtime: `${kit.base}runtime.js` });
    // Previews' errors and overflow never count; the final program's do.
    if (program.final) report.forget();
    sendModule(frame.current?.contentWindow, id, code, program.final);
    sent.current = { doc: built.doc, js: program.js, final: program.final };
    if (program.final) setFinalSent(true);
  });
  const committed = !built?.streams || finalSent;

  const error = streaming
    ? null
    : compiled?.error
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
  // A streaming document reports 0 before its first program draws.
  const drawn = built?.streams ? height > 0 : report.height != null;
  const waiting = !streaming && !error && (!doc || !drawn);
  // Only a widget that runs is worth keeping; a streamed one once its final program is in.
  const runs = !streaming && !error && !!doc && drawn && committed;
  // A committed program that doesn't compile has no frame, as if it had never streamed.
  const framed = !!doc && !(streamed && !streaming && compiled?.error);

  const expand = () => {
    const next = finalJs ? docFor(finalJs) : null;
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
        building={streaming}
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
        status={streaming ? undefined : <AskRunning ask={ask} />}
        source={streaming ? undefined : showSource}
        onSource={streaming ? undefined : () => setShowSource((s) => !s)}
        onExpand={!streaming && finalJs ? expand : undefined}
      />
      {!streaming && <AskLine ask={ask} />}
      {error && <WidgetError message={error} onAsk={send ? () => send(fixRequest(title, error, source)) : undefined} />}
      {waiting && <Loading />}
      {streaming && !drawn && <Sketch />}
      {framed && (
        // Inert and dimmed while it streams: nothing in a half-written widget takes a click.
        <div inert={streaming || undefined} className={cn("transition-opacity", streaming && "opacity-60")}>
          <iframe
            // A new element per document, so the old one's late messages can't match.
            key={built?.id}
            ref={frame}
            // Never `allow-same-origin`: the widget would get the app's origin and the Tauri IPC.
            sandbox="allow-scripts"
            srcDoc={doc!}
            title={title}
            style={{ height }}
            className="block w-full border-0"
          />
        </div>
      )}
      {showSource && !streaming && <div className="border-t border-border px-3 [&>pre]:my-3">{code}</div>}
      {expanded && (
        <WidgetExpand
          doc={expanded.doc}
          title={title}
          width={expanded.width}
          height={report.height ?? 0}
          onState={save}
          ask={ask}
          onClose={closeExpanded}
        />
      )}
    </WidgetCard>
  );
}
