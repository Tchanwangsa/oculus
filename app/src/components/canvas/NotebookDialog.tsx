import { useEffect, useId, useRef, useState } from 'react';
import { CircleNotch, FilePdf, MagnifyingGlass, Warning } from '@phosphor-icons/react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { PillTabs } from '@/components/ui/table/PillTabs';
import { useScrollFade } from '@/hooks/ui/useScrollFade';
import type { DbFile } from '@/lib/db';
import { searchLibraryFiles, type LibraryFileHit } from '@/lib/db/search';
import { createBlankNotebook, createNotebookFromPdf, type NotebookOrientation } from '@/lib/canvas/notebook';
import { PAPER_TEMPLATES, type PaperTemplate } from '@/lib/canvas/paper';
import { cn } from '@/lib/utils';

const MODES = [{ value: 'blank', label: 'Blank paper' }, { value: 'pdf', label: 'From a PDF' }] as const;
type Mode = typeof MODES[number]['value'];
const TEMPLATE_LABELS: Record<PaperTemplate, string> = { blank: 'Blank', grid: 'Grid', ruled: 'Ruled', dotted: 'Dotted' };
const ORIENTATIONS: readonly NotebookOrientation[] = ['portrait', 'landscape'];

/** A small drawing of a page's template: coarse on purpose, since the real pattern is too fine to read at thumbnail size. */
function thumbPath(template: PaperTemplate, width: number, height: number): string {
  let d = '';
  if (template === 'grid') {
    for (let x = 10; x < width; x += 10) d += `M${x} 0V${height}`;
    for (let y = 10; y < height; y += 10) d += `M0 ${y}H${width}`;
  } else if (template === 'ruled') {
    for (let y = 18; y < height; y += 9) d += `M0 ${y}H${width}`;
  } else if (template === 'dotted') {
    for (let x = 10; x < width; x += 10) for (let y = 10; y < height; y += 10) d += `M${x} ${y}h0`;
  }
  return d;
}

function PaperThumb({ template, orientation }: { template: PaperTemplate; orientation: NotebookOrientation }) {
  const [width, height] = orientation === 'portrait' ? [60, 84] : [84, 60];
  return <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} aria-hidden="true" className="rounded-xs bg-white shadow-xs ring-1 ring-border">
    <path d={thumbPath(template, width, height)} fill="none" strokeLinecap="round" strokeWidth={template === 'dotted' ? 1.6 : 0.6} className="stroke-muted-foreground/50" />
  </svg>;
}

