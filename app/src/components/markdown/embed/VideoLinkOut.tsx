import { VideoCamera } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { openExternal } from "@/lib/browser";

/**
 * A page's video outside a library file, where there is no subject to
 * resolve or download it against (`EmbedVideoCard`, `CanvasVideoCard`). Opens
 * `href` in a browser tab; ⌘-click opens the system browser. A span, as the
 * tag can sit inside a paragraph or a table cell.
 */
export function VideoLinkOut({ label, href }: { label: string; href: string }) {
  return (
    <span className="my-3 block">
      <Button
        variant="outline"
        size="sm"
        className="max-w-full"
        title={label}
        onClick={(e) => void openExternal(href, e.metaKey)}
      >
        <VideoCamera size={14} />
        <span className="truncate">{label}</span>
      </Button>
    </span>
  );
}
