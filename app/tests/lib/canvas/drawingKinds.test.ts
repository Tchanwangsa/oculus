import { describe, expect, test } from 'bun:test';
import type { DbFile } from '../../../src/lib/db';
import { drawingKindOf, drawingKindOfPath, planDrawingRows } from '../../../src/lib/canvas/drawingKinds';

const row = (id: number, category: string, relative_path: string, size_bytes = 10): DbFile => ({
  id, subject_id: 1, filename: relative_path.split('/').pop()!, relative_path, file_type: category, size_bytes, category,
  canvas_id: null, source_url: null, modified_at: null, scraped_at: '', parse_status: null, parsed_at: null,
  embed_status: null, embedded_at: null, first_seen_at: null, last_accessed_at: null,
} as DbFile);
const disk = (relative_path: string, size_bytes = 10, file_type = 'canvas') =>
  ({ filename: relative_path.split('/').pop()!, relative_path, file_type, size_bytes });

describe('drawing kinds', () => {
  test('the folder and the extension name the kind, never the contents', () => {
    expect(drawingKindOfPath('courses/X/notebooks/Week 3.notebook')).toBe('notebook');
    expect(drawingKindOfPath('courses/X/canvases/Old notebook.canvas')).toBe('canvas');
    for (const path of ['courses/X/canvases/a.notebook', 'courses/X/notebooks/a.canvas', 'courses/X/notebooks/.notebook', 'courses/X/documents/a.md', 'notebooks/a.notebook']) {
      expect(drawingKindOfPath(path)).toBeNull();
    }
    expect(drawingKindOf({ category: 'notebook' })).toBe('notebook');
    expect(drawingKindOf({ category: 'canvas' })).toBe('canvas');
    expect(drawingKindOf({ category: 'document' })).toBeNull();
  });
});

describe('planDrawingRows', () => {
  const canvasRow = row(1, 'canvas', 'courses/X/canvases/Board.canvas');
  const legacyRow = row(2, 'canvas', 'courses/X/canvases/Old notebook.canvas');
  const notebookRow = row(3, 'notebook', 'courses/X/notebooks/Week 3.notebook');
  const noteRow = row(4, 'document', 'courses/X/documents/Notes.md');
  const rows = [canvasRow, legacyRow, notebookRow, noteRow];

  test('a notebook reconcile never removes a canvas row, even with no notebooks on disk', () => {
    const plan = planDrawingRows('notebook', [], rows);
    expect(plan.remove).toEqual([notebookRow]);
    expect(plan.add).toEqual([]);
  });

  test('a canvas reconcile never removes a notebook row', () => {
    const plan = planDrawingRows('canvas', [], rows);
    expect(plan.remove.map(r => r.id)).toEqual([1, 2]);
  });

  test('each kind adds and resizes only its own files', () => {
    const notebooks = planDrawingRows('notebook', [
      disk('courses/X/notebooks/Week 3.notebook', 99, 'notebook'),
      disk('courses/X/notebooks/Week 4.notebook', 5, 'notebook'),
      disk('courses/X/canvases/Board.canvas'),
    ], rows);
    expect(notebooks.add.map(f => f.relative_path)).toEqual(['courses/X/notebooks/Week 4.notebook']);
    expect(notebooks.touch).toEqual([{ row: notebookRow, size: 99 }]);
    expect(notebooks.remove).toEqual([]);

    const canvases = planDrawingRows('canvas', [disk('courses/X/canvases/Board.canvas'), disk('courses/X/canvases/Old notebook.canvas')], rows);
    expect(canvases).toEqual({ add: [], touch: [], remove: [] });
  });
});
