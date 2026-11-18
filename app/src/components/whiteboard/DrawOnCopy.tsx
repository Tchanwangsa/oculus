import { useState } from 'react';
import { CaretDownIcon, CircleNotchIcon, NotebookIcon, PencilSimpleIcon, ScribbleIcon, type Icon } from '@phosphor-icons/react';
import { DropdownMenu } from 'radix-ui';
import { Button } from '@/components/ui/button';
import { getSubjects, type DbFile } from '@/lib/db';
import { createNotebookFromPdf } from '@/lib/whiteboard/notebook';
import { createWhiteboardFromPdf } from '@/lib/whiteboard/pdfCopy';
import type { DrawingKind } from '@/lib/whiteboard/drawingKinds';
import { filePagePath, fileTitle } from '@/lib/files/openFile';
import { openBeside } from '@/lib/shell/tabRouters';

const CHOICES: { kind: DrawingKind; icon: Icon; title: string; body: string }[] = [
  { kind: 'notebook', icon: NotebookIcon, title: 'Notebook', body: 'Write on the pages, like a paper notebook. Each PDF page becomes a page.' },
  { kind: 'whiteboard', icon: ScribbleIcon, title: 'Whiteboard', body: 'An infinite board with the pages laid out to sketch around.' },
];

/**
 * A PDF's header button: a copy of its pages to draw on, as a notebook or a whiteboard, opened beside it.
 * The PDF itself is never changed.
 */
export function DrawOnCopy({ file }: { file: DbFile }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const start = async (kind: DrawingKind) => {
    if (busy) return;
    setBusy(true); setError(null);
    try {
      const subject = (await getSubjects()).find(s => s.id === file.subject_id);
      if (!subject) throw new Error("This file's subject is no longer in the library.");
      const title = `${fileTitle(file).replace(/\.pdf$/i, '')} (${kind})`;
      const copy = kind === 'notebook'
        ? await createNotebookFromPdf(subject, file.relative_path, title)
        : await createWhiteboardFromPdf(subject, file.relative_path, title);
      openBeside(filePagePath(copy.subject_id, copy.relative_path));
    } catch (problem) {
      setError(problem instanceof Error ? problem.message : String(problem));
    } finally {
      setBusy(false);
    }
  };
  return <>
    {error ? <span role="alert" title={error} className="min-w-0 max-w-64 truncate text-[11px] text-destructive">{error}</span> : null}
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button type="button" size="xs" variant="outline" disabled={busy} title="Draw on a copy of these pages">
          {busy ? <CircleNotchIcon className="animate-spin" aria-hidden /> : <PencilSimpleIcon aria-hidden />}
          {busy ? 'Copying…' : 'Draw'}
          {busy ? null : <CaretDownIcon aria-hidden />}
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content align="end" sideOffset={6} className="z-50 w-64 rounded-xl border border-border-subtle bg-popover p-1 text-popover-foreground shadow-lg outline-none">
          {CHOICES.map(({ kind, icon: ChoiceIcon, title, body }) => (
            <DropdownMenu.Item key={kind} onSelect={() => void start(kind)} className="flex cursor-pointer items-start gap-2.5 rounded-lg px-2.5 py-2 outline-none data-highlighted:bg-accent">
              <ChoiceIcon size={15} className="mt-px shrink-0 text-reference-page" aria-hidden />
              <span className="min-w-0">
                <span className="block text-[12px] font-medium text-foreground">{title}</span>
                <span className="mt-0.5 block text-[11px] leading-snug text-muted-foreground">{body}</span>
              </span>
            </DropdownMenu.Item>
          ))}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  </>;
}
