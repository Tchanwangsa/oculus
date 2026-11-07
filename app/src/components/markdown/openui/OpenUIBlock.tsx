import { Component, useCallback, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Renderer, type ActionEvent, type OpenUIError } from "@openuidev/react-lang";
import { useAssistantSend } from "./assistant";
import { SaveArtifact } from "@/components/revision/SaveWidget";
import { library } from "./library";
import { toolProvider } from "./query";
import { QueryRefresh } from "./query/refresh";
import { QueryStatus } from "./query/QueryStatus";
import { isFenceFailure } from "./query/errors";
import { OpenUISkeleton, StreamingPending } from "./Streaming";
import { parseProgram, streamVerdict } from "./parse";
import { useWidgetRow, WidgetFence, WidgetRow } from "./state";
import { useWidgetStore, widgetKey } from "@/stores/chat/widgetStore";

/** The runtime stays mounted across deltas. Strict validation and durable
 * snapshots begin only when the closed fence belongs to a committed row. */
export default function OpenUIBlock({ source, fallback, blockIndex, savedState, onPersistState, hideSave = false, isStreaming = false }: { source: string; fallback: ReactNode; blockIndex: number; savedState?: Record<string, unknown>; onPersistState?: (state: Record<string, unknown>) => void; hideSave?: boolean; isStreaming?: boolean }) {
  const verdict = useMemo(() => isStreaming ? null : parseProgram(source), [source, isStreaming]);
  const stream = useMemo(() => isStreaming ? streamVerdict(source) : null, [source, isStreaming]);
  const [failed, setFailed] = useState(false);
  const [ready, setReady] = useState(false);
  const row = useWidgetRow();
  const key = row ? widgetKey(row.thread, row.row, blockIndex) : null;
  // A stable hydration object: feeding each update back to initialState would
  // re-initialize the runtime while the student drags a slider.
  const [initialState] = useState(() => {
    if (savedState) return savedState;
    const saved = key ? useWidgetStore.getState().entries[key] : undefined;
    return saved?.source === source ? saved.state : undefined;
  });
  const latest = useRef(initialState ?? {});
  const send = useAssistantSend();
  const onStateUpdate = useCallback((state: Record<string, unknown>) => {
    latest.current = state;
    if (isStreaming) return;
    onPersistState?.(state);
    if (key) useWidgetStore.getState().save(key, source, state);
  }, [key, source, onPersistState, isStreaming]);
  // A layout update flushes the runtime's initialization effects before the
  // content's first paint, including on the LiveTail → committed remount.
  useLayoutEffect(() => setReady(true), []);

  const onAction = useCallback(
    (event: ActionEvent) => {
      // Only the conversation step leaves the widget; @Set and @Reset are
      // handled inside the runtime and never reach here.
      if (event.type === "continue_conversation" && event.humanFriendlyMessage) send?.(event.humanFriendlyMessage);
    },
    [send],
  );
  const onError = useCallback((errors: OpenUIError[]) => {
    if (!isStreaming && isFenceFailure(errors)) setFailed(true);
  }, [isStreaming]);

  if (!isStreaming && (!verdict?.ok || failed)) return <>{fallback}</>;
  return (
    <Boundary fallback={isStreaming ? <OpenUISkeleton /> : fallback}>
      <div data-md={`\`\`\`openui\n${source}\n\`\`\``} className="openui-block group/openui" data-openui-streaming={isStreaming} aria-busy={isStreaming}>
        <WidgetRow.Provider value={isStreaming ? null : row}>
        <WidgetFence.Provider value={blockIndex}>
        <StreamingPending.Provider value={stream?.pending ?? false}>
        <Renderer response={source} library={library} toolProvider={toolProvider} isStreaming={isStreaming} onAction={onAction} onError={onError}
          initialState={initialState} onStateUpdate={onStateUpdate}
          publishObservability={false}>
          <QueryRefresh queries={(verdict?.result ?? stream?.result)?.queryStatements ?? []} isStreaming={isStreaming} />
          <QueryStatus />
          {ready && <div inert={isStreaming} className="contents"><Renderer.Content /></div>}
          {isStreaming && (!ready || !stream?.drawable) && <OpenUISkeleton />}
        </Renderer>
        </StreamingPending.Provider>
        </WidgetFence.Provider>
        </WidgetRow.Provider>
        {isStreaming && !hideSave && !stream?.card && <div aria-hidden="true" className="h-6" />}
        {!isStreaming && !hideSave && verdict?.ok && !verdict.card && <div className="opacity-0 transition-opacity will-change-[opacity] group-hover/openui:opacity-100 focus-within:opacity-100"><SaveArtifact source={source} state={() => latest.current} /></div>}
      </div>
    </Boundary>
  );
}

/** The runtime catches a component's throw and keeps its last good render,
 *  which on a first render is nothing; this catches the rest. */
class Boundary extends Component<{ fallback: ReactNode; children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    return this.state.failed ? this.props.fallback : this.props.children;
  }
}
