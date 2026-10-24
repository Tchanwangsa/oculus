import { useEffect, useMemo, useState } from 'react';
import type { CanvasDocument } from '@/lib/canvas/model';
import { NotebookSnapshots } from '@/lib/canvas/snapshots';

/**
 * Keeps a notebook's page pictures current for chat agents (`oculus read` hands them over). Returns why the
 * last write failed, or null. A whiteboard has no pages and writes nothing.
 */
export function useNotebookSnapshots(file: { id: number; relative_path: string }, document: CanvasDocument | undefined): string | null {
  const [error, setError] = useState<string | null>(null);
  const writer = useMemo(() => new NotebookSnapshots(file.id, undefined, setError), [file.id]);
  useEffect(() => { if (document) writer.schedule(document, file.relative_path); }, [writer, document, file.relative_path]);
  useEffect(() => () => { void writer.flush().finally(() => writer.dispose()); }, [writer]);
  return error;
}
