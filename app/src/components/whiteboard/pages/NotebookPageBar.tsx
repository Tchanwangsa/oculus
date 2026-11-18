import { CaretLeftIcon, CaretRightIcon, SidebarSimpleIcon } from '@phosphor-icons/react';
import { CanvasIconButton } from '../CanvasToolbar';
import type { NotebookPages } from './PageSidebar';

/** A notebook's bottom bar: the Pages sidebar's toggle and which page is in view, with steps to the pages either side. */
export function NotebookPageBar({ pages, sidebarOpen, onToggleSidebar }: { pages: NotebookPages; sidebarOpen: boolean; onToggleSidebar: () => void }) {
  return <div className="flex items-center rounded-xl border border-border-subtle p-1 shadow-xs canvas-chrome" role="group" aria-label="Notebook pages">
    <CanvasIconButton label={sidebarOpen ? 'Hide pages' : 'Show pages'} active={sidebarOpen} aria-pressed={sidebarOpen} onClick={onToggleSidebar}><SidebarSimpleIcon size={17} /></CanvasIconButton>
    <div className="ml-1 flex items-center border-l border-border-subtle pl-1">
      <CanvasIconButton label="Previous page" onClick={() => pages.go(pages.index - 1)} disabled={pages.index === 0}><CaretLeftIcon size={15} /></CanvasIconButton>
      <span role="status" className="min-w-20 px-1 text-center text-[12px] tabular-nums text-muted-foreground">Page {pages.index + 1} of {pages.count}</span>
      <CanvasIconButton label="Next page" onClick={() => pages.go(pages.index + 1)} disabled={pages.index >= pages.count - 1}><CaretRightIcon size={15} /></CanvasIconButton>
    </div>
  </div>;
}
