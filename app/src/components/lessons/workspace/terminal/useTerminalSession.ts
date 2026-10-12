import { useEffect, useRef, useState } from "react";

import { useTauriEvent } from "@/hooks/backend/useEvents";
import { createTermSink, decodeBase64 } from "@/lib/ed/termOutput";
import {
  WORKSPACE_EVENT,
  openTerminal,
  resizeTerminal,
  terminalInput,
  type WorkspaceEvent,
} from "@/lib/ed/workspace";

export type TerminalPhase = "idle" | "opening" | "open" | "exited" | "failed";

/** Before the view has laid out; the first fit resizes it. */
const FALLBACK_SIZE = { cols: 80, rows: 24 };

/**
 * Ed's terminal for one workspace: opened (`ed_terminal_open`) the first time
 * it is `wanted` — shown, unfolded and online — and again after a dropped
 * connection comes back, when Rust re-attaches the one still running on Ed
 * (`resumed`). Output that lands before the open returns its id is held and
 * replayed. A failed open waits for "Try again"; nothing retries by itself.
 */
export function useTerminalSession(challengeId: number, wanted: boolean, online: boolean) {
  const [sink] = useState(createTermSink);
  const [phase, setPhase] = useState<TerminalPhase>("idle");
  const [error, setError] = useState<string | null>(null);
  const idRef = useRef<string | null>(null);
  /** `term` events seen while opening, before the id is known. */
  const early = useRef<Array<{ id: string; data: string }> | null>(null);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  useEffect(() => {
    if (!wanted || !online || phase !== "idle") return;
    setPhase("opening");
    setError(null);
    early.current = [];
    const asked = sink.size() ?? FALLBACK_SIZE;
    openTerminal(challengeId, asked.cols, asked.rows)
      .then(({ id, resumed }) => {
        const held = early.current ?? [];
        early.current = null;
        if (!mounted.current) return;
        idRef.current = id;
        if (resumed) sink.note("Reattached to the terminal still running on Ed.");
        for (const e of held) if (e.id === id) sink.write(decodeBase64(e.data));
        setPhase("open");
        const now = sink.size();
        if (now && (now.cols !== asked.cols || now.rows !== asked.rows)) {
          resizeTerminal(challengeId, id, now.cols, now.rows).catch(warn);
        }
      })
      .catch((e) => {
        early.current = null;
        if (!mounted.current) return;
        setError(String(e));
        setPhase("failed");
      });
  }, [wanted, online, phase, challengeId, sink]);

  // A dropped socket takes the terminal with it; reconnecting opens it again.
  useEffect(() => {
    if (online || phase !== "open") return;
    idRef.current = null;
    sink.note("Disconnected from Ed.");
    setPhase("idle");
  }, [online, phase, sink]);

  useTauriEvent<WorkspaceEvent>(WORKSPACE_EVENT, ({ payload }) => {
    if (payload.challengeId !== challengeId) return;
    if (payload.kind === "term") {
      if (payload.id === idRef.current) sink.write(decodeBase64(payload.data));
      else early.current?.push({ id: payload.id, data: payload.data });
    } else if (payload.kind === "termState" && payload.id === idRef.current && payload.state === "exited") {
      idRef.current = null;
      sink.note("The terminal closed.");
      setPhase("exited");
    }
  });

  return {
    sink,
    phase,
    error,
    input(data: string) {
      const id = idRef.current;
      if (id) terminalInput(challengeId, id, data).catch(warn);
    },
    resize(cols: number, rows: number) {
      const id = idRef.current;
      if (id) resizeTerminal(challengeId, id, cols, rows).catch(warn);
    },
    /** A new terminal after the last one closed or failed to open. */
    restart() {
      if (phase !== "exited" && phase !== "failed") return;
      sink.reset();
      setPhase("idle");
    },
  };
}

const warn = (e: unknown) => console.warn("Ed terminal:", e);
