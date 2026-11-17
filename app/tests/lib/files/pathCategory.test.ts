import { describe, expect, test } from 'bun:test';
import { pathFile } from '../../../src/lib/files/pathCategory';

describe('pathFile', () => {
  test('each drawing folder maps to its own category, as category_of does', () => {
    expect(pathFile('courses/X/notebooks/Week 3.notebook')).toEqual({ category: 'notebook', filename: 'Week 3.notebook' });
    expect(pathFile('courses/X/canvases/Board.canvas')).toEqual({ category: 'canvas', filename: 'Board.canvas' });
    expect(pathFile('courses/X/documents/Notes.md')).toEqual({ category: 'document', filename: 'Notes.md' });
  });
});
