import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { ShareNetworkIcon } from "@phosphor-icons/react";
import type { DbFile } from "@/lib/db";
import { fileTitle } from "@/lib/files/openFile";
import { parseCanvasDocument, serializeCanvasDocument, type CanvasDocument } from "@/lib/canvas/model";
import { canvasSessions } from "@/lib/canvas/sessionRuntime";
import { noteNotebookPage, registerOpenNotebook } from "@/lib/canvas/openNotebooks";
import { useNotebookSnapshots } from "@/hooks/canvas/useNotebookSnapshots";
import { saveCanvasCopy } from "@/lib/canvas/files";
import { errorMessage } from "@/lib/canvas/authority";
import type { CanvasSnapshot } from "@/lib/canvas/sessions";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import { Button } from "@/components/ui/button";
import { useSharing } from "@/lib/sharing/store";
import { devices } from "@/lib/sharing/view";
import { CanvasShareDialog } from "./CanvasShareDialog";
import { CanvasSurface } from "./CanvasSurface";

const SHARING: Partial<Record<CanvasSnapshot["authority"], string>> = { attaching: "Starting sharing", live: "Sharing", paused: "Sharing paused", ended: "Sharing ended" };
const saveLabel = (status: CanvasSnapshot["status"]) =>
  status === "saving" ? "Saving…" : status === "unsaved" ? "Unsaved changes" : status === "error" ? "Save failed" : "Saved";

