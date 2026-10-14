import { useEffect, useRef, useState } from "react";

import { useTauriEvent } from "@/hooks/backend/useEvents";
import { flushForEd } from "@/lib/ed/flushForEd";
import { createTermSink, decodeBase64, runExitLines, type RunExit } from "@/lib/ed/termOutput";
import {
  WORKSPACE_EVENT,
  resizeRun,
  runInput,
  runWorkspace,
  stopRun,
  type RunFile,
  type WorkspaceEvent,
} from "@/lib/ed/workspace";
import type { WorkspaceSaver } from "@/lib/ed/workspaceSaver";

export type RunPhase = "idle" | "starting" | "running" | "stopping";

const FALLBACK_SIZE = { cols: 80, rows: 24 };

type RunEvent = Extract<WorkspaceEvent, { kind: "run" } | { kind: "runFiles" } | { kind: "runExit" }>;

/**
 * Run, as Ed's button: write pending saves (`flushForEd`), then
 * `ed_workspace_run` at the Run tab's size, its output streaming into a clean
 * Run view and typed input passed through. It ends with Ed's message and
 * "Program exited with code N", dim, and the files it left (`runFiles`) show
 * under the output. Events that beat the run id back are held and replayed.
 */
export function useRunSession(challengeId: number, saver: WorkspaceSaver | null) {
  const [sink] = useState(createTermSink);
  const [phase, setPhase] = useState<RunPhase>("idle");
  const [error, setError] = useState<string | null>(null);
  /** The last run's output files. */
  const [files, setFiles] = useState<RunFile[]>([]);
  const runRef = useRef<string | null>(null);
  const early = useRef<RunEvent[] | null>(null);
  const mounted = useRef(true);
  /** From the click until the run id is back, against a double click. */
  const starting = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const finish = (exit: RunExit) => {
    for (const line of runExitLines(exit)) sink.note(line);
    runRef.current = null;
    setPhase("idle");
  };

  /** Applies a run event for the current run; true when it ended the run. */
  const apply = (e: RunEvent): boolean => {
    if (e.runId !== runRef.current) return false;
    if (e.kind === "run") {
      sink.write(decodeBase64(e.data));
      return false;
    }
    if (e.kind === "runFiles") {
      setFiles(e.files);
      return false;
    }
    finish(e);
    return true;
  };

  useTauriEvent<WorkspaceEvent>(WORKSPACE_EVENT, ({ payload }) => {
    if (payload.challengeId !== challengeId) return;
    if (payload.kind !== "run" && payload.kind !== "runFiles" && payload.kind !== "runExit") return;
    if (early.current) early.current.push(payload);
    else apply(payload);
  });

  async function start() {
    if (phase !== "idle" || starting.current || !saver) return;
    starting.current = true;
    try {
      await begin(saver);
    } finally {
      starting.current = false;
    }
  }

  async function begin(saver: WorkspaceSaver) {
    setError(null);
    setPhase("starting");
    const blocked = await flushForEd(saver);
    if (!mounted.current) return;
    if (blocked) {
      setError(blocked);
      setPhase("idle");
      return;
    }
    sink.reset();
    setFiles([]);
    early.current = [];
    const size = sink.size() ?? FALLBACK_SIZE;
    try {
      const { runId } = await runWorkspace(challengeId, size.cols, size.rows);
      const held = early.current ?? [];
      early.current = null;
      if (!mounted.current) return;
      runRef.current = runId;
      if (held.some(apply)) return;
      setPhase("running");
      const now = sink.size();
      if (now && (now.cols !== size.cols || now.rows !== size.rows)) {
        resizeRun(challengeId, runId, now.cols, now.rows).catch(warn);
      }
    } catch (e) {
      early.current = null;
      if (!mounted.current) return;
      setError(String(e));
      setPhase("idle");
    }
  }

  return {
    sink,
    phase,
    error,
    files,
    dismissError: () => setError(null),
    start,
    stop() {
      const runId = runRef.current;
      if (!runId || phase !== "running") return;
      setPhase("stopping");
      stopRun(challengeId, runId).catch((e) => {
        if (!mounted.current) return;
        setError(String(e));
        setPhase("running");
      });
    },
    input(data: string) {
      const runId = runRef.current;
      if (runId) runInput(challengeId, runId, data).catch(warn);
    },
    resize(cols: number, rows: number) {
      const runId = runRef.current;
      if (runId) resizeRun(challengeId, runId, cols, rows).catch(warn);
    },
  };
}

const warn = (e: unknown) => console.warn("Ed run:", e);
