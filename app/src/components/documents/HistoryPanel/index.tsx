import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import type { EditorView } from "@codemirror/view";

import { VersionView, type VersionViewHandle, type VersionViewInfo } from "@/components/documents/VersionView";
import { SkeletonRows } from "@/components/ui/layout/PageParts";
import { ResizeHandle } from "@/components/ui/layout/ResizeHandle";
import { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import { useStoredState } from "@/hooks/ui/useStoredState";
import type { DbFile } from "@/lib/db";

import type { NoteHost } from "../editor/core/host";
import { PANEL, PANEL_MAX_SHARE } from "./constants";
import { HistoryDialogs } from "./HistoryDialogs";
import { NoVersions } from "./NoVersions";
import { useCompare } from "./useCompare";
import { useShownVersion } from "./useShownVersion";
import { useVersionActions } from "./useVersionActions";
import { useVersionList } from "./useVersionList";
import { useVersionTexts } from "./useVersionTexts";
import { HistoryBar, VersionBar } from "./VersionBar";
import { VersionList } from "./VersionList";
import { VersionPage } from "./VersionPage";

const readListShown = (raw: string | null) => raw !== "0";
const writeListShown = (on: boolean) => (on ? "1" : "0");

/**
 * A note's saved versions (`@/lib/notes/documentVersions`), laid over the
 * editor while History is open. The selected version takes the note's place
 * on the page, read-only and marked against the previous version or the note
 * now (`VersionView`) — skeletons until it lands, or why there is none. The
 * note stays mounted underneath, hidden by the editor while History is open.
 * The list is a sidebar docked right at any width, resizable, and hidden or
 * shown from its header or the bar's Versions toggle without leaving History;
 * the bar's Back arrow (or Esc anywhere inside) leaves. A version can come
 * back as a new note (the default) or replace the open one — after a
 * `restore` snapshot, as one undoable edit through the editor so it saves
 * like typing. Checkpoints can be renamed or deleted; snapshots are the app's
 * to prune.
 */
export function HistoryPanel({
  file,
  subject,
  title,
  host,
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

  /** The sidebar, kept across notes. */
  const [listShown, setListShown] = useStoredState("oculus-history-list-shown", readListShown, writeListShown);
  // Focusable, so a click on the read-only text keeps focus (and Esc) here.
  const pageRef = useRef<HTMLDivElement>(null);
  // With the sidebar hidden no row takes focus, so the page does, for Esc.
  useEffect(() => {
    if (!listShown) pageRef.current?.focus({ preventScroll: true });
    // Only on open.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const pageShown = shown != null && selected != null;
  const toggleList = () => setListShown((s) => !s);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "Escape" || e.defaultPrevented) return;
    // A portalled dialog's keys bubble here through React; they aren't ours.
    if (!e.currentTarget.contains(e.target as Node) || confirm || renaming) return;
    e.preventDefault();
    onClose();
  };

  return (
    <div className="absolute inset-0 z-20 flex" onKeyDown={onKeyDown}>
      <div ref={pageRef} tabIndex={-1} className="relative min-w-0 flex-1 bg-card outline-none">
        {pageShown ? (
          <VersionPage
            bar={
              <VersionBar
                version={selected}
                listShown={listShown}
                onToggleList={toggleList}
                onClose={onClose}
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
                  setListShown(true);
                  setRenaming(true);
                }}
                onDelete={() => setConfirm("delete")}
              />
            }
            findBar={findBar}
            errors={[listError, actionError]}
            title={title}
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
          <VersionPage
            bar={<HistoryBar title={title} onClose={onClose} listShown={listShown} onToggleList={toggleList} />}
            errors={[listError, actionError]}
            title={title}
          >
            {versions?.length === 0 ? (
              <NoVersions className="py-16" />
            ) : shownError ? (
              <p className="mt-3 text-[11px] text-destructive">{shownError}</p>
            ) : (
              // Loading; a failed list read says so over the page instead.
              !listError && <SkeletonRows count={6} rowClassName="h-4" className="mt-6" />
            )}
          </VersionPage>
        )}
      </div>

      {listShown && (
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
            style={{ width: panel.width, maxWidth: PANEL_MAX_SHARE }}
            className="flex min-h-0 shrink-0 flex-col border-l border-border-subtle bg-card"
          >
            <VersionList
              versions={versions}
              listError={listError}
              selectedId={selectedId}
              onSelect={setSelectedId}
              renaming={renaming}
              onRelabel={relabel}
              onCancelRename={() => setRenaming(false)}
              onHide={() => {
                setListShown(false);
                // The button goes with the sidebar; keep focus (and Esc) in History.
                pageRef.current?.focus({ preventScroll: true });
              }}
            />
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
