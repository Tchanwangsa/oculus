import { describe, expect, test } from 'bun:test';
import { pathFile } from '../../../src/lib/files/pathCategory';

describe('pathFile', () => {
  test('an own file takes its category from its extension, as category_of does', () => {
    expect(pathFile('courses/X/Week 3.notebook')).toEqual({ category: 'notebook', filename: 'Week 3.notebook' });
    expect(pathFile('courses/X/Board.canvas')).toEqual({ category: 'canvas', filename: 'Board.canvas' });
    expect(pathFile('courses/X/Exam prep/Notes.md')).toEqual({ category: 'document', filename: 'Notes.md' });
  });
});
