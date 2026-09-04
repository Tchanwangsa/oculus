import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft, Play, Trash } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import AutomationCanvas from "@/components/automations/AutomationCanvas";
import {
  deleteAutomation,
  getAutomations,
  setAutomationEnabled,
  updateAutomation,
  type DbAutomation,
} from "@/lib/db";
import {
  parseGraph,
  runAutomationNow,
  serializeGraph,
  type AutomationGraph,
} from "@/lib/automations";

const SAVE_DEBOUNCE_MS = 400;

/**
 * One automation, open on the canvas.
 *
 * The graph lives in React state and is written back on a short debounce —
 * dragging a node emits a change per frame, and each of those would otherwise
 * be a SQLite write. The pending write is flushed on unmount so leaving the
 * page never loses the last edit.
 */
export default function AutomationEditorPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const automationId = Number(id);

  const [row, setRow] = useState<DbAutomation | null>(null);
  const [graph, setGraph] = useState<AutomationGraph | null>(null);
  const [name, setName] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  // Debounced graph write. The ref pair lets the unmount flush see the latest
  // graph without making the effect depend on it.
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pending = useRef<string | null>(null);

  const flush = useCallback(() => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    const g = pending.current;
    pending.current = null;
    if (g != null) updateAutomation(automationId, { graph: g }).catch(console.error);
  }, [automationId]);

  useEffect(() => {
    getAutomations()
      .then((all) => {
        const found = all.find((a) => a.id === automationId) ?? null;
        setRow(found);
        setGraph(found ? parseGraph(found.graph) : null);
        setName(found?.name ?? "");
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [automationId]);

  useEffect(() => flush, [flush]);

  const onGraphChange = (g: AutomationGraph) => {
    setGraph(g);
    pending.current = serializeGraph(g);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(flush, SAVE_DEBOUNCE_MS);
  };

  const runNow = async () => {
    if (!row || !graph) return;
    setStatus("Running…");
    try {
      await runAutomationNow({ ...row, graph: serializeGraph(graph) }, {});
      setStatus("Ran once just now");
    } catch (e) {
      setStatus(String(e));
    }
  };

  if (!row || !graph) {
    return (
      <div className="flex h-full items-center justify-center text-xs text-muted-foreground">
        {loading ? "Loading…" : "Automation not found."}
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex shrink-0 items-center gap-3 border-b border-border-subtle px-4 py-2.5">
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Back to automations"
          onClick={() => navigate("/automations")}
          className="text-muted-foreground"
        >
          <ArrowLeft size={14} />
        </Button>

        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={() => {
            if (name.trim() && name !== row.name) {
              updateAutomation(row.id, { name: name.trim() }).catch(console.error);
              setRow({ ...row, name: name.trim() });
            } else {
              setName(row.name);
            }
          }}
          className="min-w-0 flex-1 bg-transparent text-[13px] font-medium text-foreground outline-none"
          aria-label="Automation name"
        />

        {status && (
          <span className="max-w-[280px] truncate text-[11px] text-muted-foreground">{status}</span>
        )}

        <Button variant="ghost" size="sm" className="h-7" onClick={runNow}>
          <Play size={12} /> Run now
        </Button>

        <Switch
          checked={row.enabled}
          onCheckedChange={async (v) => {
            await setAutomationEnabled(row.id, v);
            setRow({ ...row, enabled: v });
          }}
          aria-label="Enable automation"
        />

        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Delete automation"
          onClick={async () => {
            await deleteAutomation(row.id);
            navigate("/automations");
          }}
          className="text-muted-foreground/60 hover:text-destructive"
        >
          <Trash size={13} />
        </Button>
      </div>

      <AutomationCanvas graph={graph} onChange={onGraphChange} />
    </div>
  );
}
