import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { ArrowsInSimpleIcon, ArrowSquareOutIcon, DownloadSimpleIcon, FileImageIcon, FileSvgIcon, GridFourIcon, MinusIcon, PlusIcon, QuestionIcon, SpinnerGapIcon } from '@phosphor-icons/react';
import { CANVAS_DEFAULTS, textAlignOf, type CanvasDocument } from '@/lib/canvas/model';
import { loadCanvasEngine } from '@/lib/canvas/engine';
import { exportCanvasPng } from '@/lib/canvas/render';
import { exportCanvasSvg } from '@/lib/canvas/renderSvg';
import { CANVAS_FONT_STACKS, labelBox } from '@/lib/canvas/renderText';
import { useCanvasInteraction, type TextDraft } from '@/hooks/canvas/useCanvasInteraction';
import { Button } from '@/components/ui/button';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { shortcut } from '@/lib/platform';
import { cn } from '@/lib/utils';
import { invoke } from '@tauri-apps/api/core';
import { base64 } from '@/lib/harness/attachments';
import { CanvasHistory, CanvasIconButton, CanvasToolbar } from './CanvasToolbar';
import { CanvasStylePanel } from './CanvasStylePanel';
import './canvas.css';

async function saveExport(blob: Blob, format: 'png' | 'svg') {
  if ('__TAURI_INTERNALS__' in window) {
    await invoke<string | null>('export_canvas_image', { format, data: await base64(blob) });
    return;
  }
  const url = URL.createObjectURL(blob);
  const anchor = window.document.createElement('a'); anchor.href = url; anchor.download = `Canvas.${format}`; anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** Standalone text grows with its content; a label is fixed to its shape's label box, wraps, and sits per its vertical alignment. */
function CanvasTextInput({ draft, viewport, onInput, onCommit, onCancel }: { draft: TextDraft; viewport: CanvasDocument['viewport']; onInput: (text: string) => void; onCommit: (text: string) => void; onCancel: () => void }) {
  const input = useRef<HTMLTextAreaElement>(null);
  const finished = useRef(false);
  const { element } = draft, zoom = viewport.zoom;
  const box = draft.kind === 'label' ? labelBox(element) : null;
  const resize = () => {
    const field = input.current;
    if (!field) return;
    field.style.height = 'auto';
    if (!box) { field.style.width = '1px'; field.style.width = `${field.scrollWidth + 2}px`; }
    const height = field.scrollHeight;
    field.style.height = `${height}px`;
    if (!box) return;
    // A label taller than its box starts at the box's top, as the shape grows downward to fit it on commit.
    const top = (element.y + box.y) * zoom + viewport.y, room = box.height * zoom, align = element.verticalAlign ?? CANVAS_DEFAULTS.verticalAlign;
    field.style.top = `${height >= room || align === 'top' ? top : align === 'bottom' ? top + room - height : top + (room - height) / 2}px`;
  };
  useLayoutEffect(resize, [draft, viewport]);
  useEffect(() => { input.current?.focus(); if (draft.existing && draft.text) input.current?.select(); }, [draft.element.id, draft.existing, draft.text]);
  const commit = () => { if (!finished.current) { finished.current = true; onCommit(input.current?.value ?? draft.text); } };
  const position = box
    ? { left: (element.x + box.x) * zoom + viewport.x, width: Math.max(box.width * zoom, 1) }
    : { left: element.x * zoom + viewport.x, top: element.y * zoom + viewport.y };
  return <textarea ref={input} aria-label={box ? 'Shape label' : 'Canvas text'} data-selectable="true" className={cn('canvas-text-input absolute z-20', box && 'canvas-label-input')} rows={1} defaultValue={draft.text}
    onInput={event => { resize(); onInput(event.currentTarget.value); }}
    style={{ ...position, color: element.stroke, fontFamily: CANVAS_FONT_STACKS[element.fontFamily ?? CANVAS_DEFAULTS.fontFamily], fontSize: (element.fontSize ?? CANVAS_DEFAULTS.fontSize) * zoom, textAlign: textAlignOf(element) }}
    onBlur={commit} onKeyDown={event => {
      event.stopPropagation();
      if (event.key === 'Escape') { event.preventDefault(); finished.current = true; onCancel(); }
      else if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) { event.preventDefault(); commit(); }
    }} />;
}

