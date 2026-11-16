import { memo, useMemo } from "react";
import { ShieldCheck, ShieldSlash, ShieldWarning } from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import { requestDetail, requestState, type HarnessItem, type HarnessRequest, type RequestState } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { QuestionView } from "./QuestionRow";
import { RequestBody } from "./RequestBody";
import { RequestNotes, SettledRow } from "./SettledRow";

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
 * what was asked. A question is `QuestionView`'s.
 */
export const RequestRow = memo(function RequestRow({ item }: { item: HarnessItem }) {
  const id = Number(item.ref_id);
  const request = useHarnessStore((s) => s.requests[item.thread_id]?.[id]);
  if (!request) return null;
  if (request.kind === "question") return <QuestionView request={request} />;
  return <RequestView request={request} />;
});

/** A request as its row draws it, given the request. */
export function RequestView({ request }: { request: HarnessRequest }) {
  const detail = useMemo(() => requestDetail(request), [request]);
  const state = requestState(request);

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
          {/* The run's fold or the live tail says "Waiting for approval". */}
          <RequestNotes notes={state.notes} warn={state.warn} />
        </div>
      </div>
    );
  }

  return (
    <SettledRow
      icon={SETTLED_ICON[state.kind]}
      state={state.kind}
      label={state.label}
      subject={detail.subject}
      title={detail.title}
      notes={state.notes}
      warn={state.warn}
    >
      <RequestBody detail={detail} />
    </SettledRow>
  );
}
