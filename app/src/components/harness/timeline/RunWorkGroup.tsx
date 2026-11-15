import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { CaretRight } from "@phosphor-icons/react";
import { EXPANDABLE, onExpand } from "@/lib/ui/expandable";
import { cn } from "@/lib/utils";
import { awaitsApproval } from "@/lib/harness";
import { chooseFold, foldChoice } from "@/stores/chat/runFolds";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { Bundle, Item } from "./ItemRows";
import { WAITING, Working } from "./Working";
import { foldFindText, foldLabel, foldMatches, foldOpens, foldRows, type WorkFold } from "./rows";

/**
 * One run's work — earlier replies, reasoning and tools — as one disclosure
 * above its answer. Open while the run works and after a stop or failure;
 * closed once it completes, unless the student chose otherwise
 * (`runFolds.ts`). Its rows read as one flat list under a ruled header. A
 * find that matches a folded reply opens it until the query stops matching. Widget fences are drawn outside (`ArtifactRow`); here
 * a folded reply shows where they went.
 */
export const RunWorkGroup = memo(
  function RunWorkGroup({ fold }: { fold: WorkFold }) {
    const live = fold.state === "running";
    // A request of this run waits on the student: the run is not working.
    const waiting = useHarnessStore((s) => live && awaitsApproval(s.requests[fold.thread], fold.run));
    const [chosen, setChosen] = useState(() => foldChoice(fold.thread, fold.run));
    const [found, setFound] = useState(false);
    const open = found || (chosen ?? foldOpens(fold.state));
    const ref = useRef<HTMLDivElement>(null);
    const text = useMemo(() => foldFindText(fold.items), [fold.items]);
    const latest = useRef({ text, found });
    useLayoutEffect(() => {
      latest.current = { text, found };
    });

    useEffect(() => {
      const el = ref.current;
      if (!el) return;
      return onExpand(el, ({ sync, query }) => {
        if (query === undefined) return;
        const hit = foldMatches(latest.current.text, query);
        if (hit === latest.current.found) return;
        if (sync) flushSync(() => setFound(hit));
        else setFound(hit);
      });
    }, []);

    const rows = useMemo(() => (open ? foldRows(fold.items, live) : []), [open, fold.items, live]);
    const toggle = () => {
      setFound(false);
      setChosen(!open);
      chooseFold(fold.thread, fold.run, !open);
    };
    return (
      <div ref={ref} {...{ [EXPANDABLE]: "" }} className="min-w-0">
        {/* A hairline under the header, open or closed, sets the work apart
            from the answer below it. */}
        <div className="border-b border-border/60 pb-1">
          <button
            type="button"
            aria-expanded={open}
            onClick={toggle}
            className={cn(
              "group/row flex w-full min-w-0 cursor-pointer items-center gap-1.5 rounded-md px-2 py-0.5 text-left text-xs leading-5 transition-colors hover:text-foreground",
              open ? "text-foreground" : "text-muted-foreground",
            )}
          >
            {/* Steady while the run works: only its live step shimmers. */}
            <span className="min-w-0 truncate tabular-nums">
              {waiting ? WAITING : live ? <Working since={fold.startedMs} /> : foldLabel(fold)}
            </span>
            <CaretRight
              size={11}
              className={cn(
                "shrink-0 transition-[opacity,transform] will-change-[opacity,transform] duration-150",
                open ? "rotate-90 opacity-60" : "opacity-60",
              )}
            />
          </button>
        </div>
        {open && (
          <div className="flex flex-col gap-1 pt-1.5">
            {rows.map((r) =>
              r.kind === "bundle" ? (
                <Bundle key={r.id} items={r.items} dim={false} />
              ) : r.kind === "item" ? (
                <Item key={r.item.id} item={r.item} dim={false} folded />
              ) : null,
            )}
          </div>
        )}
      </div>
    );
  },
  (prev, next) =>
    prev.fold.id === next.fold.id &&
    prev.fold.state === next.fold.state &&
    prev.fold.startedMs === next.fold.startedMs &&
    prev.fold.endedMs === next.fold.endedMs &&
    prev.fold.items.length === next.fold.items.length &&
    prev.fold.items.every((item, i) => item === next.fold.items[i]),
);