/** The subject's PDFs, searchable: what a notebook can start from. */
function PdfChoices({ subjectId, selected, disabled, onSelect }: { subjectId: number; selected: number | null; disabled: boolean; onSelect: (file: LibraryFileHit) => void }) {
  const searchId = useId();
  const list = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState('');
  const [answer, setAnswer] = useState<{ query: string; files: LibraryFileHit[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const pending = answer?.query !== query;
  useScrollFade(list, 'y', answer);

  useEffect(() => {
    let current = true;
    setError(null);
    const timer = window.setTimeout(() => {
      searchLibraryFiles(query, 40, { subjectId, fileType: 'pdf' })
        .then(files => { if (current) setAnswer({ query, files }); })
        .catch(problem => { if (current) setError(problem instanceof Error ? problem.message : String(problem)); });
    }, query.trim() ? 150 : 0);
    return () => { current = false; window.clearTimeout(timer); };
  }, [subjectId, query]);

  const files = pending || !answer ? [] : answer.files;
  return <div className="min-h-0 space-y-2">
    <div className="relative">
      <Label htmlFor={searchId} className="sr-only">Search this subject's PDFs</Label>
      <MagnifyingGlass size={14} aria-hidden className="pointer-events-none absolute top-2.5 left-3 text-muted-foreground" />
      <Input id={searchId} className="pl-9" placeholder="Search this subject's PDFs…" value={query} onChange={event => setQuery(event.target.value)} disabled={disabled} spellCheck={false} />
    </div>
    <div ref={list} role="radiogroup" aria-label="PDFs in this subject" aria-busy={pending && !error} className="max-h-56 overflow-y-auto rounded-lg border border-border-subtle p-1">
      {files.map(file => {
        const chosen = file.id === selected;
        return <button key={file.id} type="button" role="radio" aria-checked={chosen} disabled={disabled} onClick={() => onSelect(file)}
          className={cn('flex w-full cursor-pointer items-center gap-3 rounded-md px-3 py-2 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-60', chosen ? 'bg-brand/12' : 'hover:bg-accent')}>
          <FilePdf size={16} aria-hidden className="shrink-0 text-reference-pdf" />
          <span className="min-w-0 flex-1 truncate text-[12px]" title={file.filename}>{file.filename}</span>
          {chosen ? <span className="shrink-0 text-[11px] text-brand">Selected</span> : null}
        </button>;
      })}
      {error ? <p role="alert" className="px-3 py-4 text-[12px] text-destructive">{error}</p>
        : pending ? <p role="status" className="flex items-center gap-2 px-3 py-4 text-[12px] text-muted-foreground"><CircleNotch className="animate-spin" aria-hidden />Looking through this subject…</p>
          : files.length === 0 ? <p role="status" className="px-3 py-4 text-[12px] text-muted-foreground">{query.trim() ? 'No PDFs match.' : 'This subject has no PDFs yet. Sync it, or add one under Files → Uploads.'}</p> : null}
    </div>
  </div>;
}

const stem = (filename: string) => filename.replace(/\.pdf$/i, '');

/** New notebook: blank paper in a template, or a copy of one of the subject's PDFs to write on. */
export function NotebookDialog({ open, onOpenChange, subject, onCreated }: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  subject: { id: number; code: string };
  onCreated: (file: DbFile) => void;
}) {
  const groupId = useId();
  const nameId = useId();
  const [mode, setMode] = useState<Mode>('blank');
  const [template, setTemplate] = useState<PaperTemplate>('grid');
  const [orientation, setOrientation] = useState<NotebookOrientation>('portrait');
  const [pdf, setPdf] = useState<LibraryFileHit | null>(null);
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState('');
  const [error, setError] = useState<string | null>(null);
  const suggested = mode === 'pdf' && pdf ? `${stem(pdf.filename)} (notebook)` : 'Notebook';

  const changeOpen = (next: boolean) => {
    if (busy) return;
    if (!next) { setError(null); setProgress(''); }
    onOpenChange(next);
  };
  const create = async () => {
    if (busy || (mode === 'pdf' && !pdf)) return;
    setBusy(true); setError(null);
    const title = name.trim() || suggested;
    try {
      const file = mode === 'pdf' && pdf
        ? await createNotebookFromPdf(subject, pdf.relative_path, title, (done, total) => setProgress(`Copying page ${Math.min(done + 1, total)} of ${total}…`))
        : await createBlankNotebook(subject, title, template, orientation);
      onOpenChange(false);
      onCreated(file);
    } catch (problem) {
      setError(problem instanceof Error ? problem.message : String(problem));
    } finally {
      setBusy(false); setProgress('');
    }
  };

  return <Dialog open={open} onOpenChange={changeOpen}>
    <DialogContent className="flex max-h-[calc(100dvh-2rem)] flex-col gap-4 overflow-y-auto sm:max-w-lg" onKeyDown={event => event.stopPropagation()}>
      <DialogHeader>
        <DialogTitle>New notebook</DialogTitle>
        <DialogDescription>Pages you write on, saved as you go. Ask a chat agent to look at your working any time.</DialogDescription>
      </DialogHeader>
      <div className="-ml-2"><PillTabs tabs={MODES} value={mode} onChange={setMode} /></div>

      {mode === 'blank' ? <div className="space-y-4">
        <fieldset disabled={busy} className="space-y-2">
          <legend className="text-[12px] font-medium text-foreground">Paper</legend>
          <div className="flex flex-wrap gap-3">
            {PAPER_TEMPLATES.map(value => <label key={value} className="cursor-pointer">
              <input type="radio" name={`${groupId}-template`} value={value} checked={template === value} onChange={() => setTemplate(value)} className="peer sr-only" />
              <span className="flex flex-col items-center gap-1.5 rounded-lg border border-transparent p-2 transition-colors hover:bg-accent peer-checked:border-brand peer-checked:bg-brand/12 peer-focus-visible:ring-2 peer-focus-visible:ring-ring">
                <PaperThumb template={value} orientation={orientation} />
                <span className="text-[12px]">{TEMPLATE_LABELS[value]}</span>
              </span>
            </label>)}
          </div>
        </fieldset>
        <fieldset disabled={busy} className="space-y-2">
          <legend className="text-[12px] font-medium text-foreground">A4 page</legend>
          <div className="flex gap-2">
            {ORIENTATIONS.map(value => <label key={value} className="cursor-pointer">
              <input type="radio" name={`${groupId}-orientation`} value={value} checked={orientation === value} onChange={() => setOrientation(value)} className="peer sr-only" />
              <span className="flex h-8 items-center rounded-full border border-border-subtle px-3 text-[12px] capitalize transition-colors hover:bg-accent peer-checked:border-brand peer-checked:bg-brand/12 peer-focus-visible:ring-2 peer-focus-visible:ring-ring">{value}</span>
            </label>)}
          </div>
        </fieldset>
      </div> : <div className="space-y-2">
        <p className="text-[12px] text-muted-foreground">Pick an assignment or any PDF. Its pages are copied into the notebook, so the original is never changed.</p>
        <PdfChoices subjectId={subject.id} selected={pdf?.id ?? null} disabled={busy} onSelect={setPdf} />
      </div>}

      <div className="space-y-1.5">
        <Label htmlFor={nameId} className="text-[12px]">Name</Label>
        <Input id={nameId} value={name} placeholder={suggested} onChange={event => setName(event.target.value)} disabled={busy} spellCheck={false}
          onKeyDown={event => { if (event.key === 'Enter' && !event.nativeEvent.isComposing) { event.preventDefault(); void create(); } }} />
      </div>

      {error ? <Alert variant="warning" className="w-auto px-2.5 py-2"><Warning aria-hidden /><AlertDescription className="text-[11px] leading-snug">{error}</AlertDescription></Alert> : null}
      <DialogFooter className="items-center">
        {progress ? <span role="status" className="mr-auto text-[12px] text-muted-foreground">{progress}</span> : null}
        <Button variant="outline" disabled={busy} onClick={() => changeOpen(false)}>Cancel</Button>
        <Button disabled={busy || (mode === 'pdf' && !pdf)} onClick={() => void create()}>
          {busy ? <CircleNotch className="animate-spin" aria-hidden /> : null}{busy ? 'Creating…' : 'Create notebook'}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>;
}
