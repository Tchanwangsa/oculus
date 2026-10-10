import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeRaw from "rehype-raw";
import rehypeKatex from "rehype-katex";
import type { PluggableList } from "unified";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { hasMath, normalizeMath } from "@/lib/markdown/math";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import { readCourseFile } from "@/lib/files/courseFiles";
import { loadPagesRecord } from "@/lib/citations";
import { layoutTop } from "@/lib/citations/locateQuote";
import { blockAnchorPlugins, refOfElement } from "@/lib/pdf/blockAnchors";
import { buildBlockDoc, type BlockDoc, type BlockRef } from "@/lib/pdf/pdfBlocks";
import { copyAsMarkdown, dragAsMarkdown } from "@/lib/markdown/selection";
import type { PdfMdLink } from "@/components/files/pdf/pdfMdLink";

const PLAIN = [remarkGfm];
const WITH_MATH = [remarkGfm, remarkMath];
const MATH = [rehypeKatex];
const RAW_MATH = [rehypeRaw, rehypeKatex];

/** KaTeX also accepts math fences and HTML math classes without delimiters.
 *  `anchors` (`blockAnchorPlugins`) go around KaTeX, after raw HTML is parsed. */
export function fileMarkdownPlugins(
  text: string,
  anchors?: ReturnType<typeof blockAnchorPlugins>,
): { remarkPlugins: PluggableList; rehypePlugins: PluggableList } {
  const raw = text.includes("<");
  return {
    remarkPlugins: hasMath(text) ? WITH_MATH : PLAIN,
    rehypePlugins: anchors
      ? [...(raw ? [rehypeRaw] : []), anchors.before, rehypeKatex, anchors.after]
      : raw
        ? RAW_MATH
        : MATH,
  };
}

/** Layout pixels between the view's top and a block brought to it; the
 *  reading line a position is read at sits just under it, so a restore
 *  reports the block it restored. */
const PLACE = 12;

/** A position is reported this long after the last scroll. */
const REPORT_MS = 150;

type Content = { text: string } | { doc: BlockDoc };

/**
 * A library file's markdown. Given `link`, it is a parsed PDF's Markdown
 * face: rendered from the `.pages.json` record with page and block anchors
 * (`lib/pdf/pdfBlocks.ts`), keeping its reading position in step with the PDF
 * face (`components/files/pdf/pdfMdLink.ts`). Without a record it falls back to the flat `.md`.
 */
export const FileMarkdown = memo(function FileMarkdown({
  relPath,
  components,
  link,
}: {
  relPath: string;
  components: Components;
  /** The parsed PDF's shared state; absent for any other markdown. */
  link?: PdfMdLink;
}) {
  const [content, setContent] = useState<Content | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const parsed = !!link;

  useEffect(() => {
    setContent(null);
    setErr(null);
    let live = true;
    const load: Promise<Content> = parsed
      ? loadPagesRecord(relPath, true).then(async (record) =>
          record ? { doc: buildBlockDoc(record) } : { text: await readCourseFile(relPath) },
        )
      : readCourseFile(relPath).then((text) => ({ text }));
    load.then((value) => live && setContent(value)).catch((e) => live && setErr(String(e)));
    return () => {
      live = false;
    };
  }, [relPath, parsed]);

  if (err)
    return (
      <div className="px-6 py-5">
        <Alert variant="destructive">
          <AlertDescription className="text-xs">Failed to load file: {err}</AlertDescription>
        </Alert>
      </div>
    );
  if (content === null) return <LoadingFill />;
  if ("doc" in content && link)
    return <BlockMarkdown doc={content.doc} components={components} link={link} />;
  const text = "text" in content ? content.text : content.doc.text;
  const math = hasMath(text);
  return (
    <div className="flex-1 overflow-y-auto">
      <article
        data-selectable
        className="markdown-body mx-auto w-full max-w-4xl px-6 py-5"
        onCopy={copyAsMarkdown}
        onDragStart={dragAsMarkdown}
      >
        <ReactMarkdown {...fileMarkdownPlugins(text)} components={components}>
          {math ? normalizeMath(text) : text}
        </ReactMarkdown>
      </article>
    </div>
  );
});

/** One render of the whole document; scrolls never re-run it. */
const Body = memo(function Body({
  text,
  plugins,
  components,
}: {
  text: string;
  plugins: ReturnType<typeof fileMarkdownPlugins>;
  components: Components;
}) {
  return (
    <ReactMarkdown {...plugins} components={components}>
      {text}
    </ReactMarkdown>
  );
});

/** An element's top in the scroller's content, in layout pixels: offsets,
 *  which page zoom leaves alone, not `getBoundingClientRect`. */
