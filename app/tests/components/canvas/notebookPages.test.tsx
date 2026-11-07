import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { TooltipProvider } from '@/components/ui/tooltip';
import { createCanvasDocument } from '@/lib/canvas/model';
import type { Paper } from '@/lib/canvas/paper';
import { NotebookPageBar } from '@/components/canvas/pages/NotebookPageBar';
import { PageSidebar, type NotebookPages } from '@/components/canvas/pages/PageSidebar';
import { dropTarget } from '@/components/canvas/pages/usePageReorder';
import { staleThumbnails } from '@/components/canvas/pages/usePageThumbnails';

const paper: Paper = { pages: [{ id: 'p1', width: 595, height: 842 }, { id: 'p2', width: 842, height: 595, template: 'grid' }, { id: 'p3', width: 595, height: 842 }] };
const noop = () => {};
const pages: NotebookPages = {
  paper, count: 3, index: 1, go: noop, add: noop, remove: noop, insert: noop, duplicate: noop, move: noop, setTemplate: noop, setSize: noop, insertPdfPages: noop,
};

describe('notebook page controls', () => {
  test('the bar says which page is in view and toggles the sidebar', () => {
    const html = renderToStaticMarkup(<TooltipProvider><NotebookPageBar pages={pages} sidebarOpen={false} onToggleSidebar={noop} /></TooltipProvider>);
    expect(html).toContain('Page 2 of 3');
    expect(html).toContain('aria-label="Show pages"');
    expect(html).toContain('aria-pressed="false"');
  });
  test('the sidebar lists every page at its proportions, marks the one in view, and gives each a menu', () => {
    const html = renderToStaticMarkup(<PageSidebar pages={pages} document={{ ...createCanvasDocument(), paper }} onClose={noop} />);
    expect(html.match(/aria-label="Page \d"/g)).toEqual(['aria-label="Page 1"', 'aria-label="Page 2"', 'aria-label="Page 3"']);
    expect(html.match(/aria-current="page"/g)).toHaveLength(1);
    expect(html).toMatch(/aria-label="Page 2" aria-current="page"/);
    expect(html.match(/options"/g)).toHaveLength(3);
    // The widest page fills the thumbnail width; a portrait page is narrower at the same scale.
    expect(html).toContain('width:148px;height:104.58');
    expect(html).toContain('Add page');
  });
});

describe('reordering', () => {
  test('a drop into the gap either side of the page leaves it alone', () => {
    expect(dropTarget(2, 2)).toBeNull();
    expect(dropTarget(2, 3)).toBeNull();
  });
  test('a drop above or below lands the page in that gap', () => {
    expect(dropTarget(2, 0)).toBe(0);
    expect(dropTarget(0, 3)).toBe(2);
    expect(dropTarget(1, 4)).toBe(3);
  });
});

describe('thumbnails', () => {
  test('only pages whose fingerprint changed are redrawn, nearest the page in view first', () => {
    const keys = new Map([['a', '1'], ['b', '2'], ['c', '3'], ['d', '4']]);
    const cached = new Map([['a', { key: '1' }], ['c', { key: 'old' }]]);
    expect(staleThumbnails(keys, cached, ['a', 'b', 'c', 'd'], 3)).toEqual(['d', 'c', 'b']);
    expect(staleThumbnails(keys, new Map([...keys].map(([id, key]) => [id, { key }])), ['a', 'b', 'c', 'd'], 0)).toEqual([]);
  });
});
