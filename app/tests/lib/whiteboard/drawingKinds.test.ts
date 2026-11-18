import { describe, expect, test } from 'bun:test';
import type { DbFile } from '../../../src/lib/db';
import { drawingKindOf, drawingKindOfPath, planDrawingRows } from '../../../src/lib/whiteboard/drawingKinds';

const row = (id: number, category: string, relative_path: string, size_bytes = 10): DbFile => ({
  id, subject_id: 1, filename: relative_path.split('/').pop()!, relative_path, file_type: category, size_bytes, category,
  canvas_id: null, source_url: null, modified_at: null, scraped_at: '', parse_status: null, parsed_at: null,
  embed_status: null, embedded_at: null, first_seen_at: null, last_accessed_at: null,
} as DbFile);
const disk = (relative_path: string, size_bytes = 10, file_type = 'whiteboard') =>
  ({ filename: relative_path.split('/').pop()!, relative_path, file_type, size_bytes });

describe('drawing kinds', () => {
  test('the extension names the kind anywhere in the own files, never the contents', () => {
    expect(drawingKindOfPath('courses/X/Week 3.notebook')).toBe('notebook');
    expect(drawingKindOfPath('courses/X/Exam prep/Old notebook.whiteboard')).toBe('whiteboard');
    for (const path of ['courses/X/Course/Files/a.notebook', 'courses/X/.oculus/a.whiteboard', 'courses/X/.notebook', 'courses/X/.drafts/a.whiteboard', 'courses/X/a.md', 'notebooks/a.notebook']) {
      expect(drawingKindOfPath(path)).toBeNull();
    }
    expect(drawingKindOf({ category: 'notebook' })).toBe('notebook');
    expect(drawingKindOf({ category: 'whiteboard' })).toBe('whiteboard');
    expect(drawingKindOf({ category: 'document' })).toBeNull();
  });
});

describe('planDrawingRows', () => {
  const whiteboardRow = row(1, 'whiteboard', 'courses/X/Board.whiteboard');
  const legacyRow = row(2, 'whiteboard', 'courses/X/Old notebook.whiteboard');
  const notebookRow = row(3, 'notebook', 'courses/X/Week 3.notebook');
  const noteRow = row(4, 'document', 'courses/X/Notes.md');
  const rows = [whiteboardRow, legacyRow, notebookRow, noteRow];

  test('a notebook reconcile never removes a whiteboard row, even with no notebooks on disk', () => {
    const plan = planDrawingRows('notebook', [], rows);
    expect(plan.remove).toEqual([notebookRow]);
    expect(plan.add).toEqual([]);
  });

  test('a whiteboard reconcile never removes a notebook row', () => {
    const plan = planDrawingRows('whiteboard', [], rows);
    expect(plan.remove.map(r => r.id)).toEqual([1, 2]);
  });

  test('each kind adds and resizes only its own files', () => {
    const notebooks = planDrawingRows('notebook', [
      disk('courses/X/Week 3.notebook', 99, 'notebook'),
      disk('courses/X/Week 4.notebook', 5, 'notebook'),
      disk('courses/X/Board.whiteboard'),
    ], rows);
    expect(notebooks.add.map(f => f.relative_path)).toEqual(['courses/X/Week 4.notebook']);
    expect(notebooks.touch).toEqual([{ row: notebookRow, size: 99 }]);
    expect(notebooks.remove).toEqual([]);

    const whiteboards = planDrawingRows('whiteboard', [disk('courses/X/Board.whiteboard'), disk('courses/X/Old notebook.whiteboard')], rows);
    expect(whiteboards).toEqual({ add: [], touch: [], remove: [] });
  });
});
