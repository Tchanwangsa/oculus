import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { ArrowsInSimpleIcon, ArrowSquareOutIcon, DownloadSimpleIcon, FileImageIcon, FilePdfIcon, FileSvgIcon, GridFourIcon, MinusIcon, PlusIcon, QuestionIcon, SpinnerGapIcon } from '@phosphor-icons/react';
import { WHITEBOARD_DEFAULTS, textAlignOf, type WhiteboardDocument } from '@/lib/whiteboard/model';
import { loadWhiteboardEngine } from '@/lib/whiteboard/engine';
import { installBrowserSelectionGuard } from '@/lib/whiteboard/browserSelectionGuard';
import { loadWhiteboardToolPreferences, saveWhiteboardToolPreferences, type WhiteboardToolPreferences } from '@/lib/whiteboard/toolPreferences';
import { exportWhiteboardPng, renderPagePng } from '@/lib/whiteboard/render';
import { notebookPaper, pageRects, type WhiteboardKind } from '@/lib/whiteboard/paper';
import { exportWhiteboardSvg } from '@/lib/whiteboard/renderSvg';
import { subscribeWhiteboardImages } from '@/lib/whiteboard/renderImages';
import { WHITEBOARD_FONT_STACKS, labelBox } from '@/lib/whiteboard/renderText';
import { useWhiteboardInteraction, type TextDraft } from '@/hooks/whiteboard/useWhiteboardInteraction';
import { useWhiteboardMediaDrop } from '@/hooks/whiteboard/useWhiteboardMediaDrop';
import { Button } from '@/components/ui/button';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { shortcut } from '@/lib/platform';
import { cn } from '@/lib/utils';
import { useWhiteboardPlatform } from '@/lib/whiteboard/platform';
import { WhiteboardHistory, WhiteboardIconButton, WhiteboardToolbar } from './WhiteboardToolbar';
import { WhiteboardStyleBar } from './WhiteboardStyleBar';
import { WhiteboardCaptureHold } from './WhiteboardCaptureHold';
import { WhiteboardRegionActions } from './WhiteboardRegionActions';
import { NotebookPageBar } from './pages/NotebookPageBar';
import { PageSidebar } from './pages/PageSidebar';
import { WhiteboardImageDialog } from './WhiteboardImageDialog';
import './whiteboard.css';

/** Standalone text grows with its content; a label is fixed to its shape's label box, wraps, and sits per its vertical alignment. */
function WhiteboardTextInput({ draft, viewport, onInput, onCommit }: { draft: TextDraft; viewport: WhiteboardDocument['viewport']; onInput: (text: string) => void; onCommit: (text: string) => void }) {
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
    const top = (element.y + box.y) * zoom + viewport.y, room = box.height * zoom, align = element.verticalAlign ?? WHITEBOARD_DEFAULTS.verticalAlign;
    field.style.top = `${height >= room || align === 'top' ? top : align === 'bottom' ? top + room - height : top + (room - height) / 2}px`;
  };
  useLayoutEffect(resize, [draft, viewport]);
  useEffect(() => { input.current?.focus(); if (draft.existing && draft.text) input.current?.select(); }, [draft.element.id, draft.existing, draft.text]);
  const commit = () => { if (!finished.current) { finished.current = true; onCommit(input.current?.value ?? draft.text); } };
  const position = box
    ? { left: (element.x + box.x) * zoom + viewport.x, width: Math.max(box.width * zoom, 1) }
    : { left: element.x * zoom + viewport.x, top: element.y * zoom + viewport.y };
  return <textarea ref={input} aria-label={box ? 'Shape label' : 'Whiteboard text'} data-selectable="true" className={cn('whiteboard-text-input absolute z-20', box && 'whiteboard-label-input')} rows={1} defaultValue={draft.text}
    onInput={event => { resize(); onInput(event.currentTarget.value); }}
    style={{ ...position, color: element.stroke, opacity: element.opacity ?? WHITEBOARD_DEFAULTS.opacity, fontFamily: WHITEBOARD_FONT_STACKS[element.fontFamily ?? WHITEBOARD_DEFAULTS.fontFamily], fontSize: (element.fontSize ?? WHITEBOARD_DEFAULTS.fontSize) * zoom, textAlign: textAlignOf(element) }}
    onKeyDown={event => {
      event.stopPropagation();
      if (event.key === 'Escape' || (event.key === 'Enter' && (event.metaKey || event.ctrlKey))) { event.preventDefault(); commit(); }
    }} />;
}