const topIn = (scroller: HTMLElement, el: HTMLElement) =>
  layoutTop(el) - layoutTop(scroller) - scroller.clientTop;

const anchorsIn = (article: HTMLElement) => Array.from(article.querySelectorAll<HTMLElement>("[data-page]"));

/** The anchored element at the reading line: the last starting above it if
 *  it reaches past the line, else the next. Anchors are in document order,
 *  so their tops never decrease. */
function topmost(scroller: HTMLElement, els: HTMLElement[]): HTMLElement | null {
  const line = scroller.scrollTop + PLACE + 1;
  let lo = 0;
  let hi = els.length - 1;
  let found = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (topIn(scroller, els[mid]) <= line) {
      found = mid;
      lo = mid + 1;
    } else hi = mid - 1;
  }
  if (found < 0) return els[0] ?? null;
  const el = els[found];
  return topIn(scroller, el) + el.offsetHeight > line ? el : (els[found + 1] ?? el);
}

/** The element showing `ref`: its block, else the page's next block, else
 *  the next page's first element. */
function elementFor(els: HTMLElement[], ref: BlockRef): HTMLElement | null {
  let later: HTMLElement | null = null;
  for (const el of els) {
    const r = refOfElement(el);
    if (!r || r.page < ref.page) continue;
    if (r.page > ref.page) return later ?? el;
    if (ref.block == null || r.block === ref.block) return el;
    if (!later && r.block != null && r.block > ref.block) later = el;
  }
  return later;
}

function BlockMarkdown({ doc, components, link }: { doc: BlockDoc; components: Components; link: PdfMdLink }) {
  const scrollerRef = useRef<HTMLDivElement>(null);
  const articleRef = useRef<HTMLElement>(null);
  const plugins = useMemo(() => fileMarkdownPlugins(doc.text, blockAnchorPlugins(doc)), [doc]);
  /** Where this face last read itself, for putting back after a reflow. */
  const selfRef = useRef<BlockRef | null>(null);
  /** Held at the top through reflows (images loading, a width change) until
   *  the reader scrolls somewhere else. */
  const pinRef = useRef<BlockRef | null>(null);
  /** The scroll offset `show` last left, to tell the reader's scrolls from ours. */
  const shownTopRef = useRef(0);
  const restoredRef = useRef(false);

  /** Brings `ref` to the view's top. */
  const show = (ref: BlockRef) => {
    const scroller = scrollerRef.current;
    const article = articleRef.current;
    if (!scroller || !article) return false;
    const el = elementFor(anchorsIn(article), ref);
    if (!el) return false;
    scroller.scrollTop = topIn(scroller, el) - PLACE;
    shownTopRef.current = scroller.scrollTop;
    return true;
  };
  const showRef = useRef(show);
  showRef.current = show;

  // Restore the shared position before the first paint.
  useLayoutEffect(() => {
    const initial = link.anchor;
    if (initial && showRef.current(initial)) {
      pinRef.current = initial;
      selfRef.current = initial;
    }
    restoredRef.current = true;
  }, [link]);

  // Report the position after scrolling settles, and once more on unmount
  // (a layout effect, so the elements are still laid out) so a toggle right
  // after a scroll keeps it. A reflow puts a pinned position back.
  useLayoutEffect(() => {
    const scroller = scrollerRef.current;
    const article = articleRef.current;
    if (!scroller || !article) return;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const report = () => {
      timer = null;
      if (!restoredRef.current) return;
      const ref = refOfElement(topmost(scroller, anchorsIn(article)));
      if (!ref) return;
      selfRef.current = ref;
      link.anchor = ref;
    };
    const onScroll = () => {
      if (Math.abs(scroller.scrollTop - shownTopRef.current) > 1) pinRef.current = null;
      if (timer != null) clearTimeout(timer);
      timer = setTimeout(report, REPORT_MS);
    };
    let width = article.offsetWidth;
    const ro = new ResizeObserver(() => {
      if (article.offsetWidth !== width && !pinRef.current) pinRef.current = selfRef.current;
      width = article.offsetWidth;
      if (pinRef.current) showRef.current(pinRef.current);
    });
    ro.observe(article);
    scroller.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      if (timer != null) {
        clearTimeout(timer);
        report();
      }
      ro.disconnect();
      scroller.removeEventListener("scroll", onScroll);
    };
  }, [link]);

  return (
    <div ref={scrollerRef} className="flex-1 overflow-y-auto">
      <article
        ref={articleRef}
        data-selectable
        className="markdown-body mx-auto w-full max-w-4xl px-6 py-5"
        onCopy={copyAsMarkdown}
        onDragStart={dragAsMarkdown}
      >
        <Body text={doc.text} plugins={plugins} components={components} />
      </article>
    </div>
  );
}
