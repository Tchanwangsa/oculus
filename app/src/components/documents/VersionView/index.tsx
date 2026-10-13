import { useEffect, useImperativeHandle, useMemo, useRef, type Ref } from "react";
import { EditorView } from "@codemirror/view";

import { diffHunks, diffStats, diffText } from "@/lib/notes/versionDiff";

import { noteHost, type NoteHost } from "../editor/core/host";
import { versionDiff, type VersionDiff } from "./diffDecorations";
import { versionHost, versionState } from "./extensions";
import { goToHunk } from "./navigation";

/**
 * A saved version of a note on the page, read-only, drawn as the note's Live
 * mode draws it, with its changes against `base` marked (`diffDecorations.ts`).
 * Only the text: the host's page scroller scrolls it, and its bar and list
 * belong to the caller. One view for the component's life; a new text or
 * base swaps its state, so scrolling stays the host's.
 */

export interface VersionViewHandle {
  next(): void;
  prev(): void;
}

export interface VersionViewInfo {
  hunks: number;
  current: number | null;
  inserted: number;
  deleted: number;
}

function diffOf(text: string, base: string | null): { diff: VersionDiff | null; inserted: number; deleted: number } {
  if (base == null || base === text) return { diff: null, inserted: 0, deleted: 0 };
  const changes = diffText(base, text);
  const { inserted, deleted } = diffStats(base, text, changes);
  return { diff: { base, changes, hunks: diffHunks(changes, text) }, inserted, deleted };
}

export function VersionView({
  text,
  base,
  host,
  handleRef,
  onInfo,
  onView,
  className,
}: {
  text: string;
  base: string | null;
  host: NoteHost;
  handleRef?: Ref<VersionViewHandle>;
  onInfo?: (info: VersionViewInfo) => void;
  onView?: (view: EditorView | null) => void;
  className?: string;
}) {
  const rootRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const computed = useMemo(() => diffOf(text, base), [text, base]);
  const latest = useRef({ text, computed, host, onInfo, onView });
  latest.current = { text, computed, host, onInfo, onView };

  // Reads refs only, so the effects below need not depend on it.
  const report = (current: number | null) => {
    const { computed: c, onInfo: cb } = latest.current;
    cb?.({ hunks: c.diff?.hunks.length ?? 0, current, inserted: c.inserted, deleted: c.deleted });
  };

  useEffect(() => {
    const { text: t, computed: c, host: h } = latest.current;
    const v = new EditorView({ parent: rootRef.current!, state: versionState(t, c.diff, h) });
    viewRef.current = v;
    latest.current.onView?.(v);
    return () => {
      v.destroy();
      viewRef.current = null;
      latest.current.onView?.(null);
    };
  }, []);

  // A new version or base: a fresh state, its marks and no current hunk.
  useEffect(() => {
    const v = viewRef.current;
    if (!v) return;
    const { host: h } = latest.current;
    if (v.state.doc.toString() !== text || v.state.facet(versionDiff) !== computed.diff) {
      v.setState(versionState(text, computed.diff, h));
    }
    report(null);
  }, [text, computed]);

  useEffect(() => {
    const v = viewRef.current;
    if (v && v.state.facet(noteHost) !== host) v.dispatch({ effects: versionHost.reconfigure(noteHost.of(host)) });
  }, [host]);

  useImperativeHandle(
    handleRef,
    () => ({
      next: () => {
        if (viewRef.current) report(goToHunk(viewRef.current, 1));
      },
      prev: () => {
        if (viewRef.current) report(goToHunk(viewRef.current, -1));
      },
    }),
    [],
  );

  return <div ref={rootRef} className={className} data-selectable />;
}
