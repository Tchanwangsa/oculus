import { useState } from 'react';
import { CircleNotch, Notebook } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import { getSubjects, type DbFile } from '@/lib/db';
import { createNotebookFromPdf } from '@/lib/canvas/notebook';
import { filePagePath, fileTitle } from '@/lib/files/openFile';
import { openBeside } from '@/lib/shell/tabRouters';

/** A PDF's header button: a new notebook on a copy of its pages, opened beside it. The PDF itself is never changed. */
export function WriteOnCopy({ file }: { file: DbFile }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const start = async () => {
    if (busy) return;
    setBusy(true); setError(null);
    try {
      const subject = (await getSubjects()).find(s => s.id === file.subject_id);
      if (!subject) throw new Error("This file's subject is no longer in the library.");
      const copy = await createNotebookFromPdf(subject, file.relative_path, `${fileTitle(file).replace(/\.pdf$/i, '')} (notebook)`);
      openBeside(filePagePath(copy.subject_id, copy.relative_path));
    } catch (problem) {
      setError(problem instanceof Error ? problem.message : String(problem));
    } finally {
      setBusy(false);
    }
  };
  return <>
    {error ? <span role="alert" title={error} className="min-w-0 max-w-64 truncate text-[11px] text-destructive">{error}</span> : null}
    <Button type="button" size="sm" variant="outline" disabled={busy} onClick={() => void start()} title="Make a notebook with a copy of these pages to write on">
      {busy ? <CircleNotch className="animate-spin" aria-hidden /> : <Notebook aria-hidden />}{busy ? 'Copying…' : 'Write on a copy'}
    </Button>
  </>;
}
