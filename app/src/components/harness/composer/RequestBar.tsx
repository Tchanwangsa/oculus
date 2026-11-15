import { useEffect, useMemo, useState } from "react";
import { CaretDown, ShieldWarning } from "@phosphor-icons/react";
import { useShallow } from "zustand/react/shallow";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  harnessRespond,
  requestActions,
  requestDetail,
  waitingRequests,
  type HarnessRequest,
  type RequestOption,
} from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { CodeText } from "@/components/markdown/MdComponents";
import { useRunCommand } from "./useRunCommand";

/**
 * The answer to an agent's request, beside the composer: what is asked in a
 * line, then Decline and Approve, and a session grant under More with its
 * scope in Rust's words. It answers the oldest waiting request and says how
 * many wait. One click answers once; Rust's refusal is shown under it, never
 * in a toast. The request's own row in the timeline holds the detail.
 */
export function RequestBar({ threadId }: { threadId: number | null }) {
  if (threadId == null) return null;
  return <Bar threadId={threadId} />;
}

function Bar({ threadId }: { threadId: number }) {
  const waiting = useHarnessStore(useShallow((s) => waitingRequests(s.requests[threadId])));
  // Answered here and saved by Rust, until its `request_updated` replaces
  // the object (a failed delivery comes back pending, to answer again).
  const [answered, setAnswered] = useState<readonly HarnessRequest[]>([]);
  const open = waiting.filter((r) => !answered.includes(r));
  const request = open[0];
  const answer = useRunCommand();
  const { clear } = answer;
  // A refusal belongs to the request it was about.
  useEffect(() => clear(), [request?.id, clear]);
  if (!request) return null;

  const respond = (option: RequestOption) =>
    void answer.run(async () => {
      await harnessRespond(threadId, request.id, option.id);
      setAnswered((s) => [...s.filter((r) => waiting.includes(r)), request]);
    });

  return (
    <RequestBarView
      request={request}
      count={open.length}
      busy={answer.pending}
      error={answer.error}
      onRespond={respond}
    />
  );
}

/** The bar as drawn, given the request (the WebKit checks render it alone). */
export function RequestBarView({
  request,
  count,
  busy,
  error,
  onRespond,
}: {
  request: HarnessRequest;
  /** How many requests wait, this one included. */
  count: number;
  busy: boolean;
  error: string | null;
  onRespond: (option: RequestOption) => void;
}) {
  const detail = useMemo(() => requestDetail(request), [request]);
  const actions = requestActions(request);
  const [moreOpen, setMoreOpen] = useState(false);
  return (
    <div
      data-request-bar={request.id}
      className="flex flex-wrap items-center gap-x-2.5 gap-y-2 rounded-xl border border-border bg-card px-3 py-2.5"
    >
      <div className="flex min-w-0 flex-1 basis-52 items-start gap-2.5">
        <ShieldWarning size={14} weight="fill" className="mt-0.5 shrink-0 text-warning" aria-hidden />
        <div className="min-w-0 flex-1">
          <div className="text-xs text-foreground">
            {detail.title}
            {count > 1 && <span className="tabular-nums text-muted-foreground"> · 1 of {count}</span>}
          </div>
          {detail.command ? (
            <CodeText title={detail.subject} className="truncate whitespace-pre text-[11px] text-muted-foreground">
              {detail.subject}
            </CodeText>
          ) : (
            <p title={detail.subject} className="truncate text-[11.5px] text-muted-foreground">
              {detail.subject}
            </p>
          )}
          {actions.approve?.scope && (
            <p className="text-[11px] text-muted-foreground">Approve: {actions.approve.scope}</p>
          )}
        </div>
      </div>
      <div className="ml-auto flex shrink-0 items-center gap-1.5">
        {actions.more.length > 0 && (
          <Popover open={moreOpen} onOpenChange={setMoreOpen}>
            <PopoverTrigger asChild>
              <Button size="xs" variant="ghost" disabled={busy} className="text-muted-foreground will-change-[opacity]">
                More
                <CaretDown aria-hidden />
              </Button>
            </PopoverTrigger>
            <PopoverContent align="end" collisionPadding={8} className="w-72 max-w-[calc(100vw-16px)] p-1">
              {actions.more.map((option) => (
                <button
                  key={option.id}
                  type="button"
                  disabled={busy}
                  onClick={() => {
                    setMoreOpen(false);
                    onRespond(option);
                  }}
                  className="flex w-full cursor-default flex-col items-start gap-0.5 rounded-md px-2 py-1.5 text-left outline-none transition-colors hover:bg-accent focus-visible:bg-accent"
                >
                  <span className="text-xs text-foreground">{option.label}</span>
                  {option.scope && (
                    <span className="text-[11px] leading-relaxed text-muted-foreground">{option.scope}</span>
                  )}
                </button>
              ))}
            </PopoverContent>
          </Popover>
        )}
        {actions.decline && (
          <Button
            size="xs"
            variant="outline"
            disabled={busy}
            title={actions.decline.label}
            onClick={() => onRespond(actions.decline!)}
            className="will-change-[opacity]"
          >
            Decline
          </Button>
        )}
        {actions.approve && (
          <Button
            size="xs"
            disabled={busy}
            title={[actions.approve.label, actions.approve.scope].filter(Boolean).join(" — ")}
            onClick={() => onRespond(actions.approve!)}
            className="will-change-[opacity]"
          >
            Approve
          </Button>
        )}
      </div>
      {error && (
        <p role="alert" className="basis-full break-words text-[11px] leading-relaxed text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}
