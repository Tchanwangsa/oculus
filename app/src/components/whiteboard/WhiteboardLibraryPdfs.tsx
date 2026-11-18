import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react';
import { FilePdfIcon, MagnifyingGlassIcon, SpinnerGapIcon } from '@phosphor-icons/react';
import { Input } from '@/components/ui/input';
import { useScrollFade } from '@/hooks/ui/useScrollFade';
import { useCanvasPlatform, type CanvasLibraryPdf } from '@/lib/canvas/platform';
import { displayCode } from '@/lib/format/format';
import { cn } from '@/lib/utils';

/** The host supplies the library, so the shared canvas never imports desktop SQL. */
export function CanvasLibraryPdfs({ disabled, onChoose }: {
  disabled: boolean;
  onChoose: (file: CanvasLibraryPdf) => void;
}) {
  const platform = useCanvasPlatform();
  const listId = useId();
  const list = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState('');
  const [answer, setAnswer] = useState<{ query: string; files: CanvasLibraryPdf[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [active, setActive] = useState(0);
  const pending = answer?.query !== query;
  const files = pending ? [] : answer.files;
  useScrollFade(list, 'y', answer);

  useEffect(() => {
    let current = true;
    setError(null);
    const timer = window.setTimeout(() => {
      void platform.searchPdfs(query).then(files => {
        if (!current) return;
        setAnswer({ query, files });
        setActive(0);
      }).catch(problem => {
        if (current) setError(problem instanceof Error ? problem.message : String(problem));
      });
    }, query.trim() ? 150 : 0);
    return () => { current = false; window.clearTimeout(timer); };
  }, [platform, query]);

  useEffect(() => {
    list.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.scrollIntoView({ block: 'nearest' });
  }, [active, answer]);

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.nativeEvent.isComposing || disabled) return;
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      if (files.length) setActive(index => (index + (event.key === 'ArrowDown' ? 1 : -1) + files.length) % files.length);
    } else if (event.key === 'Enter') {
      event.preventDefault();
      if (files[active]) onChoose(files[active]);
    }
  };

  return <div className="min-h-0 space-y-2">
    <div className="relative">
      <MagnifyingGlassIcon size={16} aria-hidden className="pointer-events-none absolute top-2 left-3 text-muted-foreground" />
      <Input autoFocus className="pl-9" placeholder="Search PDFs by name or subject…" aria-label="Search library PDFs"
        role="combobox" aria-expanded aria-controls={listId} aria-autocomplete="list"
        aria-activedescendant={files[active] ? `${listId}-${files[active].id}` : undefined}
        value={query} onChange={event => setQuery(event.target.value)} onKeyDown={onKeyDown} disabled={disabled} spellCheck={false} />
    </div>
    <p className="px-1 text-xs text-muted-foreground">{query.trim() ? 'Matching PDFs' : 'PDFs in your library'}</p>
    <div ref={list} id={listId} role="listbox" aria-label="Library PDFs" aria-busy={pending && !error}
      className="max-h-64 overflow-y-scroll rounded-lg border border-border-subtle p-1">
      {files.map((file, index) => <button key={file.id} id={`${listId}-${file.id}`} data-index={index} type="button" role="option"
        aria-selected={index === active} disabled={disabled} onMouseMove={() => setActive(index)} onClick={() => onChoose(file)}
        className={cn('flex w-full cursor-pointer items-center gap-3 rounded-md px-3 py-2 text-left outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-60', index === active ? 'bg-accent' : 'hover:bg-accent')}>
        <FilePdfIcon size={20} aria-hidden className="shrink-0 text-reference-pdf" />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm" title={file.filename}>{file.filename}</span>
          <span className="block truncate text-xs text-muted-foreground">{displayCode(file.subjectCode)}</span>
        </span>
      </button>)}
      {error ? <p role="alert" className="px-3 py-4 text-[13px] text-destructive" data-selectable>{error}</p>
        : pending ? <p role="status" className="flex items-center gap-2 px-3 py-4 text-sm text-muted-foreground"><SpinnerGapIcon className="animate-spin" aria-hidden />Searching library…</p>
          : files.length === 0 ? <p role="status" className="px-3 py-4 text-sm text-muted-foreground">{query.trim() ? 'No PDFs match your search.' : 'No PDFs in your library yet.'}</p> : null}
    </div>
  </div>;
}
