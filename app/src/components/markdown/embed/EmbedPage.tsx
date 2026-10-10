import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { ArrowSquareOut, ArrowsOutSimple, CircleNotch, FileHtml } from "@phosphor-icons/react";
import { useDataDir } from "@/hooks/backend/useDataDir";
import { readCourseFile } from "@/lib/files/courseFiles";
import { isDark, subscribeDark } from "@/lib/ui/theme";
import { IconAction } from "@/components/markdown/embed/IconAction";
import { PageViewer } from "@/components/markdown/embed/PageViewer";
import {
  DEFAULT_HEIGHT,
  HEIGHT_MESSAGE,
  MAX_HEIGHT,
  MIN_HEIGHT,
  baseHref,
  prepare,
} from "@/components/markdown/embed/pagePrepare";
import { basename, openExternally, sourceOf } from "@/components/markdown/embed/shared";

export function EmbedPage({ path, alt }: { path: string; alt?: string }) {
  const dataDir = useDataDir();
  const dark = useSyncExternalStore(subscribeDark, isDark);
  const [source, setSource] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const [height, setHeight] = useState(DEFAULT_HEIGHT);
  const [open, setOpen] = useState(false);
  const frame = useRef<HTMLIFrameElement>(null);
  const title = alt || basename(path);

  useEffect(() => {
    let live = true;
    setSource(null);
    setFailed(false);
    readCourseFile(path).then(
      (text) => live && setSource(text),
      () => live && setFailed(true),
    );
    return () => {
      live = false;
    };
  }, [path]);

  // A theme flip rebuilds the document (and reloads the frame) so the
  // injected scheme follows; the last height holds until it reports again.
  const doc = useMemo(
    () => (source != null && dataDir ? prepare(source, baseHref(dataDir, path), dark) : null),
    [source, dataDir, path, dark],
  );

  // Only this frame's own reports; the expanded copy and other embeds post too.
  useEffect(() => {
    const onMessage = (e: MessageEvent) => {
      if (!frame.current || e.source !== frame.current.contentWindow) return;
      if (e.data?.type !== HEIGHT_MESSAGE) return;
      const h = Number(e.data.height);
      if (Number.isFinite(h)) setHeight(Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, Math.ceil(h))));
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, []);

  return (
    <span
      data-md={sourceOf(path, alt)}
      className="my-3 block overflow-hidden rounded-lg border border-border bg-card"
    >
      <span className="flex h-8 min-w-0 select-none items-center gap-1.5 border-b border-border pr-1 pl-2.5 text-xs text-muted-foreground">
        <FileHtml size={14} className="shrink-0" />
        <span className="min-w-0 flex-1 truncate" title={path}>
          {title}
        </span>
        <IconAction label="Expand" onClick={() => setOpen(true)} disabled={!doc}>
          <ArrowsOutSimple size={13} />
        </IconAction>
        <IconAction label="Open externally" onClick={() => openExternally(path)}>
          <ArrowSquareOut size={13} />
        </IconAction>
      </span>
      {failed ? (
        <span className="block px-3 py-3 text-xs break-all text-muted-foreground">
          Couldn't read {path}
        </span>
      ) : doc == null ? (
        <span style={{ height }} className="flex items-center justify-center text-muted-foreground">
          <CircleNotch size={14} className="animate-spin" />
        </span>
      ) : (
        <iframe
          ref={frame}
          // Never `allow-same-origin`: the page would get the app's origin and the Tauri IPC.
          sandbox="allow-scripts"
          srcDoc={doc}
          title={title}
          style={{ height }}
          className="block w-full border-0"
        />
      )}
      {doc != null && (
        <PageViewer doc={doc} title={title} path={path} open={open} onOpenChange={setOpen} />
      )}
    </span>
  );
}
