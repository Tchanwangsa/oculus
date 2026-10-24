import { describe, expect, test } from 'bun:test';
import { noteNotebookPage, openNotebookContext, registerOpenNotebook } from '../../../src/lib/canvas/openNotebooks';

const a = { path: 'courses/X/canvases/Assignment 1.canvas', title: 'Assignment 1', pages: 4, page: 1 };
const b = { path: 'courses/X/canvases/Notes "quoted" $5.canvas', title: 'Notes', pages: 2, page: 1 };

describe('openNotebookContext', () => {
  test('is null with no notebook open', () => {
    expect(openNotebookContext()).toBeNull();
  });
  test('names the notebook, its page in view and the command that reads it', () => {
    const forget = registerOpenNotebook(1, a);
    const text = openNotebookContext()!;
    expect(text).toContain('Assignment 1');
    expect(text).toContain('page 1 of 4 in view');
    expect(text).toContain('oculus read "courses/X/canvases/Assignment 1.canvas"');
    forget();
    expect(openNotebookContext()).toBeNull();
  });
  test('follows the page in view', () => {
    const forget = registerOpenNotebook(1, a);
    noteNotebookPage(1, 3);
    expect(openNotebookContext()).toContain('page 3 of 4 in view');
    forget();
  });
  test('quotes a path so the shell reads it as one argument', () => {
    const forget = registerOpenNotebook(2, b);
    expect(openNotebookContext()).toContain('oculus read "courses/X/canvases/Notes \\"quoted\\" \\$5.canvas"');
    forget();
  });
  test('lists the notebook touched last first, at most three', () => {
    const forgets = [1, 2, 3, 4].map(id => registerOpenNotebook(id, { ...a, title: `Book ${id}` }));
    noteNotebookPage(2, 2);
    const lines = openNotebookContext()!.split('\n').filter(line => line.startsWith('- '));
    expect(lines).toHaveLength(3);
    expect(lines[0]).toContain('Book 2');
    forgets.forEach(forget => forget());
  });
  test('a page update for a notebook that is not open is ignored', () => {
    noteNotebookPage(99, 2);
    expect(openNotebookContext()).toBeNull();
  });
});
