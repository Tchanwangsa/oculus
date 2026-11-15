import { memo, useMemo } from "react";
import { ShieldCheck, ShieldSlash, ShieldWarning } from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import { requestDetail, requestState, type HarnessItem, type HarnessRequest, type RequestState } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { cn } from "@/lib/utils";
import { RowShell } from "../WorkRow";
import { RequestBody } from "./RequestBody";

const SETTLED_ICON: Record<Exclude<RequestState["kind"], "waiting">, Icon> = {
  approved: ShieldCheck,
  declined: ShieldSlash,
  expired: ShieldSlash,
  cancelled: ShieldSlash,
};

/**
 * A `request` row: it holds the request's id, and the request itself is
 * read from the store (`harnessStore.requests`), so an answer, a delivery
 * or an ending redraws only this row. While it waits it is a card with
 * everything the agent asked; the answer is given by the composer's bar
 * (`RequestBar`). Settled, it is one quiet line saying how, opening to
 * what was asked.
 */
export const RequestRow = memo(function RequestRow({ item }: { item: HarnessItem }) {
  const id = Number(item.ref_id);
  const request = useHarnessStore((s) => s.requests[item.thread_id]?.[id]);
  if (!request) return null;
  return <RequestView request={request} />;
});

/** A request as its row draws it, given the request. */
export function RequestView({ request }: { request: HarnessRequest }) {
  const detail = useMemo(() => requestDetail(request), [request]);
  const state = requestState(request);
  const notes = state.notes.length > 0 && (
    <div className="flex flex-col gap-0.5">
      {state.notes.map((note) => (
        <p
          key={note}
          className={cn("break-words text-[11px] leading-relaxed", state.warn ? "text-warning" : "text-muted-foreground")}
        >
          {note}
        </p>
      ))}
    </div>
  );

  if (state.kind === "waiting") {
    return (
      <div
        data-request={request.id}
        data-request-state="waiting"
        className="flex items-start gap-2.5 rounded-xl border border-border bg-card px-3 py-2.5"
      >
        <ShieldWarning size={14} weight="fill" className="mt-0.5 shrink-0 text-warning" aria-hidden />
        <div className="flex min-w-0 flex-1 flex-col gap-1.5">
          <div className="text-xs text-foreground">{detail.title}</div>
          <RequestBody detail={detail} />
          {/* Steady, never a shimmer: nothing works until the student answers. */}
          <p className="text-[11px] text-muted-foreground">{state.label}</p>
          {notes}
        </div>
      </div>
    );
  }

  return (
    <div data-request={request.id} data-request-state={state.kind} className="min-w-0">
      <RowShell
        icon={SETTLED_ICON[state.kind]}
        title={state.label}
        em={detail.subject}
        expandable
      >
        <div className="flex flex-col gap-1.5 pl-5">
          <p className="text-[11px] text-muted-foreground">{detail.title}</p>
          <RequestBody detail={detail} />
        </div>
      </RowShell>
      {notes && <div className="pl-[30px] pr-2">{notes}</div>}
    </div>
  );
}