function CanvasEmpty({ onStart }: { onStart: () => void }) {
  return <div className="pointer-events-none absolute inset-0 flex items-center justify-center pb-2">
    <div className="flex max-w-72 flex-col items-center text-center">
      <svg width="146" height="87" viewBox="0 0 146 87" fill="none" aria-hidden="true" className="mb-5 text-brand">
        <rect x="17" y="21" width="47" height="41" rx="7" transform="rotate(-7 17 21)" fill="currentColor" fillOpacity=".07" stroke="currentColor" strokeOpacity=".45" strokeWidth="1.5" />
        <path d="M75 42C88 30 94 30 108 38M102 30L109 38L99 40" stroke="currentColor" strokeOpacity=".55" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        <ellipse cx="118" cy="48" rx="20" ry="23" transform="rotate(10 118 48)" fill="currentColor" fillOpacity=".06" stroke="currentColor" strokeOpacity=".4" strokeWidth="1.5" />
        <path d="M38 77C50 68 58 83 71 74" stroke="currentColor" strokeOpacity=".3" strokeWidth="1.5" strokeLinecap="round" />
      </svg>
      <h2 className="text-[18px] font-semibold tracking-tight text-foreground">Room to think.</h2>
      <p className="mt-2 max-w-60 text-[13px] leading-relaxed text-muted-foreground">Sketch an idea, connect the pieces, or make space for something new.</p>
      <Button variant="outline" size="sm" className="pointer-events-auto mt-5 bg-card" onClick={onStart}>Start drawing<span className="ml-2 text-[11px] text-muted-foreground">P</span></Button>
    </div>
  </div>;
}

const hints: Record<string, string> = {
  select: 'Drag to select · Double-click a shape or text to write in it', hand: 'Drag to move around',
  rectangle: 'Drag to draw · Hold Shift for a square', ellipse: 'Drag to draw · Hold Shift for a circle',
  diamond: 'Drag to draw · Hold Shift to constrain', line: 'Drag to draw · Hold Shift to snap angle',
  arrow: `Drag to connect · Hold Shift to snap angle · Hold ${shortcut('mod')} to place freely`, freedraw: 'Draw freely · Escape to select',
  text: `Click anywhere or on a shape to write · ${shortcut('mod+Enter')} to finish`, eraser: 'Drag over shapes to erase',
};

