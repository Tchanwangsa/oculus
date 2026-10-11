import { EmbedImage } from "@/components/markdown/embed/EmbedImage";
import { EmbedPage } from "@/components/markdown/embed/EmbedPage";

/**
 * A picture or HTML page from the library, drawn inline in a reply — what an
 * agent made in `agents/` (a chart, a visual summary) shown where it names it.
 *
 * Every element is a `span` or phrasing content: `![alt](path)` alone on a
 * line parses as `<p><img></p>`, and a `<div>` inside a `<p>` is invalid.
 * The wrapper carries `data-md`, so a copied selection gets the markdown
 * back (`lib/markdown/selection.ts`), not the asset URL or the header's text.
 */

const IMAGE = /\.(png|jpe?g|gif|webp|avif|svg)$/i;
const HTML = /\.html?$/i;

/** What `OutputEmbed` draws for a path, by extension; `null` for anything else. */
export function embedKind(path: string): "image" | "html" | null {
  const bare = path.split(/[?#]/)[0];
  if (IMAGE.test(bare)) return "image";
  if (HTML.test(bare)) return "html";
  return null;
}

/** `path` is data-dir-relative (`agents/…`, `courses/…`, `lectures/…`). */
export function OutputEmbed({ path, alt }: { path: string; alt?: string }) {
  const kind = embedKind(path);
  if (kind === "image") return <EmbedImage path={path} alt={alt} />;
  if (kind === "html") return <EmbedPage path={path} alt={alt} />;
  return null;
}
