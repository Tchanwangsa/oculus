import { useCallback, useEffect, useRef, useState } from "react";
import { CheckCircle, CircleNotch, Copy } from "@phosphor-icons/react";

import { harnessUpdateRun, harnessUpdates, type Provider, type UpdateInfo } from "@/lib/harness";
import { copyText } from "@/lib/utils";
import { listen } from "@tauri-apps/api/event";
import { refreshBridgeHealth } from "@/hooks/useBridgeHealth";
import { Button } from "@/components/ui/button";
import { AnsiText, CodeText } from "@/components/markdown/MdComponents";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

/** One line of a running update, or its last event (`InstallLine`'s shape). */
interface UpdateLine {
  provider: Provider;
  line: string | null;
  done: boolean;
  ok: boolean | null;
  status: string | null;
}

const UPDATE_EVENT = "harness-update";

export interface AgentUpdateRun {
  command: string;
  lines: string[];
  result: { ok: boolean; status: string } | null;
  error: string | null;
}

interface UpdatesState {
  updates: UpdateInfo[] | null;
  checking: boolean;
  queue: Provider[];
  current: Provider | null;
  runs: Partial<Record<Provider, AgentUpdateRun>>;
}

/**
 * Update checks and runs for Settings' CLI agents, held at module level so a
 * queue and its output outlive the page: leaving Settings mid-update loses
 * nothing, and the next queued update still starts. The event listener is
 * attached once and never detached. Updates run one at a time, as Rust allows
 * only one; after each, bridge health and the check are re-read so the row
 * shows the new version. A successful run's output is dropped; a failed one
 * stays for the Output dialog.
 */
let state: UpdatesState = { updates: null, checking: false, queue: [], current: null, runs: {} };
const subscribers = new Set<(s: UpdatesState) => void>();
let listening: Promise<unknown> | null = null;

function set(patch: Partial<UpdatesState> | ((s: UpdatesState) => Partial<UpdatesState>)) {
  state = { ...state, ...(typeof patch === "function" ? patch(state) : patch) };
  for (const fn of subscribers) fn(state);
  pump();
}

function patchRun(provider: Provider, patch: Partial<AgentUpdateRun>) {
  set((s) => {
    const run = s.runs[provider];
    return run ? { runs: { ...s.runs, [provider]: { ...run, ...patch } } } : {};
  });
}

async function load(recheck: boolean) {
  set({ checking: true });
  try {
    set({ updates: await harnessUpdates(recheck), checking: false });
  } catch {
    // A failed check offers no updates rather than a wrong one.
    set({ updates: [], checking: false });
  }
}

async function finish(provider: Provider, ok: boolean) {
  try {
    await refreshBridgeHealth();
    await load(false);
  } finally {
    set((s) => {
      const runs = { ...s.runs };
      if (ok) delete runs[provider];
      return { runs, current: s.current === provider ? null : s.current };
    });
  }
}

function listenOnce(): Promise<unknown> {
  listening ??= listen<UpdateLine>(UPDATE_EVENT, (e) => {
    const ev = e.payload;
    const run = state.runs[ev.provider];
    if (!run) return;
    if (ev.done) {
      patchRun(ev.provider, { result: { ok: ev.ok ?? false, status: ev.status ?? "finished" } });
      void finish(ev.provider, ev.ok ?? false);
    } else if (ev.line !== null) {
      patchRun(ev.provider, { lines: [...run.lines, ev.line] });
    }
  });
  return listening;
}

/** Start the next queued update once nothing is running. */
function pump() {
  if (state.current !== null || state.queue.length === 0) return;
  const [next, ...queue] = state.queue;
  const command = state.updates?.find((u) => u.provider === next)?.command ?? "";
  set((s) => ({
    queue,
    current: next,
    runs: { ...s.runs, [next]: { command, lines: [], result: null, error: null } },
  }));
  // The listener must be live before the child talks, or the first lines go.
  void listenOnce()
    .then(() => harnessUpdateRun(next))
    .catch((e) => {
      patchRun(next, { error: String(e) });
      set((s) => ({ current: s.current === next ? null : s.current }));
    });
}

function enqueue(providers: Provider[]) {
  set((s) => {
    const fresh = providers.filter((p) => p !== s.current && !s.queue.includes(p));
    // A retry drops the last failure's line from the row.
    const runs = { ...s.runs };
    for (const p of fresh) delete runs[p];
    return { queue: [...s.queue, ...fresh], runs };
  });
}

function stateOf(provider: Provider): "running" | "queued" | null {
  return state.current === provider ? "running" : state.queue.includes(provider) ? "queued" : null;
}

/** The shared update state; the check loads on every visit (Rust caches the
 *  registry answers) unless one is already in flight. */
export function useAgentUpdates() {
  const [s, setS] = useState(state);

  useEffect(() => {
    subscribers.add(setS);
    setS(state);
    void listenOnce();
    if (!state.checking) void load(false);
    return () => {
      subscribers.delete(setS);
    };
  }, []);

  const recheck = useCallback(() => load(true), []);

  return { updates: s.updates, checking: s.checking, recheck, enqueue, runs: s.runs, stateOf };
}

/** A failed (or running) update's literal command and full output. */
export function UpdateOutputDialog({
  label,
  run,
  onClose,
}: {
  label: string;
  run: AgentUpdateRun;
  onClose: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const log = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // Follow the tail.
    const el = log.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [run.lines, run.result]);

  const copy = async () => {
    if (await copyText(run.command)) {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    }
  };

  const description = run.error
    ? run.error
    : run.result
      ? run.result.ok
        ? `${label} is updated.`
        : `The command ${run.result.status}. Run it in a terminal to see what it wanted.`
      : "Running. The output is live.";

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Update {label}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-2">
          <div className="rounded-lg border border-border-subtle p-2.5">
            <div className="flex items-center justify-between gap-3">
              <div className="text-xs text-muted-foreground">Command</div>
              <Button variant="ghost" size="xs" onClick={() => void copy()}>
                {copied ? <CheckCircle size={12} /> : <Copy size={12} />}
                {copied ? "Copied" : "Copy"}
              </Button>
            </div>
            <CodeText className="mt-2 text-muted-foreground">{run.command}</CodeText>
          </div>
          <div
            ref={log}
            className="max-h-64 overflow-y-auto rounded-lg border border-border-subtle bg-surface p-2.5"
          >
            {run.lines.length === 0 && !run.result ? (
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                {!run.error && <CircleNotch size={12} className="animate-spin" />}
                <span>{run.error ? "No output." : "Starting…"}</span>
              </div>
            ) : run.lines.length === 0 ? (
              <div className="text-xs text-muted-foreground">No output.</div>
            ) : (
              <AnsiText lines={run.lines} />
            )}
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" size="sm" onClick={onClose}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
