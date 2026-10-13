import { useRef, useState } from "react";
import { FileText, FolderOpen } from "@phosphor-icons/react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

import { Button } from "@/components/ui/button";
import { ImageLightbox } from "@/components/ui/lightbox/Lightbox";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { runFileView } from "@/lib/ed/runFiles";
import type { RunFile } from "@/lib/ed/workspace";
import { isWindows } from "@/lib/platform";

import { RunFileText } from "./RunFileText";

const REVEAL = isWindows ? "Show in Explorer" : "Show in Finder";

/**
 * The files a run left, under its output as Ed shows them: pictures at the
 * strip's width (a click opens one large), other files as chips — text opens
 * in a dialog, anything else in the file manager.
 */
export function RunFiles({ files }: { files: RunFile[] }) {
  const scroller = useRef<HTMLDivElement>(null);
  useScrollFade(scroller, "y");
  const [zoomed, setZoomed] = useState<RunFile | null>(null);
  const [reading, setReading] = useState<RunFile | null>(null);
  const images = files.filter((f) => runFileView(f) === "image");
  const others = files.filter((f) => runFileView(f) !== "image");
  return (
    <section
      aria-label="Files the program made"
      ref={scroller}
      className="max-h-[55%] shrink-0 space-y-2 overflow-y-auto border-t border-border-subtle px-3 py-2"
    >
      {images.map((file) => (
        <figure key={file.name} className="space-y-1">
          {/* A transparent diagram is drawn for a white page, as Ed shows it. */}
          <button
            type="button"
            onClick={() => setZoomed(file)}
            title="Open larger"
            className="block max-w-full cursor-zoom-in rounded-md border border-border-subtle bg-white"
          >
            <img src={file.dataUrl ?? ""} alt={file.name} draggable={false} className="block h-auto max-w-full" />
          </button>
          <figcaption className="text-xs text-muted-foreground">{file.name}</figcaption>
        </figure>
      ))}
      {others.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {others.map((file) =>
            runFileView(file) === "text" ? (
              <Button key={file.name} size="xs" variant="secondary" onClick={() => setReading(file)}>
                <FileText aria-hidden />
                {file.name}
              </Button>
            ) : (
              <Button
                key={file.name}
                size="xs"
                variant="secondary"
                title={REVEAL}
                onClick={() => file.path && revealItemInDir(file.path).catch((e) => console.warn("Ed run: reveal failed", e))}
              >
                <FolderOpen aria-hidden />
                {file.name}
              </Button>
            ),
          )}
        </div>
      )}
      <ImageLightbox
        src={zoomed?.dataUrl ?? ""}
        alt={zoomed?.name}
        open={zoomed != null}
        onOpenChange={(open) => !open && setZoomed(null)}
      />
      <RunFileText file={reading} onClose={() => setReading(null)} />
    </section>
  );
}