export default function CanvasEditor({ file }: { file: DbFile }) {
  const session = useMemo(() => canvasSessions.open(file), [file.id]);
  const snapshot = useSyncExternalStore(session.subscribe, session.snapshot);
  const lastDocument = useRef<{ text: string; document: CanvasDocument } | null>(null);
  const [title, setTitle] = useState(() => fileTitle(file));
  const [renaming, setRenaming] = useState(false);
  const [renameError, setRenameError] = useState<string | null>(null);
  const [notice, setNotice] = useState<{ text: string; failed: boolean } | null>(null);
  const [shareOpen, setShareOpen] = useState(false);
  // Mounted on first use, then kept so closing animates; sharing outlives the dialog.
  const [shareMounted, setShareMounted] = useState(false);
  const { share } = useSharing(file.id);
  useEffect(() => { setTitle(fileTitle(session.file)); }, [session, file.filename]);
  useEffect(() => {
    void session.load().catch(() => {});
    const flush = () => { void session.flush().catch(() => {}); };
    window.addEventListener("pagehide", flush);
    return () => { window.removeEventListener("pagehide", flush); void session.flush().finally(() => session.release()).catch(() => {}); };
  }, [session]);

  const parsed = useMemo(() => {
    if (snapshot.text == null) return {};
    try {
      if (lastDocument.current?.text === snapshot.text) return { document: lastDocument.current.document };
      const document = parseCanvasDocument(snapshot.text);
      lastDocument.current = { text: snapshot.text, document };
      return { document };
    }
    catch (error) { return { error: String(error) }; }
  }, [snapshot.text]);

  const rename = async () => {
    if (renaming || title === fileTitle(session.file)) return;
    setRenaming(true);
    setRenameError(null);
    try { await session.rename(title); setTitle(fileTitle(session.file)); }
    catch (error) { setRenameError(errorMessage(error)); }
    finally { setRenaming(false); }
  };
  const act = (action: () => Promise<unknown>) => { setNotice(null); void action().catch((error) => setNotice({ text: errorMessage(error), failed: true })); };
  const keepDraftCopy = () => act(async () => {
    const copy = await saveCanvasCopy(session.file, `${fileTitle(session.file)} (draft)`, snapshot.text!);
    setNotice({ text: `Draft saved as ${fileTitle(copy)} in Drive.`, failed: false });
  });
  const clients = snapshot.authority === "live" && share?.clients ? ` · ${devices(share.clients)}` : "";
  const sharing = SHARING[snapshot.authority] && `${SHARING[snapshot.authority]}${clients}`;
  const blocked = !!snapshot.recovery || !!snapshot.conflict;
  const snapshotError = useNotebookSnapshots(session.file, parsed.document);
  const pageCount = parsed.document?.paper?.pages.length;
  const pageInView = useRef(0);
  const openTitle = fileTitle(session.file), openPath = session.file.relative_path;
  // While a notebook is open, a chat message tells the agent which one and where to read it.
  useEffect(() => {
    if (!pageCount) return;
    return registerOpenNotebook(file.id, { path: openPath, title: openTitle, pages: pageCount, page: pageInView.current + 1 });
  }, [file.id, openPath, openTitle, pageCount]);

  if (snapshot.text == null && snapshot.status !== "error") return <LoadingFill />;
  if (!parsed.document) return (
    <div className="p-6 text-xs text-destructive" role="alert">
      <p>{parsed.error ?? snapshot.error ?? "Canvas could not be loaded."}</p>
      {snapshot.text == null && <Button className="mt-3" size="sm" variant="outline" onClick={() => void session.load().catch(() => {})}>Retry</Button>}
    </div>
  );
  if (snapshot.status === "deleted") return <div className="p-6 text-xs text-muted-foreground">This canvas was deleted.</div>;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center gap-3 border-b border-border-subtle px-4 py-2">
        <input
          aria-label="Canvas title" value={title} disabled={renaming}
          onChange={(event) => setTitle(event.target.value)} onBlur={() => void rename()}
          onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); if (event.key === "Escape") { setTitle(fileTitle(session.file)); event.preventDefault(); } }}
          className="min-w-0 flex-1 bg-transparent text-xs font-medium outline-none"
        />
        <span role="status" className="shrink-0 text-[11px] text-muted-foreground">
          {sharing && `${sharing} · `}{renaming ? "Renaming…" : saveLabel(snapshot.status)}
        </span>
        {snapshot.status === "error" && !blocked && snapshot.authority !== "paused" && <Button size="sm" variant="outline" onClick={() => void session.flush().catch(() => {})}>Retry save</Button>}
        <Button size="sm" variant="ghost" onClick={() => { setShareMounted(true); setShareOpen(true); }}><ShareNetworkIcon />Share</Button>
      </div>
      {shareMounted && <CanvasShareDialog session={session} open={shareOpen} onOpenChange={setShareOpen} />}
      {(snapshot.error || renameError) && <p role="alert" className="shrink-0 border-b border-border-subtle px-4 py-2 text-xs text-destructive">{renameError ?? snapshot.error}</p>}
      {snapshotError && <p role="alert" className="shrink-0 border-b border-border-subtle px-4 py-2 text-xs text-destructive">The page pictures a chat agent reads could not be updated: {snapshotError}</p>}
      {notice && <p role={notice.failed ? "alert" : "status"} className={`shrink-0 border-b border-border-subtle px-4 py-2 text-xs ${notice.failed ? "text-destructive" : "text-muted-foreground"}`}>{notice.text}</p>}
      {snapshot.recovery && <div className="flex shrink-0 gap-2 px-4 py-2">
        <Button size="sm" variant="outline" disabled={!snapshot.recovery.valid} onClick={() => session.recoverDraft()}>Use recovery copy</Button>
        <Button size="sm" variant="outline" onClick={() => session.discardDraft()}>Keep disk version</Button>
      </div>}
      {snapshot.conflict && <div className="flex shrink-0 gap-2 px-4 py-2">
        <Button size="sm" variant="outline" onClick={() => act(() => session.useSharedCanvas())}>Use shared canvas</Button>
        <Button size="sm" variant="outline" onClick={keepDraftCopy}>Save draft as copy</Button>
      </div>}
      {snapshot.authority === "paused" && <div className="flex shrink-0 gap-2 px-4 py-2">
        <Button size="sm" variant="outline" onClick={() => act(() => session.resolveExternal("adopt-disk"))}>Use file on disk</Button>
        <Button size="sm" variant="outline" onClick={() => act(() => session.resolveExternal("keep-ours"))}>Keep my version</Button>
      </div>}
      <div className="relative min-h-0 flex-1">
        {snapshot.recoveryError && <p role="status" className="absolute right-3 bottom-16 left-3 z-30 rounded-xl border border-border-subtle bg-card px-3 py-2 text-xs text-muted-foreground">{snapshot.recoveryError}</p>}
        <div inert={blocked} className="h-full"><CanvasSurface document={parsed.document} title={openTitle} onPageChange={(index) => { pageInView.current = index; noteNotebookPage(file.id, index + 1); }} onChange={(document) => { const text = serializeCanvasDocument(document); lastDocument.current = { text, document }; session.update(text); }} /></div>
      </div>
    </div>
  );
}
