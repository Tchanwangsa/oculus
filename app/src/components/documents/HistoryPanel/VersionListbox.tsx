import { Fragment, useEffect, useId, useMemo, useRef, type KeyboardEvent } from "react";

import { useNow } from "@/hooks/ui/useNow";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import type { DocumentVersion } from "@/lib/notes/documentVersions";

import { LabelField } from "./LabelField";
import { dayHeading, groupVersionsByDay, lengthDelta } from "./listModel";
import { VersionRow } from "./VersionRow";

/** Which row an arrow, Home or End lands on, clamped to the list. */
function step(key: string, index: number, count: number): number | null {
  switch (key) {
    case "ArrowDown":
      return Math.min(index + 1, count - 1);
    case "ArrowUp":
      return Math.max(index - 1, 0);
    case "Home":
      return 0;
    case "End":
      return count - 1;
    default:
      return null;
  }
}

/**
 * The versions as a listbox under day headings. ↑/↓/Home/End move the
 * selection and focus with it (a roving tabindex); the selected row takes
 * focus when the list first shows one, so the keys and Esc work at once.
 */
export function VersionListbox({
  versions,
  selectedId,
  onSelect,
  onOpen,
  renaming,
  onRelabel,
  onCancelRename,
}: {
  versions: DocumentVersion[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  /** A row clicked (or Enter on it), after `onSelect`: the narrow tier shows it. */
  onOpen?: (id: number) => void;
  /** The selected checkpoint's label is being edited in its row. */
  renaming: boolean;
  onRelabel: (v: DocumentVersion, label: string) => void;
  onCancelRename: () => void;
}) {
  const now = useNow();
  const uid = useId();
  const days = useMemo(() => groupVersionsByDay(versions), [versions]);
  const ref = useRef<HTMLDivElement>(null);
  useScrollFade(ref, "y", versions);

  const option = (id: number) =>
    ref.current?.querySelector<HTMLElement>(`[role="option"][data-version-id="${id}"]`) ?? null;

  const focused = useRef(false);
  useEffect(() => {
    if (focused.current || selectedId == null) return;
    const el = option(selectedId);
    if (!el) return;
    focused.current = true;
    el.focus({ preventScroll: true });
    el.scrollIntoView({ block: "nearest" });
  }, [selectedId]);
  // The rename field takes the row's place; focus goes back to the row after.
  const wasRenaming = useRef(renaming);
  useEffect(() => {
    if (wasRenaming.current && !renaming && selectedId != null) option(selectedId)?.focus({ preventScroll: true });
    wasRenaming.current = renaming;
  }, [renaming, selectedId]);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    // Only from a row: the rename field keeps its own keys.
    if ((e.target as HTMLElement).getAttribute("role") !== "option") return;
    const index = versions.findIndex((v) => v.id === selectedId);
    const next = step(e.key, Math.max(index, 0), versions.length);
    if (next == null) return;
    e.preventDefault();
    const id = versions[next].id;
    onSelect(id);
    // The row's tabindex follows on render; focus it now so keys keep landing.
    const el = option(id);
    el?.focus({ preventScroll: true });
    el?.scrollIntoView({ block: "nearest" });
  };

  return (
    <div
      ref={ref}
      role="listbox"
      aria-label="Versions"
      onKeyDown={onKeyDown}
      className="min-h-0 flex-1 overflow-y-auto"
    >
      {/* One child, so the fade's observer sees rows come and go. */}
      <div className="pb-1">
        {days.map(({ day, rows }) => {
          const headingId = `${uid}-day-${day}`;
          return (
            <div key={day} role="group" aria-labelledby={headingId}>
              <div
                id={headingId}
                className="px-3 pt-3 pb-1 text-[11px] font-medium text-muted-foreground"
              >
                {dayHeading(day, now)}
              </div>
              {rows.map(({ version: v, index }) => (
                <Fragment key={v.id}>
                  {renaming && v.id === selectedId ? (
                    <LabelField
                      version={v}
                      onDone={(label) => onRelabel(v, label)}
                      onCancel={onCancelRename}
                    />
                  ) : (
                    <VersionRow
                      version={v}
                      delta={lengthDelta(versions, index)}
                      selected={v.id === selectedId}
                      onSelect={() => {
                        onSelect(v.id);
                        onOpen?.(v.id);
                      }}
                    />
                  )}
                </Fragment>
              ))}
            </div>
          );
        })}
      </div>
    </div>
  );
}
