import { Component, useCallback, useMemo, useRef, useState, type ReactNode } from "react";
import { Renderer, type ActionEvent, type OpenUIError } from "@openuidev/react-lang";
import { useAssistantSend } from "./assistant";
import { SaveArtifact } from "@/components/revision/SaveWidget";
import { library } from "./library";
import { parseProgram } from "./parse";
import { useWidgetRow, WidgetFence } from "./state";
import { useWidgetStore, widgetKey } from "@/stores/chat/widgetStore";

/**
 * A finished ```openui fence, drawn as live widgets. Loaded lazily by
 * `OpenUIFence`, so the runtime and the library stay out of the startup
 * bundle. Anything short of a clean program — a parse error, a component
 * that throws, a runtime error — shows `fallback`, the fence as code. A
 * proposal or job card is part of the chat, not a widget to keep, so it has
 * no Save to Revision.
 */
export default function OpenUIBlock({ source, fallback, blockIndex, savedState, onPersistState, hideSave = false }: { source: string; fallback: ReactNode; blockIndex: number; savedState?: Record<string, unknown>; onPersistState?: (state: Record<string, unknown>) => void; hideSave?: boolean }) {
  const verdict = useMemo(() => parseProgram(source), [source]);
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
    onPersistState?.(state);
    if (key) useWidgetStore.getState().save(key, source, state);
  }, [key, source, onPersistState]);
  // Renderer initializes its store in an effect, before onParseResult. Mount
  // its content only after that commit, so defaults never flash as zeros.
  const onParseResult = useCallback(() => setReady(true), []);

  const onAction = useCallback(
    (event: ActionEvent) => {
      // Only the conversation step leaves the widget; @Set and @Reset are
      // handled inside the runtime and never reach here.
      if (event.type === "continue_conversation" && event.humanFriendlyMessage) send?.(event.humanFriendlyMessage);
    },
    [send],
  );
  const onError = useCallback((errors: OpenUIError[]) => {
    if (errors.some((e) => e.severity !== "warning")) setFailed(true);
  }, []);

  if (!verdict.ok || failed) return <>{fallback}</>;
  return (
    <Boundary fallback={fallback}>
      <div data-md={`\`\`\`openui\n${source}\n\`\`\``} className="openui-block group/openui">
        <WidgetFence.Provider value={blockIndex}>
        <Renderer response={source} library={library} onAction={onAction} onError={onError}
          initialState={initialState} onStateUpdate={onStateUpdate} onParseResult={onParseResult}
          publishObservability={false}>
          {ready && <Renderer.Content />}
        </Renderer>
        </WidgetFence.Provider>
        {!hideSave && !verdict.card && <div className="opacity-0 transition-opacity group-hover/openui:opacity-100 focus-within:opacity-100"><SaveArtifact source={source} state={() => latest.current} /></div>}
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
