import { memo, useEffect, useState } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeRaw from "rehype-raw";
import rehypeKatex from "rehype-katex";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { hasMath, normalizeMath } from "@/lib/mathMarkdown";
import { LoadingFill } from "@/components/ui/PageParts";
import { readCourseFile } from "@/lib/courseFiles";
import { copyAsMarkdown, dragAsMarkdown } from "@/lib/selectionMarkdown";

const PLAIN = [remarkGfm];
const WITH_MATH = [remarkGfm, remarkMath];
const MATH = [rehypeKatex];
const RAW_MATH = [rehypeRaw, rehypeKatex];

/** KaTeX also accepts math fences and HTML math classes without delimiters. */
export function fileMarkdownPlugins(text: string) {
  return {
    remarkPlugins: hasMath(text) ? WITH_MATH : PLAIN,
    rehypePlugins: text.includes("<") ? RAW_MATH : MATH,
  };
}

export const FileMarkdown = memo(function FileMarkdown({
  relPath, components,
}: {
  relPath: string;
  components: Components;
}) {
  const [text, setText] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    setText(null);
    setErr(null);
    let live = true;
    readCourseFile(relPath)
      .then((value) => live && setText(value))
      .catch((e) => live && setErr(String(e)));
    return () => { live = false; };
  }, [relPath]);

  if (err)
    return (
      <div className="px-6 py-5">
        <Alert variant="destructive">
          <AlertDescription className="text-xs">
            Failed to load file: {err}
          </AlertDescription>
        </Alert>
      </div>
    );
  if (text === null)
    return (
      <LoadingFill />
    );
  const math = hasMath(text);
  return (
    <div className="flex-1 overflow-y-auto">
      <article
        data-selectable
        className="markdown-body mx-auto w-full max-w-4xl px-6 py-5"
        onCopy={copyAsMarkdown}
        onDragStart={dragAsMarkdown}
      >
        <ReactMarkdown
          {...fileMarkdownPlugins(text)}
          components={components}
        >
          {math ? normalizeMath(text) : text}
        </ReactMarkdown>
      </article>
    </div>
  );
});
