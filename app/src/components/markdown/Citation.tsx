import { useState, type MouseEvent, type ReactNode } from "react";
import { FileChip } from "@/components/markdown/FileChip";
import { ImageLightbox } from "@/components/ui/lightbox/Lightbox";
import { attachmentSrc } from "@/lib/harness/attachments";
import { parseCitation, type Citation, type CitationShape } from "@/lib/citations";
import { openCitation } from "@/lib/files/openFile";
import { useCitation } from "@/hooks/agents/useCitation";
import { useDataDir } from "@/hooks/backend/useDataDir";

const PICTURE = /\.(?:png|jpe?g|gif|webp|svg|avif)$/i;

/**
 * How a citation opens: `openCitation`, except a picture under `agents/`,
 * which has no row to open in the side panel and shows in a lightbox here.
 * The lightbox sits in a span that stops clicks, since React bubbles a
 * portal's events to an enclosing `InlineMd` button.
 */
function useOpener(cite: Citation | null): [(newTab: boolean) => void, ReactNode] {
  const dataDir = useDataDir();
  const [shown, setShown] = useState<string | null>(null);
  const open = (newTab: boolean) => {
    if (!cite) return;
    if (cite.path.startsWith("agents/") && PICTURE.test(cite.path) && !newTab)
      setShown(attachmentSrc(dataDir, cite.path));
    else openCitation(cite, newTab);
  };
  const lightbox = shown !== null && (
    <span onClick={(e) => e.stopPropagation()}>
      <ImageLightbox
        src={shown}
        alt={cite?.path}
        open
        onOpenChange={(o) => !o && setShown(null)}
      />
    </span>
  );
  return [open, lightbox];
}

/** An inline code span that is wholly a citation, as a chip; `code` (the
 *  span as written) until a tail resolves, and for good if it never does. */
export function CitationCode({ shape, code }: { shape: CitationShape; code: ReactNode }) {
  const cite = useCitation(shape);
  const [open, lightbox] = useOpener(cite);
  if (!cite) return <>{code}</>;
  return (
    <>
      <FileChip path={cite.path} cite={cite} onClick={open} />
      {lightbox}
    </>
  );
}

/** A link whose href is a citation: the brand-coloured button. The link
 *  text's "page 25" counts when the href has no location. */
export function CitationLink({
  shape,
  children,
  ...p
}: {
  shape: CitationShape;
  children: ReactNode;
}) {
  const cite = useCitation(shape);
  const [open, lightbox] = useOpener(cite);
  if (!cite) return <span>{children}</span>;
  return (
    <>
      <button
        type="button"
        title={cite.path}
        onClick={(e: MouseEvent) => {
          e.stopPropagation();
          open(e.metaKey || e.ctrlKey);
        }}
        className="text-left text-brand hover:underline"
        {...p}
      >
        {children}
      </button>
      {lightbox}
    </>
  );
}

/** A link's visible text, for `parseCitation`'s page hint. */
export function linkText(children: ReactNode): string {
  if (typeof children === "string" || typeof children === "number") return String(children);
  if (Array.isArray(children)) return children.map(linkText).join("");
  if (children && typeof children === "object" && "props" in children)
    return linkText((children.props as { children?: ReactNode }).children);
  return "";
}

/** `parseCitation` for an href, reading its text for a page. */
export function linkCitation(href: string | undefined, children: ReactNode): CitationShape | null {
  return parseCitation(href, { linkText: linkText(children) });
}
