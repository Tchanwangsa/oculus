import { useEffect, useRef, useState, type KeyboardEvent } from "react";

import { DRAG_SURFACE, useStripReorder } from "@/hooks/gestures/usePointerDrag";
import type { JobChain, JobSelection } from "@/lib/db";
import type { PickerProvider, Provider } from "@/lib/harness";
import { cn } from "@/lib/utils";
import { ChainEntryRow } from "./ChainEntryRow";
import { modelLabel, moveEntry } from "./chain";

/** A chain never names an agent and model twice (`withEntry`), so the pair
 *  is a stable key for the drag. */
const entryKey = (e: JobSelection) => `${e.provider}/${e.model}`;

/**
 * A job's models as one bordered group, in run order. A row drags to a new
 * place (`useStripReorder`, as Settings → Transcription's engines); its
 * handle also moves it with the arrow keys.
 */
export function ChainList({
  chain,
  providers,
  onChange,
  onView,
  onPick,
  onRemove,
}: {
  chain: JobChain;
  providers: PickerProvider[];
  onChange: (edit: (chain: JobChain) => JobChain) => void;
  onView: (provider: Provider) => void;
  onPick: (index: number, pick: JobSelection) => void;
  onRemove: (index: number) => void;
}) {
  const keys = chain.map(entryKey);
  const [announcement, setAnnouncement] = useState("");
  const handles = useRef(new Map<string, HTMLButtonElement>());
  // Rows are keyed by place, so a move hands the focused handle's node to
  // another entry; focus follows the entry instead.
  const refocus = useRef<string | null>(null);
  useEffect(() => {
    if (!refocus.current) return;
    handles.current.get(refocus.current)?.focus();
    refocus.current = null;
  }, [chain]);

  const strip = useStripReorder({
    keys,
    axis: "y",
    // Equal-height rows: a clamped centre never passes the end ones.
    swapOn: "edge",
    onDrop: ({ key, target }) =>
      onChange((c) => {
        const from = c.findIndex((e) => entryKey(e) === key);
        if (from === -1) return c;
        const next = [...c];
        next.splice(target, 0, next.splice(from, 1)[0]);
        return next;
      }),
  });

  const onHandleKey = (e: KeyboardEvent<HTMLButtonElement>, i: number) => {
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    e.preventDefault();
    const by = e.key === "ArrowUp" ? -1 : 1;
    const to = i + by;
    if (to < 0 || to >= chain.length) return;
    refocus.current = keys[i];
    onChange((c) => moveEntry(c, i, by));
    const { provider, model } = chain[i];
    setAnnouncement(
      `${modelLabel(providers, provider, model)} moved to position ${to + 1} of ${chain.length}`,
    );
  };

  return (
    <>
      <ol
        aria-label="Models, in the order they are tried"
        className={cn(
          "divide-y divide-border-subtle rounded-lg border border-border",
          DRAG_SURFACE,
        )}
      >
        {/* Keyed by place, not model: a row whose model changes keeps its
            picker mounted, so the menu stays open for the level. */}
        {chain.map((entry, i) => {
          const key = keys[i];
          return (
            <ChainEntryRow
              key={i}
              position={i + 1}
              count={chain.length}
              entry={entry}
              providers={providers}
              rowRef={strip.itemRef(key)}
              handleRef={(node) => {
                if (node) handles.current.set(key, node);
                else handles.current.delete(key);
              }}
              style={strip.styleFor(key, i)}
              grabbed={strip.drag?.key === key}
              settling={!!strip.drag}
              onPointerDown={(e) => strip.onPointerDown(e, key)}
              // Swallow the click a drag's release raises, before a button hears it.
              onClickCapture={(e) => {
                if (strip.didDrag()) e.stopPropagation();
              }}
              onHandleKey={(e) => onHandleKey(e, i)}
              onPick={(pick) => onPick(i, pick)}
              onView={onView}
              onRemove={() => onRemove(i)}
            />
          );
        })}
      </ol>
      <p className="sr-only" aria-live="polite">
        {announcement}
      </p>
    </>
  );
}
