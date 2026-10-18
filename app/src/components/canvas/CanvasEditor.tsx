import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import type { DbFile } from "@/lib/db";
import { fileTitle } from "@/lib/files/openFile";
import { parseCanvasDocument, serializeCanvasDocument, type CanvasDocument } from "@/lib/canvas/model";
import { canvasSessions } from "@/lib/canvas/sessionRuntime";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import { Button } from "@/components/ui/button";
import { CanvasSurface } from "./CanvasSurface";

export default function CanvasEditor({ file }: { file: DbFile }) {
  const session = useMemo(() => canvasSessions.open(file), [file.id]);
  const snapshot = useSyncExternalStore(session.subscribe, session.snapshot);
  const lastDocument = useRef<{ text: string; document: CanvasDocument } | null>(null);
  const [title, setTitle] = useState(() => fileTitle(file));
  const [renaming, setRenaming] = useState(false);
  const [renameError, setRenameError] = useState<string | null>(null);
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
    catch (error) { setRenameError(String(error)); }
    finally { setRenaming(false); }
  };

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
          {renaming ? "Renaming…" : snapshot.status === "saving" ? "Saving…" : snapshot.status === "unsaved" ? "Unsaved changes" : snapshot.status === "error" ? "Save failed" : "Saved"}
        </span>
        {snapshot.status === "error" && !snapshot.recovery && <Button size="sm" variant="outline" onClick={() => void session.flush().catch(() => {})}>Retry save</Button>}
      </div>
      {(snapshot.error || renameError) && <p role="alert" className="shrink-0 border-b border-border-subtle px-4 py-2 text-xs text-destructive">{renameError ?? snapshot.error}</p>}
      {snapshot.recovery && <div className="flex shrink-0 gap-2 px-4 py-2">
        <Button size="sm" variant="outline" disabled={!snapshot.recovery.valid} onClick={() => session.recoverDraft()}>Use recovery copy</Button>
        <Button size="sm" variant="outline" onClick={() => session.discardDraft()}>Keep disk version</Button>
      </div>}
      <div inert={!!snapshot.recovery} className="min-h-0 flex-1"><CanvasSurface document={parsed.document} onChange={(document) => { const text = serializeCanvasDocument(document); lastDocument.current = { text, document }; session.update(text); }} /></div>
    </div>
  );
}
