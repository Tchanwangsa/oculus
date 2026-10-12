import { VideoCamera } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { openExternal } from "@/lib/browser";

/**
 * An Echo360 embed outside a library file, where there is no subject to
 * resolve it against (`EmbedVideoCard` falls back to this). Opens the Canvas
 * launch URL in a browser tab; ⌘-click opens the system browser. A span, as
 * the tag can sit inside a paragraph or a table cell.
 */
export function EchoVideoPlaceholder({ linkId, src }: { linkId?: string; src: string }) {
  return (
    <span className="my-3 block" data-link-id={linkId}>
      <Button variant="outline" size="sm" onClick={(e) => void openExternal(src, e.metaKey)}>
        <VideoCamera size={14} />
        Echo360 video
      </Button>
    </span>
  );
}