export function CanvasSurface({ document, onChange }: { document: CanvasDocument; onChange: (document: CanvasDocument) => void }) {
  const editor = useCanvasInteraction(document, onChange);
  const [engineError, setEngineError] = useState<string | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  useEffect(() => { let mounted = true; loadCanvasEngine().then(() => { if (mounted) editor.redraw(); }).catch(error => { if (mounted) setEngineError(String(error)); }); return () => { mounted = false; }; }, [editor.redraw]);
  const current = editor.documentRef.current;
  const exportImage = async (format: 'png' | 'svg') => {
    setExporting(true); setExportError(null);
    try {
      const blob = format === 'png' ? await exportCanvasPng(editor.documentRef.current) : new Blob([exportCanvasSvg(editor.documentRef.current)], { type: 'image/svg+xml' });
      if (!blob) throw new Error('Add something to the canvas before exporting.');
      await saveExport(blob, format);
    } catch (error) { setExportError(error instanceof Error ? error.message : String(error)); }
    finally { setExporting(false); }
  };
  return <div ref={editor.root} data-canvas-editor="true" tabIndex={0} aria-label="Infinite canvas editor" className="canvas-editor relative h-full min-h-0 w-full overflow-hidden bg-card outline-none"
    onKeyDown={editor.keyDown} onKeyUp={editor.keyUp} onBlur={editor.blur} onCopy={editor.copy} onCut={editor.cut} onPaste={editor.paste}>
    <canvas ref={editor.canvas} aria-label="Drawing canvas" className="absolute inset-0 block h-full w-full" style={{ cursor: editor.cursor }}
      onPointerDown={editor.pointerDown} onPointerMove={editor.pointerMove} onPointerUp={editor.pointerUp} onPointerCancel={editor.cancelGesture} onPointerLeave={editor.pointerLeave} onDoubleClick={editor.doubleClick} onContextMenu={event => event.preventDefault()} />
    {current.elements.length === 0 && editor.tool === 'select' && !editor.textDraft ? <CanvasEmpty onStart={() => { editor.setTool('freedraw'); editor.root.current?.focus(); }} /> : null}
    <CanvasToolbar tool={editor.tool} onTool={tool => { editor.setTool(tool); editor.root.current?.focus({ preventScroll: true }); }} />
    {editor.inspector.sections.length ? <CanvasStylePanel {...editor.inspector} onChange={editor.changeStyle} onPreview={editor.previewStyle} selectionCount={editor.selection.length} onDelete={editor.deleteSelected} layerMoves={editor.layerMoves} onReorder={editor.reorder} /> : null}
    {editor.textDraft ? <CanvasTextInput key={editor.textDraft.element.id} draft={editor.textDraft} viewport={current.viewport} onInput={editor.editText} onCommit={editor.commitText} onCancel={editor.cancelText} /> : null}
    <div className="canvas-bottom absolute bottom-4 left-4 z-10 flex items-center gap-2">
      <div className="flex items-center rounded-xl border border-border-subtle bg-card p-1 shadow-xs" role="group" aria-label="Canvas zoom">
        <CanvasIconButton label="Zoom out (−)" onClick={() => editor.zoom(1 / 1.2)} disabled={editor.zoomPercent <= 10}><MinusIcon size={15} /></CanvasIconButton>
        <Tooltip><TooltipTrigger asChild><button type="button" className="min-w-12 cursor-pointer rounded-md px-1 py-1.5 text-[12px] tabular-nums text-muted-foreground hover:bg-accent focus-visible:outline-2 focus-visible:outline-brand" aria-label={`Zoom ${editor.zoomPercent} percent; reset to 100 percent`} onClick={editor.resetZoom}>{editor.zoomPercent}%</button></TooltipTrigger><TooltipContent side="top" sideOffset={8}>Reset zoom (0)</TooltipContent></Tooltip>
        <CanvasIconButton label="Zoom in (+)" onClick={() => editor.zoom(1.2)} disabled={editor.zoomPercent >= 800}><PlusIcon size={15} /></CanvasIconButton>
        <div className="canvas-reset-view ml-1 border-l border-border-subtle pl-1"><CanvasIconButton label="Fit drawing (1)" onClick={editor.fit}><ArrowsInSimpleIcon size={17} /></CanvasIconButton></div>
      </div>
      <CanvasHistory undo={() => editor.history()} redo={() => editor.history(true)} canUndo={editor.canUndo} canRedo={editor.canRedo} />
    </div>
    <div className="canvas-short-hint pointer-events-none absolute right-0 bottom-[4.7rem] left-0 text-center text-[11px] text-muted-foreground/65">{hints[editor.tool]}</div>
    <div className="absolute right-4 bottom-4 z-10 flex items-center gap-1.5 rounded-xl border border-border-subtle bg-card p-1 shadow-xs">
      <CanvasIconButton label="Toggle grid (G)" active={editor.grid} aria-pressed={editor.grid} onClick={() => editor.setGrid(value => !value)}><GridFourIcon size={17} /></CanvasIconButton>
      <Popover><PopoverTrigger asChild><Button variant="ghost" size="icon" aria-label="Export drawing" disabled={!current.elements.length || exporting} className="rounded-lg text-muted-foreground">{exporting ? <SpinnerGapIcon className="animate-spin" /> : <DownloadSimpleIcon size={17} />}</Button></PopoverTrigger>
        <PopoverContent side="top" align="end" className="w-52 p-1.5"><div className="px-2.5 py-2 text-xs font-medium">Export drawing</div>
          <Button variant="ghost" className="w-full justify-start rounded-md text-xs" disabled={exporting} onClick={() => void exportImage('png')}><FileImageIcon />PNG image<ArrowSquareOutIcon className="ml-auto text-muted-foreground" /></Button>
          <Button variant="ghost" className="w-full justify-start rounded-md text-xs" disabled={exporting} onClick={() => void exportImage('svg')}><FileSvgIcon />SVG vector<ArrowSquareOutIcon className="ml-auto text-muted-foreground" /></Button>
          <p className="px-2.5 py-2 text-[11px] leading-relaxed text-muted-foreground">Exports the whole drawing without the grid or selection.</p>
        </PopoverContent>
      </Popover>
      <Popover><PopoverTrigger asChild><Button variant="ghost" size="icon" aria-label="Canvas shortcuts" className="rounded-lg text-muted-foreground"><QuestionIcon size={17} /></Button></PopoverTrigger>
        <PopoverContent side="top" align="end" className="w-64"><div className="mb-3 text-[13px] font-medium">A little room to move</div><dl className="space-y-2 text-xs text-muted-foreground">
          {[["Pan", 'Space + drag'], ['Zoom', `${shortcut('mod')} + scroll`], ['Multiple selection', 'Shift + click'], ['Write in a shape', 'Enter'], ['Constrain shape', 'Shift + drag'], ['Place arrow freely', `${shortcut('mod')} + drag`], ['Duplicate', shortcut('mod+D')], ['Layer up / down', `${shortcut('mod+]')} / ${shortcut('mod+[')}`], ['Layer to front / back', `${shortcut('mod+shift+]')} / ${shortcut('mod+shift+[')}`], ['Undo', shortcut('mod+Z')], ['Fit drawing', '1']].map(([label, key]) => <div key={label} className="flex justify-between gap-4"><dt>{label}</dt><dd className="text-foreground">{key}</dd></div>)}
        </dl></PopoverContent>
      </Popover>
    </div>
    {engineError || exportError ? <div role="alert" className="absolute top-[5.5rem] right-4 z-30 max-w-80 rounded-xl border border-destructive/20 bg-card p-3 text-xs text-destructive shadow-sm">{engineError ?? exportError}<button type="button" className="mt-2 block cursor-pointer text-foreground underline" onClick={() => { setEngineError(null); setExportError(null); }}>Dismiss</button></div> : null}
  </div>;
}

export default CanvasSurface;
