import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { CaretRight, Plus, SquaresFour } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Switch } from "@/components/ui/switch";
import {
  addAutomation,
  getAutomations,
  setAutomationEnabled,
  type DbAutomation,
} from "@/lib/db";
import {
  templatesByGroup,
  type AutomationTemplate,
} from "@/lib/automationTemplates";
import {
  describeGraph,
  isAutomationDue,
  nextFireMs,
  parseGraph,
  serializeGraph,
  triggerNodes,
} from "@/lib/automations";
import { SPEC_BY_KIND } from "@/components/automations/catalog";
import { fmtAgo, fmtClock, sqliteUtcToMs } from "@/lib/format";
import { useSyncStore } from "@/stores/syncStore";

/** A new automation opens on a trigger and nothing else — the canvas is where
 *  it gets its shape, so the list does not ask any questions first. */
const STARTER = serializeGraph({
  nodes: [
    {
      id: "t1",
      kind: "trigger.schedule",
      config: { scheduleKind: "daily", timeOfDay: "09:00", intervalMinutes: 360, days: [1, 2, 3, 4, 5] },
      position: { x: 80, y: 100 },
    },
  ],
  links: [],
});

function nextRunLabel(a: DbAutomation): string {
  if (!a.enabled) return "Paused";
  const triggers = triggerNodes(parseGraph(a.graph));
  if (triggers.length === 0) return "No trigger";
  if (isAutomationDue(a)) return "Due — runs shortly";
  const ms = nextFireMs(a);
  if (ms) return `Next at ${fmtClock(ms, true)}`;
  return triggers.some((t) => t.kind === "trigger.event") ? "On its event" : "—";
}

function Row({ a, onChanged }: { a: DbAutomation; onChanged: () => void }) {
  const navigate = useNavigate();
  const graph = parseGraph(a.graph);
  const kinds = [...new Set(graph.nodes.map((n) => n.kind))];

  return (
    <div className="flex items-center gap-3 border-b border-border-subtle px-4 py-2.5 last:border-0 hover:bg-accent/50">
      <button
        type="button"
        onClick={() => navigate(`/automations/${a.id}`)}
        className="flex min-w-0 flex-1 items-center gap-2.5 text-left"
      >
        <span className="flex shrink-0 items-center gap-1">
          {kinds.slice(0, 4).map((k) => {
            const Icon = SPEC_BY_KIND[k]?.icon;
            return Icon ? (
              <Icon key={k} size={12} className="text-muted-foreground" />
            ) : null;
          })}
        </span>
        <span className="truncate text-xs text-foreground">{a.name}</span>
        <span className="truncate text-[11px] text-muted-foreground">
          {describeGraph(graph)}
        </span>
      </button>

      <span className="w-[150px] shrink-0 truncate text-[11px] tabular-nums text-muted-foreground">
        {nextRunLabel(a)}
      </span>
      <span className="w-[110px] shrink-0 truncate text-[11px] tabular-nums text-muted-foreground">
        {a.last_fired_at ? fmtAgo(sqliteUtcToMs(a.last_fired_at) ?? undefined) : "Never"}
      </span>
      <Switch
        checked={a.enabled}
        onCheckedChange={async (v) => {
          await setAutomationEnabled(a.id, v);
          onChanged();
        }}
        aria-label="Enable automation"
      />
      <CaretRight size={12} className="shrink-0 text-muted-foreground/50" />
    </div>
  );
}

/** The library. Name and one line each — the graph itself is the real
 *  explanation, and it is one click away. */
function TemplatePicker({
  open,
  onOpenChange,
  onPick,
}: {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  onPick: (t: AutomationTemplate) => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle className="text-sm">Start from a template</DialogTitle>
          <DialogDescription className="text-xs">
            Each one opens as a graph you can rewire. They arrive paused, so nothing
            runs until you have read it.
          </DialogDescription>
        </DialogHeader>

        <div className="-mx-1 max-h-[55vh] overflow-y-auto px-1">
          {templatesByGroup().map((group) => (
            <div key={group.label} className="mb-3 last:mb-0">
              <p className="sticky top-0 bg-background py-1.5 text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
                {group.label}
              </p>
              <div className="divide-y divide-border-subtle">
                {group.templates.map((t) => (
                  <button
                    key={t.id}
                    type="button"
                    onClick={() => onPick(t)}
                    className="flex w-full items-start gap-3 rounded-md px-2 py-2.5 text-left hover:bg-accent/50"
                  >
                    <span className="min-w-0 flex-1">
                      <span className="block text-xs text-foreground">{t.name}</span>
                      <span className="mt-0.5 block text-[11px] leading-relaxed text-muted-foreground">
                        {t.description}
                      </span>
                    </span>
                    <CaretRight
                      size={12}
                      className="mt-1 shrink-0 text-muted-foreground/50"
                    />
                  </button>
                ))}
              </div>
            </div>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}

export default function AutomationsPage() {
  const navigate = useNavigate();
  const [automations, setAutomations] = useState<DbAutomation[]>([]);
  // Bumped by a timer so "Next at …" / "Due" labels stay current.
  const [, setNowTick] = useState(0);
  const [picking, setPicking] = useState(false);
  const completedAt = useSyncStore((s) => s.completedAt);

  const reload = useCallback(() => {
    getAutomations().then(setAutomations).catch(console.error);
  }, []);

  useEffect(() => {
    reload();
  }, [reload, completedAt]);

  useEffect(() => {
    const t = setInterval(() => setNowTick((n) => n + 1), 30_000);
    return () => clearInterval(t);
  }, []);

  const create = async () => {
    const id = await addAutomation("Untitled automation", STARTER);
    navigate(`/automations/${id}`);
  };

  /** Templates land paused. Most of them spend tokens on every run and several
   *  want a subject or a model picked first, so switching one on is a decision
   *  taken after reading the graph — not a side effect of opening it. */
  const createFromTemplate = async (t: AutomationTemplate) => {
    const id = await addAutomation(t.name, serializeGraph(t.graph));
    await setAutomationEnabled(id, false);
    setPicking(false);
    navigate(`/automations/${id}`);
  };

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex shrink-0 items-end justify-between gap-4 px-6 pt-5 pb-3">
        <div>
          <h1 className="text-[13px] font-medium text-foreground">Automations</h1>
          <p className="mt-1 text-xs text-muted-foreground">
            Graphs that run on a schedule or after an event. They fire while Oculus is
            open; a daily run missed overnight fires at the next launch.
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Button size="sm" variant="outline" className="h-7" onClick={create}>
            <Plus size={13} /> New automation
          </Button>
          <Button size="sm" className="h-7" onClick={() => setPicking(true)}>
            <SquaresFour size={13} /> Start from a template
          </Button>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-5">
        <div className="overflow-hidden rounded-lg border border-border-subtle">
          {automations.length === 0 ? (
            <div className="flex flex-col items-center gap-3 px-4 py-10">
              <p className="text-xs text-muted-foreground">No automations yet.</p>
              <Button size="sm" className="h-7" onClick={() => setPicking(true)}>
                <SquaresFour size={13} /> Start from a template
              </Button>
            </div>
          ) : (
            automations.map((a) => <Row key={a.id} a={a} onChanged={reload} />)
          )}
        </div>
      </div>

      <TemplatePicker
        open={picking}
        onOpenChange={setPicking}
        onPick={createFromTemplate}
      />
    </div>
  );
}
