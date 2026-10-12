import { lazy, Suspense } from "react";

import { LoadingFill } from "@/components/ui/layout/PageParts";

import type { CodeEditorProps } from "./CodeEditor";

/** On demand: it brings CodeMirror, which a file page otherwise loads only
 *  for a note. */
const CodeEditor = lazy(() => import("./CodeEditor").then((m) => ({ default: m.CodeEditor })));

/** `CodeEditor`, loaded the first time one shows. */
export function LazyCodeEditor(props: CodeEditorProps) {
  return (
    <Suspense fallback={<LoadingFill />}>
      <CodeEditor {...props} />
    </Suspense>
  );
}