function WhiteboardEmpty({ onStart }: { onStart: () => void }) {
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

/** The shortcuts popover's rows; the tools' own keys are on the rail. */
const shortcuts = (notebook: boolean): [string, string][] => [
  ['Pan', 'Space + drag'], ['Zoom', `${shortcut('mod')} + scroll`], ['Multiple selection', 'Shift + click'], ['Write in a shape', 'Enter'], ['Constrain shape', 'Shift + drag'],
  ['Scale selection', 'Drag a corner'], ['Stretch selection', 'Drag an edge'], ['Scale from an edge', 'Shift + drag edge'], ['Place arrow freely', `${shortcut('mod')} + drag`],
  ['Duplicate', shortcut('mod+D')], ['Layer up / down', `${shortcut('mod+]')} / ${shortcut('mod+[')}`], ['Layer to front / back', `${shortcut('mod+shift+]')} / ${shortcut('mod+shift+[')}`],
  ['Undo', shortcut('mod+Z')], ...(notebook ? [['Fit page width', '1']] as [string, string][] : [['Fit drawing', '1'], ['Toggle grid', shortcut('shift+G')]] as [string, string][]),
];

const hints: Record<string, string> = {
  select: 'Drag to select · Double-click an image to crop', lasso: 'Draw a loop round what you want · Drag inside the selection to move it', capture: 'Drag an area · Adjust its edges · ' + shortcut('mod+C') + ' to copy', crop: 'Drag edges or corners to crop · Enter to apply · Escape to cancel', hand: 'Drag to move around',
  rectangle: 'Drag to draw · Hold Shift for a square', ellipse: 'Drag to draw · Hold Shift for a circle',
  diamond: 'Drag to draw · Hold Shift to constrain', line: 'Drag to draw · Hold Shift to snap angle',
  arrow: `Drag to connect · Hold Shift to snap angle · Hold ${shortcut('mod')} to place freely`, freedraw: 'Draw freely · Escape to select', highlighter: 'Highlight freely · Escape to select',
  text: `Click anywhere or on a shape to write · Escape or ${shortcut('mod+Enter')} to finish`, eraser: 'Drag over ink and shapes to erase · Escape to cancel · Select an image and use the trash to delete it',
};

/**
 * `onViewportChange` keeps pan and zoom with the host instead of saving them in the document. `kind="notebook"` binds the
 * document to its pages (`useWhiteboardInteraction`) and adds the page tools; a `whiteboard` ignores any `paper` it holds.
 */
export function WhiteboardSurface({ document, onChange, onViewportChange, onPageChange, title, kind = 'whiteboard', suppressNativeGestures = false, cameraKey }: { document: WhiteboardDocument; onChange: (document: WhiteboardDocument) => void; onViewportChange?: (viewport: WhiteboardDocument['viewport']) => void; onPageChange?: (index: number) => void; title?: string; kind?: WhiteboardKind; suppressNativeGestures?: boolean; cameraKey?: string }) {
  const platform = useWhiteboardPlatform();
  const [toolPreferences, setToolPreferences] = useState(loadWhiteboardToolPreferences);
  const [imageDialogOpen, setImageDialogOpen] = useState(false);
  const editor = useWhiteboardInteraction(document, onChange, { onViewportChange, ...toolPreferences, kind, cameraKey, onInsertImage: () => setImageDialogOpen(true) });
  const media = useWhiteboardMediaDrop(editor.root, editor.canvas, editor.documentRef, editor.insertImages, editor.notebook);
  // Saved on this device only; collaborators never see them.
  const changeToolPreferences = (patch: Partial<WhiteboardToolPreferences>) => {
    const next = { ...toolPreferences, ...patch };
    setToolPreferences(next); saveWhiteboardToolPreferences(next);
  };
  useEffect(() => { const element = editor.root.current; return element ? installBrowserSelectionGuard(element, { suppressNativeGestures, touchEventRoot: suppressNativeGestures ? element.ownerDocument.body : element }) : undefined; }, [editor.root, suppressNativeGestures]);
  const [engineError, setEngineError] = useState<string | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [pagesOpen, setPagesOpen] = useState(false);
  useEffect(() => subscribeWhiteboardImages(editor.redraw), [editor.redraw]);
  const pageInView = editor.pages?.index;
  useEffect(() => { if (pageInView !== undefined) onPageChange?.(pageInView); }, [pageInView, onPageChange]);
  useEffect(() => { let mounted = true; loadWhiteboardEngine().then(() => { if (mounted) editor.redraw(); }).catch(error => { if (mounted) setEngineError(String(error)); }); return () => { mounted = false; }; }, [editor.redraw]);
  const current = editor.documentRef.current;
  const hint = editor.tool === 'select' && editor.selection.length ? 'Corners scale proportionally · Edges stretch · Shift + edge scales proportionally' : hints[editor.tool];
  const exportImage = async (format: 'png' | 'svg') => {
    setExporting(true); setExportError(null);
    try {
      const options = { notebook: editor.notebook };
      const blob = format === 'png' ? await exportWhiteboardPng(editor.documentRef.current, options) : new Blob([exportWhiteboardSvg(editor.documentRef.current, undefined, options)], { type: 'image/svg+xml' });
      if (!blob) throw new Error('Add something to the whiteboard before exporting.');
      await platform.saveExport(blob, format);
    } catch (error) { setExportError(error instanceof Error ? error.message : String(error)); }
    finally { setExporting(false); }
  };
  /** A notebook's pages, each drawn on white at about 180 dpi, one PDF page apiece. */
  const exportPdf = async () => {
    const exportPages = platform.exportPdf, snapshot = editor.documentRef.current, paper = notebookPaper(snapshot, editor.notebook);
    if (!exportPages || !paper) return;
    setExporting(true); setExportError(null);
    try {
      const pages = [];
      for (const rect of pageRects(paper)) {
        const png = await renderPagePng(snapshot, rect.id, 2.5);
        if (!png) throw new Error('A page could not be drawn for the PDF.');
        pages.push({ width: rect.width, height: rect.height, png });
      }
      await exportPages(title ?? 'Notebook', pages);
    } catch (error) { setExportError(error instanceof Error ? error.message : String(error)); }
    finally { setExporting(false); }
  };
  const focusEditor = () => editor.root.current?.focus({ preventScroll: true });
  const showStyle = editor.inspector.sections.length > 0 || editor.tool === 'eraser' || !!editor.inkPresets;
  const divider = <span aria-hidden="true" className="h-5 w-px shrink-0 bg-border-subtle" />;
  // The rail and the page sidebar sit beside the editor root, so its keys, clipboard and focus stay the drawing's.
  return <div className="whiteboard-frame relative flex h-full min-h-0 w-full bg-card">
    <WhiteboardToolbar tool={editor.tool} onTool={tool => { editor.setTool(tool); focusEditor(); }} onAddImage={() => setImageDialogOpen(true)} onInsert={template => { editor.insertTemplate(template); focusEditor(); }} />
    {editor.pages && pagesOpen ? <PageSidebar pages={editor.pages} document={current} onClose={() => setPagesOpen(false)} /> : null}
    {/* Shown only where the sidebar lies over the drawing (`whiteboard.css`), so it closes like a drawer. */}
    {editor.pages && pagesOpen ? <div aria-hidden="true" className="whiteboard-page-scrim" onPointerDown={() => setPagesOpen(false)} /> : null}
    <div ref={editor.root} data-whiteboard-editor="true" tabIndex={0} aria-label="Whiteboard editor" className="whiteboard-editor relative flex h-full min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-card outline-none"
      onKeyDown={editor.keyDown} onKeyUp={editor.keyUp} onBlur={event => {
        editor.blur();
        // Inspector fields and their portaled pickers belong to the open text edit.
        if (!(event.relatedTarget instanceof Element && event.relatedTarget.closest('.whiteboard-text-input, [data-whiteboard-inspector]'))) editor.commitText();
      }} onCopy={editor.copy} onCut={editor.cut} onPaste={event => { media.paste(event); if (!event.defaultPrevented) editor.paste(event); }}>
      <div className="whiteboard-bar flex h-12 shrink-0 items-center gap-2.5 border-b border-border-subtle bg-card px-2.5">
        {showStyle ? <WhiteboardStyleBar {...editor.inspector} onChange={editor.changeStyle} onPreview={editor.previewStyle} selectionCount={editor.selection.length} onDelete={editor.deleteSelected} onCrop={editor.canCrop ? () => editor.startCrop() : undefined} layerMoves={editor.layerMoves} onReorder={editor.reorder}
          presets={editor.inkPresets ? { items: editor.inkPresets.items, active: editor.inkPresets.active, onSelect: editor.inkPresets.select, label: editor.tool === 'highlighter' ? 'Highlighter' : 'Pen' } : null}
          eraser={editor.tool === 'eraser' ? { preferences: toolPreferences, onChange: changeToolPreferences } : undefined} /> : null}
        <div className="ml-auto flex shrink-0 items-center gap-2.5" role="toolbar" aria-label="History and view">
          <WhiteboardHistory undo={() => editor.history()} redo={() => editor.history(true)} canUndo={editor.canUndo} canRedo={editor.canRedo} />
          {divider}
          <div className="flex shrink-0 items-center gap-0.5" role="group" aria-label="Whiteboard zoom">
            <WhiteboardIconButton label="Zoom out (−)" onClick={() => editor.zoom(1 / 1.2)} disabled={!editor.canZoomOut}><MinusIcon size={15} /></WhiteboardIconButton>
            <Tooltip><TooltipTrigger asChild><button type="button" className="whiteboard-bar-zoom min-w-12 cursor-pointer rounded-md px-1 py-1.5 text-[12px] tabular-nums text-muted-foreground hover:bg-accent focus-visible:outline-2 focus-visible:outline-brand" aria-label={`Zoom ${editor.zoomPercent} percent; reset to 100 percent`} onClick={editor.resetZoom}>{editor.zoomPercent}%</button></TooltipTrigger><TooltipContent side="bottom" sideOffset={8}>Reset zoom (0)</TooltipContent></Tooltip>
            <WhiteboardIconButton label="Zoom in (+)" onClick={() => editor.zoom(1.2)} disabled={!editor.canZoomIn}><PlusIcon size={15} /></WhiteboardIconButton>
            <WhiteboardIconButton label={editor.notebook ? 'Fit page width (1)' : 'Fit drawing (1)'} onClick={editor.fit} className="whiteboard-bar-zoom"><ArrowsInSimpleIcon size={17} /></WhiteboardIconButton>
          </div>
          {divider}
          <div className="flex shrink-0 items-center gap-0.5">
            {editor.notebook ? null : <WhiteboardIconButton label={`Toggle grid (${shortcut('shift+G')})`} active={editor.grid} aria-pressed={editor.grid} onClick={() => editor.setGrid(value => !value)}><GridFourIcon size={17} /></WhiteboardIconButton>}
            <Popover><PopoverTrigger asChild><Button variant="ghost" size="icon" aria-label="Export drawing" disabled={(!current.elements.length && !editor.notebook) || exporting} className="rounded-lg text-muted-foreground">{exporting ? <SpinnerGapIcon className="animate-spin" /> : <DownloadSimpleIcon size={17} />}</Button></PopoverTrigger>
              <PopoverContent side="bottom" align="end" className="w-52 p-1.5"><div className="px-2.5 py-2 text-xs font-medium">Export drawing</div>
                <Button variant="ghost" className="w-full justify-start rounded-md text-xs" disabled={exporting} onClick={() => void exportImage('png')}><FileImageIcon />PNG image<ArrowSquareOutIcon className="ml-auto text-muted-foreground" /></Button>
                <Button variant="ghost" className="w-full justify-start rounded-md text-xs" disabled={exporting} onClick={() => void exportImage('svg')}><FileSvgIcon />SVG vector<ArrowSquareOutIcon className="ml-auto text-muted-foreground" /></Button>
                {editor.notebook && platform.exportPdf ? <Button variant="ghost" className="w-full justify-start rounded-md text-xs" disabled={exporting} onClick={() => void exportPdf()}><FilePdfIcon />PDF document<ArrowSquareOutIcon className="ml-auto text-muted-foreground" /></Button> : null}
                <p className="px-2.5 py-2 text-[11px] leading-relaxed text-muted-foreground">{editor.notebook ? 'Exports every page, without the selection. The PDF has one page for each notebook page.' : 'Exports the whole drawing without the grid or selection.'}</p>
              </PopoverContent>
            </Popover>
            <Popover><PopoverTrigger asChild><Button variant="ghost" size="icon" aria-label="Whiteboard shortcuts" className="whiteboard-bar-help rounded-lg text-muted-foreground"><QuestionIcon size={17} /></Button></PopoverTrigger>
              <PopoverContent side="bottom" align="end" className="w-64"><div className="mb-3 text-[13px] font-medium">A little room to move</div><dl className="space-y-2 text-xs text-muted-foreground">
                {shortcuts(editor.notebook).map(([label, key]) => <div key={label} className="flex justify-between gap-4"><dt>{label}</dt><dd className="text-foreground">{key}</dd></div>)}
              </dl></PopoverContent>
            </Popover>
          </div>
        </div>
      </div>
      <div className="whiteboard-stage relative min-h-0 flex-1 overflow-hidden">
        <canvas ref={editor.canvas} draggable={false} aria-label="Drawing canvas" className="absolute inset-0 block h-full w-full" style={{ cursor: editor.cursor }}
          onDragEnter={media.dragOver} onDragOver={media.dragOver} onDragLeave={media.dragLeave} onDrop={media.drop}
          onPointerDown={editor.pointerDown} onPointerMove={editor.pointerMove} onPointerUp={editor.pointerUp} onPointerCancel={editor.pointerCancel} onPointerLeave={editor.pointerLeave} onDoubleClick={editor.doubleClick} onContextMenu={event => event.preventDefault()} />
        <div ref={editor.inkLayer} aria-hidden="true" className="pointer-events-none absolute inset-0">
          <canvas ref={editor.inkCanvas} className="absolute inset-0 block h-full w-full" />
          <canvas ref={editor.tailCanvas} className="absolute inset-0 block h-full w-full" />
        </div>
        {media.preview ? <div className="pointer-events-none absolute inset-0 z-20" aria-label="Image drop preview">
          {media.boxes.length ? media.boxes.map((box, index) => <div key={index} className="absolute border-2 border-dashed border-brand bg-brand/10" style={{ left: box.x * current.viewport.zoom + current.viewport.x, top: box.y * current.viewport.zoom + current.viewport.y, width: box.width * current.viewport.zoom, height: box.height * current.viewport.zoom }}><img src={box.dataUrl} alt="" className="h-full w-full opacity-50" /></div>)
            : <div className="absolute flex h-28 w-44 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-xl border-2 border-dashed border-brand bg-card/90 text-xs text-brand shadow-sm" style={{ left: media.preview.at.x, top: media.preview.at.y }}>Drop images here</div>}
        </div> : null}
        {current.elements.length === 0 && !editor.notebook && editor.tool === 'select' && !editor.textDraft ? <WhiteboardEmpty onStart={() => { editor.setTool('freedraw'); focusEditor(); }} /> : null}
        {editor.region && !editor.captureHold ? <WhiteboardRegionActions key={JSON.stringify(editor.region.bounds)} region={editor.region} viewport={current.viewport} onCopy={editor.copyRegion} onDuplicate={editor.duplicateRegion} copyState={editor.regionCopyState} copyError={editor.regionError} /> : null}
        {editor.captureHold ? <WhiteboardCaptureHold hold={editor.captureHold} viewport={current.viewport} /> : null}
        {editor.textDraft ? <WhiteboardTextInput key={editor.textDraft.element.id} draft={editor.textDraft} viewport={current.viewport} onInput={editor.editText} onCommit={editor.commitText} /> : null}
        {editor.pages ? <div className="whiteboard-pages absolute left-1/2 z-10 -translate-x-1/2"><NotebookPageBar pages={editor.pages} sidebarOpen={pagesOpen} onToggleSidebar={() => setPagesOpen(open => !open)} /></div> : null}
        <div key={hint} className={cn('whiteboard-short-hint pointer-events-none absolute right-0 left-0 text-center text-[11px] text-muted-foreground/65', editor.pages ? 'bottom-[calc(var(--whiteboard-bottom-edge)+3.4rem)]' : 'bottom-(--whiteboard-bottom-edge)')}>{hint}</div>
        {engineError || exportError || editor.historyError || media.error ? <div role="alert" className="absolute top-3 right-3 z-30 max-w-80 rounded-xl border border-destructive/20 bg-card p-3 text-xs text-destructive shadow-sm">{engineError ?? exportError ?? editor.historyError ?? media.error}<button type="button" className="mt-2 block cursor-pointer text-foreground underline" onClick={() => { setEngineError(null); setExportError(null); editor.clearHistoryError(); media.clearError(); }}>Dismiss</button></div> : null}
      </div>
    </div>
    <WhiteboardImageDialog open={imageDialogOpen} onOpenChange={setImageDialogOpen} onImport={editor.insertImages} />
  </div>;
}

export default WhiteboardSurface;
