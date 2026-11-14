import type { CSSProperties } from "react";
import { referenceRestClass } from "@/components/markdown/ReferencePreview";
import { faviconFor } from "@/hooks/shell/useBrowserTabs";
import { parseLink } from "@/lib/harness/links";
import { cn } from "@/lib/utils";
import { useBrowserStore } from "@/stores/shell/browserStore";

/**
 * A web link in the composer: its text as written (a markdown link's words),
 * drawn as a link but edited as text, so the caret walks through it and
 * Backspace takes one character. The site's icon is a pseudo-element
 * (`.composer-link`), not a node in the caret's way. `readEditor` reads the
 * text back, and an edit that leaves no link turns it back into prose.
 */
export function ComposerLink({ raw }: { raw: string }) {
  const link = parseLink(raw);
  const favicon = useBrowserStore((s) => (link ? faviconFor(link.href, s.favicons) : undefined));
  if (!link) return <>{raw}</>;
  const style = favicon ? ({ "--link-icon": `url(${JSON.stringify(favicon)})` } as CSSProperties) : undefined;
  return (
    <span
      data-link=""
      data-href={link.label != null ? link.href : undefined}
      data-icon={favicon ? "site" : "globe"}
      style={style}
      className={cn(referenceRestClass, "composer-link box-decoration-clone text-brand")}
    >
      {link.label ?? raw}
    </span>
  );
}
