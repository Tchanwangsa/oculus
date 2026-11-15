import { memo, useMemo, useState } from "react";
import { CaretRight, ShieldCheck, ShieldSlash, ShieldWarning } from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import { requestDetail, requestState, type HarnessItem, type HarnessRequest, type RequestState } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { cn } from "@/lib/utils";
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
          {/* The run's fold or the live tail says "Waiting for approval". */}
          {notes}
        </div>
      </div>
    );
  }

  return <SettledRow kind={state.kind} label={state.label} subject={detail.subject} title={detail.title} notes={notes}>
    <RequestBody detail={detail} />
  </SettledRow>;
}

/** A settled request as one work-row line (`RowShell`'s look): how it was
 *  settled and what it was about, its notes always shown under it, the
 *  whole request when opened. */
function SettledRow({
  kind,
  label,
  subject,
  title,
  notes,
  children,
}: {
  kind: Exclude<RequestState["kind"], "waiting">;
  label: string;
  subject: string;
  title: string;
  notes: React.ReactNode;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const IconC = SETTLED_ICON[kind];
  return (
    <div data-request-state={kind} className="min-w-0">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        className={cn(
          "group/row flex w-full min-w-0 cursor-pointer items-center gap-1.5 rounded-md px-2 py-0.5 text-left text-xs leading-5 transition-colors hover:text-foreground",
          open ? "text-foreground" : "text-muted-foreground",
        )}
      >
        <IconC size={14} className="shrink-0" aria-hidden />
        <span className="shrink-0">{label}</span>
        <span className="min-w-0 truncate font-medium text-foreground/80" title={subject}>{subject}</span>
        <CaretRight
          size={11}
          className={cn(
            "ml-auto shrink-0 transition-[opacity,transform] will-change-[opacity,transform] duration-150",
            open ? "rotate-90 opacity-60" : "opacity-0 group-hover/row:opacity-60",
          )}
        />
      </button>
      {notes && <div className="pl-[30px] pr-2">{notes}</div>}
      {open && (
        <div className="flex flex-col gap-1.5 pb-1 pl-[30px] pr-2 pt-0.5">
          <p className="text-[11px] text-muted-foreground">{title}</p>
          {children}
        </div>
      )}
    </div>
  );
}
