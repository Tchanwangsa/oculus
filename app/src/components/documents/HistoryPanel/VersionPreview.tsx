import { memo } from "react";
import ReactMarkdown from "react-markdown";

import { fileMarkdownPlugins } from "@/components/files/FileMarkdown";
import { useLibraryMdComponents } from "@/components/files/FileViewer";
import type { DbFile } from "@/lib/db";
import { openFileSmart } from "@/lib/files/openFile";
import { hasMath, normalizeMath } from "@/lib/markdown/math";
import { useMathsReady } from "@/lib/maths";
import { copyAsMarkdown, dragAsMarkdown } from "@/lib/markdown/selection";

import { FRONTMATTER } from "./constants";

/** A version's text as the library's read-only markdown, scaled to the panel
 *  (`.md-compact`); links and pictures resolve against the note's folder. */
export const VersionPreview = memo(function VersionPreview({
  text,
  file,
  files,
}: {
  text: string;
  file: DbFile;
  files: DbFile[];
}) {
  const components = useLibraryMdComponents(file, files, openFileSmart);
  useMathsReady();
  const front = FRONTMATTER.exec(text);
  const source = front ? "~~~yaml\n" + front[1] + "\n~~~\n\n" + text.slice(front[0].length) : text;
  if (!source.trim()) {
    return <p className="text-[12px] text-muted-foreground">This version is empty.</p>;
  }
  return (
    <article data-selectable className="md-compact" onCopy={copyAsMarkdown} onDragStart={dragAsMarkdown}>
      <ReactMarkdown {...fileMarkdownPlugins(source)} components={components}>
        {hasMath(source) ? normalizeMath(source) : source}
      </ReactMarkdown>
    </article>
  );
});
