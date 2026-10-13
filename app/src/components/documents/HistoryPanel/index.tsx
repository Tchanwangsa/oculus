import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import type { EditorView } from "@codemirror/view";

import { VersionView, type VersionViewHandle, type VersionViewInfo } from "@/components/documents/VersionView";
import { SkeletonRows } from "@/components/ui/layout/PageParts";
import { ResizeHandle } from "@/components/ui/layout/ResizeHandle";
import { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import type { WidthTier } from "@/hooks/ui/useWidthTier";
import type { DbFile } from "@/lib/db";

import type { NoteHost } from "../editor/core/host";
import { PANEL } from "./constants";
import { HistoryDialogs } from "./HistoryDialogs";
import { NoVersions } from "./NoVersions";
import { useCompare } from "./useCompare";
import { useShownVersion } from "./useShownVersion";
import { useVersionActions } from "./useVersionActions";
import { useVersionList } from "./useVersionList";
import { useVersionTexts } from "./useVersionTexts";
import { VersionBar } from "./VersionBar";
import { VersionList } from "./VersionList";
import { VersionPage } from "./VersionPage";

/**
 * A note's saved versions (`@/lib/notes/documentVersions`), laid over the
 * editor while History is open. The selected version takes the note's place
 * on the page, read-only and marked against the previous version or the note
 * now (`VersionView`) — skeletons until it lands, or why there is none. The
 * note stays mounted underneath, hidden by the editor while History is open.
 * The list docks right on a wide pane, floats over the page's edge on
 * a medium one, and on a narrow one is a pane of its own the version opens
 * from. A version can come back as a new note (the default) or replace the
 * open one — after a `restore` snapshot, as one undoable edit through the
 * editor so it saves like typing. Checkpoints can be renamed or deleted;
 * snapshots are the app's to prune. Esc inside any of it closes History.
 */
export function HistoryPanel({
  file,
  subject,
  title,
  host,
  tier,
  currentText,
  replaceText,
  findBar,
  onView,
  onClose,
}: {
  file: DbFile;
  /** Null while the subject list loads; a copy needs it. */
  subject: { id: number; code: string } | null;
  /** The note's title, shown over the version. */
  title: string;
  /** The note's own host: the version's pictures and links resolve as the note's. */
  host: NoteHost;
  tier: WidthTier;
  /** The editor's text now, or null with no editor. */
  currentText: () => string | null;
  /** Swap the editor's whole text as one undoable edit; false with no editor. */
  replaceText: (text: string) => boolean;
  /** The note's find bar, while it searches the version. */
  findBar: ReactNode;
  /** The version's view while one is on the page, for find. */
  onView: (view: EditorView | null) => void;
  onClose: () => void;
}) {
  const panel = useResizablePanel(PANEL);
  const { versions, listError, selectedId, setSelectedId, selected } = useVersionList(file.id);
  const { texts, textOf } = useVersionTexts();
  const { busy, actionError, confirm, setConfirm, renaming, setRenaming, restoreCopy, replace, relabel, remove } =
    useVersionActions({ file, subject, selectedId, setSelectedId, texts, textOf, currentText, replaceText });
  const { compare, setCompare, showChanges, setShowChanges } = useCompare();
  const { shown, shownError, older, effective, rereadCurrent } = useShownVersion({
    versions,
    selected,
    compare,
    showChanges,
    textOf,
    currentText,
  });
  const [info, setInfo] = useState<VersionViewInfo | null>(null);
  const handleRef = useRef<VersionViewHandle>(null);

  /** Medium: the floating list. Narrow: which pane is up. */
  const [listShown, setListShown] = useState(true);
  const [narrowPane, setNarrowPane] = useState<"list" | "version">("list");
  // Narrowed while reading a version: stay on it rather than jump to the list.
  const lastTier = useRef(tier);
  useEffect(() => {
    if (tier === "narrow" && lastTier.current !== "narrow") setNarrowPane(shown ? "version" : "list");
    lastTier.current = tier;
  }, [tier, shown]);

  const pageShown = shown != null && selected != null;
  const listCovers = tier === "narrow" && (narrowPane === "list" || !pageShown);
  const listHeaderShown = tier === "wide" || (tier === "medium" && listShown) || listCovers;

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "Escape" || e.defaultPrevented) return;
    // A portalled dialog's keys bubble here through React; they aren't ours.
    if (!e.currentTarget.contains(e.target as Node) || confirm || renaming) return;
    e.preventDefault();
    onClose();
  };

  const list = (
    <VersionList
      versions={versions}
      listError={listError}
      selectedId={selectedId}
      onSelect={setSelectedId}
      onOpen={() => tier === "narrow" && setNarrowPane("version")}
      renaming={renaming}
      onRelabel={relabel}
      onCancelRename={() => setRenaming(false)}
      onClose={onClose}
      // The page says it when it is on screen; a covering list says it itself.
      emptyCopy={listCovers}
    />
  );
  const float = tier === "medium" && listShown && (
    <div
      role="region"
      aria-label="Version history"
      className="absolute top-2 right-2 bottom-2 z-10 flex w-[min(320px,calc(100%_-_16px))] flex-col overflow-hidden rounded-xl border border-border bg-popover shadow-lg"
    >
      {list}
    </div>
  );

  return (
    <div className="absolute inset-0 z-20 flex" onKeyDown={onKeyDown}>
      <div
        // Focusable, so a click on the read-only text keeps focus (and Esc) here.
        tabIndex={-1}
        inert={listCovers}
        className="relative min-w-0 flex-1 bg-card outline-none"
      >
        {pageShown ? (
          <VersionPage
            bar={
              <VersionBar
                version={selected}
                tier={tier}
                onBack={() => setNarrowPane("list")}
                listShown={listShown}
                onToggleList={() => setListShown((s) => !s)}
                onClose={listHeaderShown ? undefined : onClose}
                info={info}
                handle={handleRef}
                showChanges={showChanges}
                onShowChanges={setShowChanges}
                compare={effective}
                onCompare={setCompare}
                previousDisabled={!older}
                busy={busy}
                canCopy={!!subject}
                onRestoreCopy={() => void restoreCopy(selected)}
                onReplace={() => setConfirm("replace")}
                onRename={() => {
                  // The label is edited in its row, so the row must be on screen.
                  if (tier === "medium") setListShown(true);
                  if (tier === "narrow") setNarrowPane("list");
                  setRenaming(true);
                }}
                onDelete={() => setConfirm("delete")}
              />
            }
            findBar={findBar}
            actionError={actionError}
            title={title}
            float={float}
          >
            {shownError && <p className="mt-3 text-[11px] text-destructive">{shownError}</p>}
            <VersionView
              text={shown.text}
              base={shown.base}
              host={host}
              handleRef={handleRef}
              onInfo={setInfo}
              onView={onView}
              className="mt-4"
            />
          </VersionPage>
        ) : (
          <VersionPage title={title} float={float}>
            {versions?.length === 0 ? (
              <NoVersions className="py-16" />
            ) : shownError ? (
              <p className="mt-3 text-[11px] text-destructive">{shownError}</p>
            ) : (
              // Loading; a failed list read says so in the list instead.
              !listError && <SkeletonRows count={6} rowClassName="h-4" className="mt-6" />
            )}
          </VersionPage>
        )}
      </div>

      {listCovers && (
        <div
          role="region"
          aria-label="Version history"
          className="absolute inset-0 z-10 flex flex-col bg-card"
        >
          {list}
        </div>
      )}

      {tier === "wide" && (
        <>
          {/* On the seam: negative margins cost no layout width. */}
          <ResizeHandle
            onMouseDown={panel.onMouseDown}
            dragging={panel.dragging}
            label="Resize history"
            className="-mx-0.5"
          />
          <aside
            aria-label="Version history"
            style={{ width: panel.width, maxWidth: "50%" }}
            className="flex min-h-0 shrink-0 flex-col border-l border-border-subtle bg-card"
          >
            {list}
          </aside>
        </>
      )}

      <HistoryDialogs
        selected={selected}
        confirm={confirm}
        busy={busy}
        onCancel={() => setConfirm(null)}
        onReplace={async (v) => {
          await replace(v);
          rereadCurrent();
        }}
        onDelete={remove}
      />
    </div>
  );
}
