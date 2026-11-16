import type { HarnessThread } from "@/lib/harness";
import { cn } from "@/lib/utils";
import { InfoRow } from "./InfoRow";
import { openTarget } from "./openTarget";
import { useWorkKinds, type WorkKind, type WorkRow } from "./work";

const TONE: Record<NonNullable<WorkRow["tone"]>, string> = {
  muted: "text-muted-foreground",
  busy: "text-brand",
  ok: "text-success",
  bad: "text-destructive",
};

/** One component per kind, so each kind's hook keeps its own order however
 *  kinds come and go. */
function KindRows({ kind, thread, onOpened }: { kind: WorkKind; thread: HarnessThread; onOpened?: () => void }) {
  const rows = kind.useRows(thread);
  return rows.map((r) => (
    <InfoRow
      key={r.key}
      icon={<r.icon size={15} className="text-muted-foreground" />}
      label={r.title}
      trailing={r.status && <span className={TONE[r.tone ?? "muted"]}>{r.status}</span>}
      onOpen={(newTab) => {
        openTarget({ kind: "route", href: r.href }, newTab);
        onOpened?.();
      }}
    />
  ));
}

/** What the thread is making, from the registered Work kinds (`work.ts`);
 *  nothing is drawn while no kind has a row for this thread. */
export function WorkRows({ thread, onOpened }: { thread: HarnessThread; onOpened?: () => void }) {
  const kinds = useWorkKinds();
  if (kinds.length === 0) return null;
  return (
    // Hidden by CSS rather than state: only the kinds' hooks know their rows.
    <div className={cn("hidden flex-col has-[[data-info-row]]:flex")}>
      {kinds.map((k) => (
        <KindRows key={k.id} kind={k} thread={thread} onOpened={onOpened} />
      ))}
    </div>
  );
}
