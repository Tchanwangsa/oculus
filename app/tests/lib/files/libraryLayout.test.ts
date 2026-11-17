import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathFile } from '../../../src/lib/files/pathCategory';
import { categoryOf, courseRelOf, inFolder, OWN_DIRS, recordingRel } from '../../../src/lib/files/libraryLayout';

// The same cases `category_of` is held to in
// app/src-tauri/src/library/layout/tests/category.rs.
const fixture = JSON.parse(
  readFileSync(join(import.meta.dir, '../../fixtures/library/categories.json'), 'utf8'),
) as { cases: [string, string][] };

describe('libraryLayout', () => {
  test('every shared fixture path has the category Rust gives it', () => {
    for (const [path, category] of fixture.cases) {
      expect([path, pathFile(path).category]).toEqual([path, category]);
    }
  });

  test('no prototype key reads as a folder or a synced file', () => {
    expect(categoryOf('constructor')).toBe('other');
    expect(categoryOf('toString/x.md')).toBe('other');
  });

  test('paths inside a subject lose the subject prefix, others are kept', () => {
    expect(courseRelOf('courses/X/files/a.pdf')).toBe('files/a.pdf');
    expect(courseRelOf('lectures/abc/transcript.vtt')).toBe('lectures/abc/transcript.vtt');
    expect(inFolder('courses/X/documents/a.md', OWN_DIRS.document)).toBe(true);
    expect(inFolder('courses/X/uploads/documents/a.md', OWN_DIRS.document)).toBe(false);
    expect(recordingRel('abc')).toBe('lectures/abc');
  });
});
