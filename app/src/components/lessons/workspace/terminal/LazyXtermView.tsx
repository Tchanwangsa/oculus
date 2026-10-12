import { lazy, Suspense } from "react";

import type { XtermViewProps } from "./XtermView";

/** On demand: xterm.js loads with the first drawer that shows one. */
const XtermView = lazy(() => import("./XtermView").then((m) => ({ default: m.XtermView })));

/** `XtermView`, loaded the first time one mounts. Output written before then
 *  waits in its sink. */
export function LazyXtermView(props: XtermViewProps) {
  return (
    <Suspense fallback={null}>
      <XtermView {...props} />
    </Suspense>
  );
}
