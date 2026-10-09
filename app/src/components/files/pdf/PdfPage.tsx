import { memo, useEffect, useRef, useState, type CSSProperties } from "react";
import { pageLinks, pageText, rasterSize, renderPage, type PdfLink, type PdfPageSize } from "@/lib/pdfView";
import type { Rect } from "@/lib/pdfFind";
import { UNIT, type Box } from "./layout";
import { buildTextLayer } from "./textLayer";

export interface Highlight extends Rect {
  current: boolean;
}

interface Props {
  path: string;
  box: Box;
  size: PdfPageSize;
  /** The scale the canvas is drawn at: the live one once a zoom settles. */
  renderScale: number;
  dpr: number;
  /** On or near the screen: draw the canvas, text and links. */
  near: boolean;
  highlights?: readonly Highlight[];
  /** The boxes of the parse blocks a citation names. */
  blockBoxes?: readonly Rect[];
  /** After this page's text layer is (re)built. */
  onTextLayer: (page: number) => void;
  onGoToPage: (page: number) => void;
}

const pt = (v: number) => `calc(var(--pdf-unit) * ${v}px)`;

const placed = (r: Rect): CSSProperties => ({
  left: pt(r.x),
  top: pt(r.y),
  width: pt(r.width),
  height: pt(r.height),
});

/** One page: its box always, and while `near` the raster, block boxes, find
 *  highlights, text layer and links, stacked in that order. */
export const PdfPage = memo(function PdfPage({
  path,
  box,
  size,
  renderScale,
  dpr,
  near,
  highlights,
  blockBoxes,
  onTextLayer,
  onGoToPage,
}: Props) {
  const style = {
    left: box.left,
    top: box.top,
    width: box.width,
    height: box.height,
    "--pdf-unit": box.width / size.width,
  } as CSSProperties;
  return (
    <div className="page" data-page-number={box.page} style={style}>
      {near && <PageCanvas path={path} page={box.page} size={size} scale={renderScale} dpr={dpr} />}
      {near && blockBoxes?.length ? (
        <div className="pdf-block-layer">
          {blockBoxes.map((b, i) => (
            <div key={i} style={placed(b)} />
          ))}
        </div>
      ) : null}
      {near && highlights?.length ? (
        <div className="pdf-find-layer">
          {highlights.map((h, i) => (
            <div key={i} className={h.current ? "pdf-find-current" : "pdf-find-match"} style={placed(h)} />
          ))}
        </div>
      ) : null}
      {near && <TextLayer path={path} page={box.page} onBuilt={onTextLayer} />}
      {near && <LinkLayer path={path} page={box.page} onGoToPage={onGoToPage} />}
    </div>
  );
});

/** The raster at `scale` × the device ratio. Between a zoom and its settling
 *  the old pixels stretch with the box; a new raster replaces them in one
 *  step, and a response for a size since replaced is dropped. */
function PageCanvas({
  path,
  page,
  size,
  scale,
  dpr,
}: {
  path: string;
  page: number;
  size: PdfPageSize;
  scale: number;
  dpr: number;
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  const { width, height } = rasterSize(
    Math.round(size.width * scale * UNIT * dpr),
    Math.round(size.height * scale * UNIT * dpr),
  );
  useEffect(() => {
    let live = true;
    renderPage(path, page, width, height, () => live)
      .then((image) => {
        const canvas = ref.current;
        if (!live || !image || !canvas) return;
        canvas.width = width;
        canvas.height = height;
        canvas.getContext("2d")?.putImageData(image, 0, 0);
      })
      .catch((err) => {
        if (live) console.warn(`pdf_render page ${page}:`, err);
      });
    return () => {
      live = false;
    };
  }, [path, page, width, height]);
  // Zero-sized until the first raster lands, so the page shows its ground.
  return <canvas ref={ref} width={0} height={0} className="pdf-canvas" />;
}

/** React owns only the layer element; its children are `buildTextLayer`'s. */
function TextLayer({ path, page, onBuilt }: { path: string; page: number; onBuilt: (page: number) => void }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let live = true;
    pageText(path, page)
      .then((lines) => {
        const layer = ref.current;
        if (!live || !layer) return;
        buildTextLayer(layer, lines);
        onBuilt(page);
      })
      .catch((err) => {
        if (live) console.warn(`pdf_text page ${page}:`, err);
      });
    return () => {
      live = false;
    };
  }, [path, page, onBuilt]);
  return <div ref={ref} className="textLayer" />;
}

/** Web links are plain anchors (`AppLayout` opens them in-app); a link into
 *  the document scrolls to its page. */
function LinkLayer({ path, page, onGoToPage }: { path: string; page: number; onGoToPage: (page: number) => void }) {
  const [links, setLinks] = useState<PdfLink[]>([]);
  useEffect(() => {
    let live = true;
    pageLinks(path, page)
      .then((l) => live && setLinks(l))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [path, page]);
  if (!links.length) return null;
  return (
    <div className="pdf-link-layer">
      {links.map((link, i) =>
        link.uri ? (
          <a key={i} href={link.uri} title={link.uri} draggable={false} style={placed(link)} />
        ) : link.page ? (
          <a
            key={i}
            href={`#page=${link.page}`}
            draggable={false}
            style={placed(link)}
            onClick={(e) => {
              e.preventDefault();
              onGoToPage(link.page!);
            }}
          />
        ) : null,
      )}
    </div>
  );
}
